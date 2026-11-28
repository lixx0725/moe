//! 创建首个工作区窗口，最后一个窗口关闭后退出应用。

use futures::channel::mpsc::UnboundedReceiver;
use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use std::{cell::RefCell, rc::Rc};

use super::terminal::TerminalView;
use crate::{session::SessionEvent, workspace::Workspace};

/// 打开窗口并运行图形事件循环。
pub(crate) fn run(
    workspace: Workspace,
    events: UnboundedReceiver<SessionEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let failure = Rc::new(RefCell::new(None));
    let launch_failure = failure.clone();
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
                window.set_window_title(Workspace::DEFAULT_TITLE);
                let view = cx.new(|cx| TerminalView::new(workspace, events, window, cx));
                let weak_view = view.downgrade();
                window.on_window_should_close(cx, move |_, cx| {
                    // 暂留窗口接收结束事件，PTY 回收不阻塞 UI 线程。
                    weak_view
                        .update(cx, |view, _| view.request_close())
                        .unwrap_or(true)
                });
                view
            },
        );
        match window {
            Ok(_) => cx.activate(true),
            Err(error) => {
                *launch_failure.borrow_mut() = Some(error);
                cx.quit();
            }
        }
    });
    if let Some(error) = failure.take() {
        Err(error.into())
    } else {
        Ok(())
    }
}
