//! 管理窗口里的会话，以及会话变化对应的界面行为。

use crate::session::{Session, SessionEvent, SessionUpdate};

/// 当前窗口的工作区。
pub(crate) struct Workspace {
    /// 终端窗格显示的会话。
    session: Session,
}

/// 工作区需要界面执行的操作。
pub(crate) enum WorkspaceUpdate {
    /// 网格更新，需要重绘。
    Redraw,
    /// 更新窗口标题。
    Title(String),
    /// 唯一的会话已收尾，关闭窗口。
    CloseWindow,
}

impl Workspace {
    pub(crate) const DEFAULT_TITLE: &str = "moe";

    /// 用已启动的会话创建工作区。
    pub(crate) fn new(session: Session) -> Self {
        Self { session }
    }
    /// 读取当前窗格的会话。
    pub(crate) fn session(&self) -> &Session {
        &self.session
    }
    /// 更新当前窗格的会话。
    pub(crate) fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }
    /// 请求关闭窗口，等待会话完成后才真正移除。
    pub(crate) fn request_close(&mut self) -> bool {
        self.session.request_close()
    }

    /// 将会话更新转换为界面操作。
    pub(crate) fn handle_event(&mut self, event: SessionEvent) -> Option<WorkspaceUpdate> {
        Some(match self.session.handle_event(event)? {
            SessionUpdate::Redraw => WorkspaceUpdate::Redraw,
            SessionUpdate::Title(title) => {
                WorkspaceUpdate::Title(title.unwrap_or_else(|| Self::DEFAULT_TITLE.to_owned()))
            }
            SessionUpdate::Finished(reason) => {
                if let Some(message) = reason.error_message() {
                    eprintln!("moe: {message}");
                }
                WorkspaceUpdate::CloseWindow
            }
        })
    }
}
