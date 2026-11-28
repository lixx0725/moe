//! 记录会话为什么结束，以及资源是否已回收。

use std::process::ExitStatus;

/// 会话结束的原因。Shell 退出码由 PTY 事件提供。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EndReason {
    /// 用户或窗口请求关闭会话。
    Requested,
    /// Shell 自然退出。部分平台无法提供退出状态。
    Shell(Option<ExitStatus>),
    /// PTY 停止，未收到关闭请求或 Shell 退出通知。
    PtyStopped,
    /// PTY 工作线程异常终止。
    PtyPanicked,
}

impl EndReason {
    /// 返回需要呈现的 PTY 错误。
    pub(crate) fn error_message(self) -> Option<&'static str> {
        match self {
            Self::PtyStopped => Some("PTY stopped unexpectedly"),
            Self::PtyPanicked => Some("PTY worker panicked"),
            Self::Requested | Self::Shell(_) => None,
        }
    }
}

/// 会话从运行到资源回收的状态。
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum Lifecycle {
    /// 会话接收输入和尺寸变化。
    #[default]
    Running,
    /// 会话已停止接收输入，正在等待 PTY 回收。
    Closing(EndReason),
    /// PTY 已回收，工作区可以移除此会话。
    Finished,
}

impl Lifecycle {
    /// 是否还能接收普通输入。
    pub(super) fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// 首个关闭原因生效，避免重复关闭同一个 PTY。
    pub(super) fn begin_close(&mut self, reason: EndReason) -> bool {
        if !self.is_running() {
            return false;
        }
        *self = Self::Closing(reason);
        true
    }

    /// PTY 已释放时结束会话，并返回最终原因。
    pub(super) fn finish(&mut self, panicked: bool) -> Option<EndReason> {
        let reason = match *self {
            Self::Running => EndReason::PtyStopped,
            Self::Closing(reason) => reason,
            Self::Finished => return None,
        };
        *self = Self::Finished;
        Some(if panicked {
            EndReason::PtyPanicked
        } else {
            reason
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_is_idempotent_and_finishes_after_worker_shutdown() {
        let mut lifecycle = Lifecycle::default();
        assert!(lifecycle.begin_close(EndReason::Requested));
        assert!(!lifecycle.begin_close(EndReason::Shell(None)));
        assert_eq!(lifecycle, Lifecycle::Closing(EndReason::Requested));
        assert_eq!(lifecycle.finish(false), Some(EndReason::Requested));
        assert_eq!(lifecycle.finish(false), None);
    }

    #[test]
    fn unexpected_worker_exit_and_panic_have_distinct_reasons() {
        assert_eq!(
            Lifecycle::default().finish(false),
            Some(EndReason::PtyStopped)
        );
        let mut lifecycle = Lifecycle::default();
        lifecycle.begin_close(EndReason::Requested);
        assert_eq!(lifecycle.finish(true), Some(EndReason::PtyPanicked));
    }
}
