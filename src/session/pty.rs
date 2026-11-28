//! 将 Alacritty 工作线程和会话事件流接在一起。

use alacritty_terminal::{
    event::{Event, EventListener, WindowSize},
    event_loop::{EventLoop, EventLoopSender, Msg},
    tty::{self, Options},
};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use crate::terminal::Screen;

/// PTY 交给会话处理的事件。
pub(crate) enum SessionEvent {
    /// 终端输出、协议请求或 Shell 状态变化。
    Terminal(Event),
    /// 工作线程已结束，且 PTY 已释放。
    WorkerStopped {
        /// Alacritty 工作线程是否因 panic 退出。
        panicked: bool,
    },
}

/// 将 PTY 线程发出的事件送到应用线程。
#[derive(Clone)]
pub(crate) struct TerminalEvents {
    /// 会话事件的有序发送端。
    sender: UnboundedSender<SessionEvent>,
    /// 是否已有重绘提醒排队。
    redraw_pending: Arc<AtomicBool>,
}

impl TerminalEvents {
    /// 创建发送端和供工作区订阅的接收端。
    pub(super) fn channel() -> (Self, UnboundedReceiver<SessionEvent>) {
        let (sender, receiver) = unbounded();
        (
            Self {
                sender,
                redraw_pending: Arc::default(),
            },
            receiver,
        )
    }

    /// 消费重绘提醒，允许工作线程安排下一次。
    pub(super) fn redraw_handled(&self) {
        self.redraw_pending.store(false, Ordering::Relaxed);
    }
}
impl EventListener for TerminalEvents {
    fn send_event(&self, event: Event) {
        // 合并绘制提醒，保留退出和协议回复的顺序。
        if matches!(event, Event::Wakeup) && self.redraw_pending.swap(true, Ordering::Relaxed) {
            return;
        }
        let _ = self.sender.unbounded_send(SessionEvent::Terminal(event));
    }
}

/// 供会话发送输入和控制请求的 PTY 句柄。
pub(super) struct PtyWorker {
    /// Alacritty 工作线程的消息发送端。
    commands: EventLoopSender,
}

impl PtyWorker {
    /// 由协调线程等待 I/O 线程退出并释放底层 PTY。
    pub(super) fn start(
        options: Options,
        size: WindowSize,
        screen: &Screen<TerminalEvents>,
        events: TerminalEvents,
    ) -> io::Result<Self> {
        let pty = tty::new(&options, size, 0)?;
        let engine = EventLoop::new(
            screen.share(),
            events.clone(),
            pty,
            options.drain_on_exit,
            false,
        )?;
        let commands = engine.channel();
        thread::Builder::new()
            .name("moe-pty-cleanup".into())
            .spawn(move || {
                // 线程返回值持有 PTY。先释放，再通知界面。
                let panicked = engine.spawn().join().is_err();
                let _ = events
                    .sender
                    .unbounded_send(SessionEvent::WorkerStopped { panicked });
            })?;
        Ok(Self { commands })
    }

    /// 保序发送非空字节。
    pub(super) fn write(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() {
            let _ = self.commands.send(Msg::Input(bytes.into()));
        }
    }

    /// 尝试将尺寸变更放入 PTY 队列。
    pub(super) fn resize(&self, size: WindowSize) -> bool {
        self.commands.send(Msg::Resize(size)).is_ok()
    }

    /// 请求工作线程关闭。
    pub(super) fn shutdown(&self) {
        let _ = self.commands.send(Msg::Shutdown);
    }
}
