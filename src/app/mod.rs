//! 组装应用服务，启动首个会话和窗口。

pub(crate) mod paths;
mod services;

use services::AppServices;

use crate::{session::Session, shell, ui, workspace::Workspace};

/// 启动图形应用，供 CLI 选择进入。
pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let services = AppServices::new();
    let options = shell::options(services.paths.working_directory())?;
    let (session, events) = Session::start(options)?;
    ui::run(Workspace::new(session), events)
}
