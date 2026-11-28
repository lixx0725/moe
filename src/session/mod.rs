//! 连接 Shell、PTY 和终端网格，统一处理输入、协议回复和退出。

mod lifecycle;
mod pty;
#[cfg(test)]
mod tests;

use crate::terminal::{Screen, palette};
use alacritty_terminal::{
    event::{Event, WindowSize},
    term::{Term, TermMode},
    tty::Options,
};
use futures::channel::mpsc::UnboundedReceiver;
use lifecycle::{EndReason, Lifecycle};
pub(crate) use pty::SessionEvent;
use pty::{PtyWorker, TerminalEvents};
use std::io;

/// 工作区需要处理的会话变化。
pub(crate) enum SessionUpdate {
    /// 网格或光标改变，需要重绘。
    Redraw,
    /// 更新窗口标题，空值表示恢复默认标题。
    Title(Option<String>),
    /// PTY 已释放，可以移除会话。
    Finished(EndReason),
}

/// 在本地 PTY 上运行的终端会话。
pub(crate) struct Session {
    /// Alacritty 驱动的终端网格。
    screen: Screen<TerminalEvents>,
    /// 已提交给 PTY 的尺寸。行列以单元格计，字宽及行高以物理像素计。
    size: WindowSize,
    /// 工作线程的控制句柄。
    worker: PtyWorker,
    /// 事件发送端，记录重绘提醒是否正在等待消费。
    events: TerminalEvents,
    /// 运行及关闭状态。
    lifecycle: Lifecycle,
}

impl Session {
    /// 启动默认尺寸的本地会话。
    pub(crate) fn start(options: Options) -> io::Result<(Self, UnboundedReceiver<SessionEvent>)> {
        let size = WindowSize {
            num_cols: 80,
            num_lines: 24,
            cell_width: 8,
            cell_height: 19,
        };
        let (events, receiver) = TerminalEvents::channel();
        let screen = Screen::new(size, events.clone());
        let worker = PtyWorker::start(options, size, &screen, events.clone())?;
        Ok((
            Self {
                screen,
                size,
                worker,
                events,
                lifecycle: Lifecycle::default(),
            },
            receiver,
        ))
    }

    /// 运行期间向 PTY 提交已编码的输入。
    pub(crate) fn write(&self, bytes: Vec<u8>) {
        if self.lifecycle.is_running() {
            self.worker.write(bytes);
        }
    }

    /// 读取当前终端模式，供输入路由使用。
    pub(crate) fn mode(&self) -> TermMode {
        self.screen.read(|term| *term.mode())
    }

    /// 短暂读取网格，在返回后释放锁。
    pub(crate) fn read_screen<R>(&self, read: impl FnOnce(&Term<TerminalEvents>) -> R) -> R {
        self.screen.read(read)
    }

    /// 将尺寸变更同步到 PTY 和网格。
    pub(crate) fn fit(&mut self, next: WindowSize) {
        let current = self.size;
        if !self.lifecycle.is_running()
            || (
                current.num_cols,
                current.num_lines,
                current.cell_width,
                current.cell_height,
            ) == (
                next.num_cols,
                next.num_lines,
                next.cell_width,
                next.cell_height,
            )
        {
            return;
        }
        if self.screen.resize(next, || self.worker.resize(next)) {
            self.size = next;
        }
    }

    /// 发起关闭。返回值表示界面是否可以立即销毁。
    pub(crate) fn request_close(&mut self) -> bool {
        if self.lifecycle.begin_close(EndReason::Requested) {
            self.worker.shutdown();
        }
        self.lifecycle == Lifecycle::Finished
    }

    /// 处理协议回复、输出更新和关闭事件。
    pub(crate) fn handle_event(&mut self, event: SessionEvent) -> Option<SessionUpdate> {
        match event {
            SessionEvent::Terminal(Event::Wakeup) => {
                self.events.redraw_handled();
                Some(SessionUpdate::Redraw)
            }
            SessionEvent::Terminal(Event::Title(title)) => Some(SessionUpdate::Title(Some(title))),
            SessionEvent::Terminal(Event::ResetTitle) => Some(SessionUpdate::Title(None)),
            SessionEvent::Terminal(Event::PtyWrite(text)) => {
                self.worker.write(text.into_bytes());
                None
            }
            SessionEvent::Terminal(Event::TextAreaSizeRequest(reply)) => {
                self.worker.write(reply(self.size).into_bytes());
                None
            }
            SessionEvent::Terminal(Event::ColorRequest(index, reply)) => {
                let rgb = self
                    .screen
                    .read(|term| palette::reported(index, term.colors()));
                self.worker.write(reply(rgb).into_bytes());
                None
            }
            SessionEvent::Terminal(Event::ChildExit(status)) => {
                self.lifecycle.begin_close(EndReason::Shell(Some(status)));
                None
            }
            SessionEvent::Terminal(Event::Exit) => {
                self.lifecycle.begin_close(EndReason::Shell(None));
                None
            }
            SessionEvent::Terminal(_) => None,
            SessionEvent::WorkerStopped { panicked } => {
                self.lifecycle.finish(panicked).map(SessionUpdate::Finished)
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // 启动失败等路径也要请求结束，资源由后台线程回收。
        self.request_close();
    }
}
