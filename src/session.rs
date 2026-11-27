//! Alacritty 管理真实 PTY，视图持有独立的终端会话。

use alacritty_terminal::{
    event::{Event, EventListener, WindowSize},
    event_loop::{EventLoop, EventLoopSender, Msg, State},
    sync::FairMutex,
    term::{Config, Term, test::TermSize},
    tty::{self, Options, Pty},
};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

#[derive(Clone)]
pub(crate) struct Events {
    sender: UnboundedSender<Event>,
    redraw_pending: Arc<AtomicBool>,
}

impl EventListener for Events {
    fn send_event(&self, event: Event) {
        // 合并绘制提醒，保留退出和协议回复的顺序。
        if matches!(event, Event::Wakeup) && self.redraw_pending.swap(true, Ordering::Relaxed) {
            return;
        }
        let _ = self.sender.unbounded_send(event);
    }
}

type Worker = JoinHandle<(EventLoop<Pty, Events>, State)>;

/// PTY 事件线程独占子进程；读取网格仅短暂持锁。
pub(crate) struct Session {
    pub grid: Arc<FairMutex<Term<Events>>>,
    pub resize: WindowSize,
    writer: EventLoopSender,
    worker: Option<Worker>,
    redraw_pending: Arc<AtomicBool>,
}

impl Session {
    pub fn start(options: Options) -> io::Result<(Self, UnboundedReceiver<Event>)> {
        let resize = WindowSize {
            num_cols: 80,
            num_lines: 24,
            cell_width: 8,
            cell_height: 19,
        };
        let (sender, receiver) = unbounded();
        let redraw_pending = Arc::new(AtomicBool::new(false));
        let events = Events {
            sender,
            redraw_pending: redraw_pending.clone(),
        };
        let grid = Arc::new(FairMutex::new(Term::new(
            Config::default(),
            &grid_size(resize),
            events.clone(),
        )));
        let pty = tty::new(&options, resize, 0)?;
        let event_loop = EventLoop::new(grid.clone(), events, pty, options.drain_on_exit, false)?;
        let writer = event_loop.channel();
        Ok((
            Self {
                grid,
                resize,
                writer,
                worker: Some(event_loop.spawn()),
                redraw_pending,
            },
            receiver,
        ))
    }

    pub fn redraw(&self) {
        self.redraw_pending.store(false, Ordering::Relaxed);
    }

    pub fn write(&self, bytes: Vec<u8>) {
        let _ = self.writer.send(Msg::Input(bytes.into()));
    }

    pub fn fit(&mut self, next: WindowSize) {
        let current = &self.resize;
        if (
            current.num_cols,
            current.num_lines,
            current.cell_width,
            current.cell_height,
        ) != (
            next.num_cols,
            next.num_lines,
            next.cell_width,
            next.cell_height,
        ) {
            self.grid.lock().resize(grid_size(next));
            if self.writer.send(Msg::Resize(next)).is_ok() {
                self.resize = next;
            }
        }
    }

    pub fn stop(&self) {
        let _ = self.writer.send(Msg::Shutdown);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
        if let Some(worker) = self.worker.take() {
            // 等待事件循环注销原生句柄，随后释放 PTY 并回收 Shell。
            let _ = worker.join();
        }
    }
}

fn grid_size(winsize: WindowSize) -> TermSize {
    TermSize::new(
        usize::from(winsize.num_cols),
        usize::from(winsize.num_lines),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn interactive_default_shell_accepts_a_command_and_exits() {
        let (mut session, mut events) =
            Session::start(crate::shell::options().expect("default shell"))
                .expect("start real PTY");
        session.fit(WindowSize {
            num_cols: 97,
            num_lines: 29,
            ..session.resize
        });
        let command = if cfg!(windows) {
            "Write-Output ('moe_' + 'smoke')\r"
        } else {
            "printf 'moe_%s\\n' smoke\r"
        };
        session.write(command.as_bytes().to_owned());

        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            while let Ok(event) = events.try_recv() {
                assert!(
                    !matches!(event, Event::ChildExit(_) | Event::Exit),
                    "shell exited before the command"
                );
            }
            let output: String = session
                .grid
                .lock()
                .renderable_content()
                .display_iter
                .map(|cell| cell.cell.c)
                .collect();
            if output.contains("moe_smoke") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "no command output in the real PTY: {output:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        session.write(b"exit\r".to_vec());
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            if let Ok(event) = events.try_recv() {
                if matches!(event, Event::Exit | Event::ChildExit(_)) {
                    break;
                }
            } else {
                assert!(Instant::now() < deadline, "shell did not exit");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
