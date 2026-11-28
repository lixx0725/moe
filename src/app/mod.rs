//! 组装应用服务，启动首个会话和窗口。

pub(crate) mod paths;
mod services;

use services::AppServices;
use std::{ffi::OsString, path::PathBuf};

use crate::{session::Session, shell, ui, workspace::Workspace};

/// 解析本次 Shell 选择，再启动会话与图形窗口。
pub(crate) fn run(
    program: Option<PathBuf>,
    shell_args: Vec<OsString>,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    let _installation_guard = crate::platform::windows::hold_installation_guard()?;
    let services = AppServices::new();
    let launch = shell::Launch::resolve(program, shell_args, services.paths.working_directory())?;
    let (session, events) =
        Session::start(launch.options()?).map_err(|source| shell::LaunchError::Start {
            program: launch.program().to_owned(),
            source,
        })?;
    ui::run(Workspace::new(session), events)
}
