//! 在真实 PTY 上验证会话，不依赖显示服务器。

use alacritty_terminal::{
    event::{Event, WindowSize},
    grid::Dimensions,
    term::cell::Flags,
    tty::Options,
};
use futures::channel::mpsc::UnboundedReceiver;
use std::{
    collections::VecDeque,
    thread,
    time::{Duration, Instant},
};

use super::{EndReason, Session, SessionEvent, SessionUpdate};
use crate::app::paths::AppPaths;

const DEADLINE: Duration = Duration::from_secs(15);

/// 驱动真实会话并收集失败诊断。
struct Harness {
    /// 测试和应用共用的会话对象。
    session: Session,
    /// 来自真实 PTY 的有序事件。
    events: UnboundedReceiver<SessionEvent>,
    /// 最近的事件摘要。
    recent: VecDeque<String>,
    /// 资源回收后的退出原因。
    ended: Option<EndReason>,
}
impl Harness {
    /// 启动测试会话。
    fn start(options: Options) -> Self {
        let (session, events) = Session::start(options).expect("启动真实 PTY");
        Self {
            session,
            events,
            recent: VecDeque::new(),
            ended: None,
        }
    }

    /// 发送一行 Shell 输入。
    fn command(&self, command: &str) {
        self.session.write(format!("{command}\r").into_bytes());
    }

    /// 读取当前可见文本，跳过宽字符的占位单元格。
    fn screen(&self) -> String {
        self.session.read_screen(|term| {
            term.renderable_content()
                .display_iter
                .filter(|entry| {
                    !entry
                        .cell
                        .flags
                        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                })
                .map(|entry| entry.cell.c)
                .collect()
        })
    }

    /// 分批处理事件，持续输出时也能检查超时。
    fn until<T>(&mut self, step: &str, ready: impl Fn(&Self) -> Option<T>) -> T {
        let deadline = Instant::now() + DEADLINE;
        loop {
            for _ in 0..64 {
                let Ok(event) = self.events.try_recv() else {
                    break;
                };
                if self.recent.len() == 12 {
                    self.recent.pop_front();
                }
                self.recent.push_back(match &event {
                    SessionEvent::Terminal(Event::Wakeup) => "Wakeup".to_owned(),
                    SessionEvent::Terminal(event) => format!("{event:?}"),
                    SessionEvent::WorkerStopped { panicked } => {
                        format!("WorkerStopped(panicked={panicked})")
                    }
                });
                if let Some(SessionUpdate::Finished(reason)) = self.session.handle_event(event) {
                    self.ended = Some(reason);
                }
            }
            if let Some(result) = ready(self) {
                return result;
            }
            if self.ended.is_some() || Instant::now() >= deadline {
                let tail: String = self
                    .screen()
                    .chars()
                    .rev()
                    .skip_while(|ch| ch.is_whitespace())
                    .take(900)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                panic!(
                    "{step}失败\n结束原因：{:?}\n最近事件：{:?}\n输出末尾：{tail:?}",
                    self.ended, self.recent
                );
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// 等待由命令拼接出的标记，输入回显无法匹配。
    fn wait_for(&mut self, marker: &str) {
        self.until(marker, |harness| {
            harness.screen().contains(marker).then_some(())
        });
    }

    /// 等待 PTY 完全释放。
    fn wait_for_end(&mut self) -> EndReason {
        self.until("等待 PTY 释放资源", |harness| harness.ended)
    }

    /// 重复请求关闭，验证会话只清理一次。
    fn close(&mut self) {
        assert!(!self.session.request_close());
        assert!(!self.session.request_close());
        assert_eq!(self.wait_for_end(), EndReason::Requested);
        assert!(self.session.request_close());
    }
}

/// 使用固定 Shell，避免本机配置脚本干扰测试。
fn fixture() -> Harness {
    let directory = AppPaths::discover().working_directory();
    #[cfg(windows)]
    let options = crate::shell::test_options(directory).expect("获取无启动脚本的 PowerShell");
    #[cfg(unix)]
    let options = {
        let mut options = crate::shell::options(directory).expect("获取默认 Shell 配置");
        options.shell = Some(alacritty_terminal::tty::Shell::new(
            "/bin/sh".to_owned(),
            vec!["-i".to_owned()],
        ));
        options.env.insert("ENV".to_owned(), String::new());
        options
    };
    Harness::start(options)
}

/// 选取当前测试 Shell 对应的命令。
fn command_for_shell<'a>(windows: &'a str, unix: &'a str) -> &'a str {
    if cfg!(windows) { windows } else { unix }
}

#[test]
fn pty_default_shell_executes_and_exits() {
    let directory = AppPaths::discover().working_directory();
    let mut harness = Harness::start(crate::shell::options(directory).expect("发现平台默认 Shell"));
    harness.command(command_for_shell(
        "Write-Host ('moe_' + 'ready')",
        "printf 'moe_%s\\n' ready",
    ));
    harness.wait_for("moe_ready");
    harness.command("exit");
    assert!(matches!(harness.wait_for_end(), EndReason::Shell(_)));
}

#[test]
fn pty_unicode_round_trip() {
    let mut harness = fixture();
    harness.command(command_for_shell(
        "Write-Host ('moe_' + '中文' + 'é')",
        "printf 'moe_%s%s\\n' '中文' 'é'",
    ));
    harness.wait_for("moe_中文é");
    harness.close();
}

#[test]
fn pty_resize_reaches_the_shell_and_terminal_grid() {
    let mut harness = fixture();
    harness.session.fit(WindowSize {
        num_cols: 97,
        num_lines: 29,
        ..harness.session.size
    });
    assert_eq!(
        harness
            .session
            .read_screen(|term| (term.columns(), term.screen_lines())),
        (97, 29)
    );
    harness.command(command_for_shell(
        "Write-Host ('moe_' + 'size_' + [Console]::WindowHeight + 'x' + [Console]::WindowWidth)",
        "printf 'moe_%s%s\\n' size_ \"$(stty size | tr ' ' x)\"",
    ));
    harness.wait_for("moe_size_29x97");
    harness.close();
}

#[test]
fn pty_ctrl_c_interrupts_foreground_command_and_keeps_shell_running() {
    let mut harness = fixture();
    #[cfg(windows)]
    {
        // 等新提示符再发送后续命令，避免输入进入中断处理过程。
        harness.command("function prompt { $global:moePromptNumber++; 'moe_prompt_' + $global:moePromptNumber + '> ' }");
        harness.wait_for("moe_prompt_1>");
    }
    harness.command(command_for_shell(
        "Write-Host ('moe_' + 'busy'); Start-Sleep -Seconds 120",
        "/bin/sh -c 'printf \"moe_%s\\n\" busy; exec sleep 120'",
    ));
    harness.wait_for("moe_busy");
    harness.session.write(vec![3]);
    #[cfg(windows)]
    harness.wait_for("moe_prompt_2>");
    harness.command(command_for_shell(
        "Write-Host ('moe_' + 'resumed')",
        "printf 'moe_%s\\n' resumed",
    ));
    harness.wait_for("moe_resumed");
    harness.close();
}

#[test]
fn pty_exit_preserves_final_output_and_status_when_available() {
    let mut harness = fixture();
    harness.command(command_for_shell(
        "Write-Host -NoNewline ('moe_' + 'final'); exit 7",
        "printf 'moe_%s' final; exit 7",
    ));
    let EndReason::Shell(status) = harness.wait_for_end() else {
        panic!("未收到 Shell 退出事件");
    };
    if let Some(status) = status {
        assert_eq!(status.code(), Some(7));
    }
    assert!(harness.screen().contains("moe_final"));
}

#[test]
fn pty_close_idle_session() {
    let mut harness = fixture();
    harness.command(command_for_shell(
        "Write-Host ('moe_' + 'idle')",
        "printf 'moe_%s\\n' idle",
    ));
    harness.wait_for("moe_idle");
    harness.close();
}

#[test]
fn pty_close_while_shell_is_producing_output() {
    let mut harness = fixture();
    harness.command(command_for_shell(
        "while ($true) { Write-Host ('moe_' + 'pulse'); Start-Sleep -Milliseconds 25 }",
        "while :; do printf 'moe_%s\\n' pulse; sleep 0.025; done",
    ));
    harness.wait_for("moe_pulse");
    harness.close();
}
