//! 查询版本或启动原生终端。

use std::{ffi::OsString, process::ExitCode};

/// 处理启动参数，返回进程退出码。
pub(crate) fn run(mut args: impl Iterator<Item = OsString>) -> ExitCode {
    if args.next().is_some_and(|arg| arg == "--version") {
        println!("moe {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    match crate::app::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("moe: {error}");
            ExitCode::FAILURE
        }
    }
}
