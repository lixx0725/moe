//! 接入终端会话事件与原生输入。

mod ime;
mod render;

use futures::{StreamExt, channel::mpsc::UnboundedReceiver};
use gpui::{
    Context, FocusHandle, Font, FontFeatures, KeyDownEvent, Pixels, Point, Size, Window, font, px,
    size,
};
use std::ops::Range;

use self::render::Caret;
use crate::{
    interaction::{self, InteractionContext, KeyboardInput},
    platform,
    session::SessionEvent,
    terminal::input,
    workspace::{Workspace, WorkspaceUpdate},
};

/// 绘制终端并接收原生输入的窗口组件。
pub(super) struct TerminalView {
    /// 原生键盘焦点。
    focus: FocusHandle,
    /// 当前窗口的工作区及会话。
    workspace: Workspace,
    /// 已选择的终端字体。
    font: Font,
    /// 单元格的逻辑像素尺寸。
    cell: Size<Pixels>,
    /// 网格左上角的逻辑像素坐标。
    origin: Point<Pixels>,
    /// 当前帧可见的光标位置。
    cursor: Option<Caret>,
    /// 输入法尚未确认的文本。
    marked: String,
    /// 输入法选区，使用 UTF-16 码元索引。
    marked_selection: Range<usize>,
}

const FONT_SIZE: Pixels = px(14.);

impl TerminalView {
    /// 初始化网格度量，并订阅会话事件。
    pub(super) fn new(
        workspace: Workspace,
        mut events: UnboundedReceiver<SessionEvent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let mut font = font(platform::terminal_font());
        font.features = FontFeatures::disable_ligatures();
        let text = cx.text_system();
        let advance = text
            .advance(text.resolve_font(&font), FONT_SIZE, 'M')
            .map_or(px(8.4), |size| size.width.max(px(1.)));
        cx.spawn_in(window, async move |view, cx| {
            while let Some(event) = events.next().await {
                let result =
                    view.update_in(cx, |view, window, cx| view.on_event(event, window, cx));
                if !matches!(result, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
        Self {
            focus,
            workspace,
            font,
            cell: size(advance, px(20.)),
            origin: Point::default(),
            cursor: None,
            marked: String::new(),
            marked_selection: 0..0,
        }
    }

    /// 转交原生关窗请求，等会话收尾。
    pub(super) fn request_close(&mut self) -> bool {
        self.workspace.request_close()
    }

    /// 执行工作区更新，返回是否继续消费事件。
    fn on_event(
        &mut self,
        event: SessionEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match self.workspace.handle_event(event) {
            Some(WorkspaceUpdate::Redraw) => cx.notify(),
            Some(WorkspaceUpdate::Title(title)) => window.set_window_title(&title),
            Some(WorkspaceUpdate::CloseWindow) => {
                window.remove_window();
                return false;
            }
            None => {}
        }
        true
    }

    /// 路由按键，普通文本由原生输入处理器提交。
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let context = InteractionContext {
            focused: self.focus.is_focused(window),
            composing: !self.marked.is_empty(),
            mode: self.workspace.session().mode(),
        };
        let bytes = match interaction::route_key(&event.keystroke, &context) {
            Some(KeyboardInput::Paste { bracketed }) => cx
                .read_from_clipboard()
                .and_then(|clipboard| clipboard.text())
                .map(|text| input::paste(&text, bracketed)),
            Some(KeyboardInput::Write(bytes)) => Some(bytes),
            None => None,
        };
        if let Some(bytes) = bytes {
            self.workspace.session().write(bytes);
            cx.stop_propagation();
        }
    }
}
