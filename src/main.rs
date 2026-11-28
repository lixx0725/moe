//! 将命令行参数交给 CLI，并按需要启动原生终端。

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod cli;
mod interaction;
mod platform;
mod session;
mod shell;
mod terminal;
mod ui;
mod workspace;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args = std::env::args_os();
    #[cfg(windows)]
    if args.len() > 1 {
        platform::windows::attach_parent_console();
    }
    cli::run(args)
}
