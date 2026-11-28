//! 将命令行参数交给 CLI，并按需要启动原生终端。

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
    cli::run(std::env::args_os().skip(1))
}
