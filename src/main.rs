//! 启动一个原生窗口及其唯一的终端会话。

mod colors;
mod keys;
mod session;
mod shell;
mod view;

use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use std::{cell::Cell, process::ExitCode, rc::Rc};

fn main() -> ExitCode {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--version")
    {
        println!("moe {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let (session, events) = match shell::options().and_then(session::Session::start) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("moe: {error}");
            return ExitCode::FAILURE;
        }
    };

    let failed = Rc::new(Cell::new(false));
    let launch_failed = failed.clone();
    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(900.), px(560.)), cx);
        let window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..WindowOptions::default()
            },
            move |window, cx| {
                window.set_window_title("moe");
                cx.new(|cx| view::Terminal::new(session, events, window, cx))
            },
        );
        match window {
            Ok(_) => cx.activate(true),
            Err(error) => {
                eprintln!("moe: {error}");
                launch_failed.set(true);
                cx.quit();
            }
        }
    });
    if failed.get() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
