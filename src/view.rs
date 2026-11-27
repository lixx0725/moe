//! 终端窗口负责网格绘制和键盘输入，Shell 文本由 PTY 解释。

use alacritty_terminal::{
    event::{Event, WindowSize},
    term::{TermMode, cell::Flags},
    vte::ansi::{Color, CursorShape, NamedColor},
};
use futures::{StreamExt, channel::mpsc::UnboundedReceiver};
use gpui::{
    Bounds, Context, ElementInputHandler, EntityInputHandler, FocusHandle, Font, FontFeatures,
    FontWeight, Hsla, IntoElement, KeyDownEvent, Pixels, Point, Size, TextAlign, TextRun,
    UTF16Selection, Window, canvas, div, fill, font, point, prelude::*, px, rgb, size,
};
use std::ops::Range;

use crate::{colors, keys, session::Session};

pub(crate) struct Terminal {
    focus: FocusHandle,
    session: Session,
    font: Font,
    cell: Size<Pixels>,
    origin: Point<Pixels>,
    cursor: Option<Caret>,
    marked: String,
    marked_selection: Range<usize>,
}

const FONT_SIZE: Pixels = px(14.);

impl Terminal {
    pub fn new(
        session: Session,
        mut events: UnboundedReceiver<Event>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let font_name = if cfg!(windows) {
            "Consolas"
        } else if cfg!(target_os = "macos") {
            "Menlo"
        } else {
            "DejaVu Sans Mono"
        };
        let mut font = font(font_name);
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
            session,
            font,
            cell: size(advance, px(20.)),
            origin: Point::default(),
            cursor: None,
            marked: String::new(),
            marked_selection: 0..0,
        }
    }

    fn on_event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match event {
            Event::Wakeup => {
                self.session.redraw();
                cx.notify();
            }
            Event::Title(title) => window.set_window_title(&title),
            Event::ResetTitle => window.set_window_title("moe"),
            Event::PtyWrite(text) => self.session.write(text.into_bytes()),
            Event::TextAreaSizeRequest(reply) => {
                self.session.write(reply(self.session.resize).into_bytes())
            }
            Event::ColorRequest(index, reply) => {
                let color = colors::reported(index, self.session.grid.lock().colors());
                self.session.write(reply(color).into_bytes());
            }
            Event::Exit | Event::ChildExit(_) => {
                window.remove_window();
                return false;
            }
            _ => {}
        }
        true
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.marked.is_empty() {
            return;
        }
        let key = &event.keystroke;
        let mode = *self.session.grid.lock().mode();
        let modifiers = key.modifiers;
        let paste = key.key == "v"
            && if cfg!(target_os = "macos") {
                modifiers.platform && !modifiers.control && !modifiers.alt
            } else {
                modifiers.control && modifiers.shift && !modifiers.alt && !modifiers.platform
            };
        let bytes = if paste {
            cx.read_from_clipboard()
                .and_then(|clipboard| clipboard.text())
                .map(|text| keys::paste(&text, mode.contains(TermMode::BRACKETED_PASTE)))
        } else {
            keys::encode(key, mode.contains(TermMode::APP_CURSOR))
        };
        if let Some(bytes) = bytes {
            self.session.write(bytes);
            cx.stop_propagation();
        }
    }

    fn capture(&mut self, bounds: Bounds<Pixels>, scale: f32) -> Vec<Run> {
        let aligned = |value: Pixels| px((f32::from(value) * scale).round() / scale);
        self.origin = point(aligned(bounds.origin.x), aligned(bounds.origin.y));
        let columns = ((bounds.size.width / self.cell.width).floor() as u16).clamp(2, 1024);
        let rows = ((bounds.size.height / self.cell.height).floor() as u16).clamp(1, 512);
        self.session.fit(WindowSize {
            num_cols: columns,
            num_lines: rows,
            cell_width: (f32::from(self.cell.width) * scale).round().clamp(1., 63.) as u16,
            cell_height: (f32::from(self.cell.height) * scale)
                .round()
                .clamp(1., 127.) as u16,
        });
        let term = self.session.grid.lock();
        let content = term.renderable_content();
        let cursor = content.cursor;
        self.cursor = usize::try_from(cursor.point.line.0)
            .ok()
            .filter(|row| *row < usize::from(rows))
            .map(|row| Caret {
                row,
                column: cursor.point.column.0,
                shape: cursor.shape,
                color: colors::resolve(Color::Named(NamedColor::Cursor), content.colors),
            });

        let mut runs: Vec<Run> = Vec::new();
        for entry in content.display_iter {
            let Ok(row) = usize::try_from(entry.point.line.0) else {
                continue;
            };
            let column = entry.point.column.0;
            let cell = entry.cell;
            if cell.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            let mut fg = colors::resolve(cell.fg, content.colors);
            let mut bg = colors::resolve(cell.bg, content.colors);
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let style = Style {
                fg,
                bg,
                bold: cell.flags.contains(Flags::BOLD),
            };
            let spacer = cell.flags.contains(Flags::WIDE_CHAR_SPACER);
            if spacer {
                continue;
            }
            let wide = cell.flags.contains(Flags::WIDE_CHAR);
            let hidden = cell.flags.contains(Flags::HIDDEN);
            let character = if hidden || cell.c == '\t' {
                ' '
            } else {
                cell.c
            };
            let marks = if hidden {
                &[]
            } else {
                cell.zerowidth().unwrap_or_default()
            };
            let width = if wide { 2 } else { 1 };
            // 独立绘制宽字和组合字，后续字符仍按终端列定位。
            let single = wide || !marks.is_empty();
            if !single
                && let Some(previous) = runs.last_mut()
                && previous.row == row
                && previous.column + previous.width == column
                && !previous.single
                && previous.style == style
            {
                previous.text.push(character);
                previous.width += width;
            } else {
                let mut text = character.to_string();
                text.extend(marks.iter().copied());
                runs.push(Run {
                    row,
                    column,
                    width,
                    text,
                    style,
                    single,
                });
            }
        }
        runs
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Style {
    fg: Hsla,
    bg: Hsla,
    bold: bool,
}

/// 当前光标的网格位置、形状与有效颜色。输入法使用同一份位置。
#[derive(Clone, Copy)]
struct Caret {
    row: usize,
    column: usize,
    shape: CursorShape,
    color: Hsla,
}

struct Run {
    row: usize,
    column: usize,
    width: usize,
    text: String,
    style: Style,
    single: bool,
}

impl Render for Terminal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let capture = cx.entity();
        let typing = capture.clone();
        let focus = self.focus.clone();
        let cell = self.cell;
        let font = self.font.clone();
        let background = colors::background();
        div()
            .id("terminal")
            .track_focus(&self.focus)
            .size_full()
            .overflow_hidden()
            .p(px(8.))
            .bg(background)
            .on_key_down(cx.listener(Self::key_down))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        capture.update(cx, |view, _| {
                            let runs = view.capture(bounds, window.scale_factor());
                            (runs, view.origin, view.cursor, view.marked.clone())
                        })
                    },
                    move |bounds, (runs, origin, cursor, marked), window, cx| {
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, typing.clone()),
                            cx,
                        );
                        let at = |row, column| {
                            origin + point(cell.width * column as f32, cell.height * row as f32)
                        };
                        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                            for run in runs {
                                let origin = at(run.row, run.column);
                                if run.style.bg != background {
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            origin,
                                            size(cell.width * run.width as f32, cell.height),
                                        ),
                                        run.style.bg,
                                    ));
                                }
                                let mut font = font.clone();
                                if run.style.bold {
                                    font.weight = FontWeight::BOLD;
                                }
                                draw_text(
                                    run.text,
                                    origin,
                                    cell,
                                    font,
                                    run.style.fg,
                                    (!run.single).then_some(cell.width),
                                    window,
                                    cx,
                                );
                            }
                            if let Some(cursor) = cursor {
                                let origin = at(cursor.row, cursor.column);
                                if cursor.shape != CursorShape::Hidden {
                                    let (offset, extent) = match cursor.shape {
                                        CursorShape::Underline => (
                                            point(px(0.), cell.height - px(2.)),
                                            size(cell.width, px(2.)),
                                        ),
                                        _ => (Point::default(), size(px(2.), cell.height)),
                                    };
                                    window.paint_quad(fill(
                                        Bounds::new(origin + offset, extent),
                                        cursor.color,
                                    ));
                                }
                                if !marked.is_empty() {
                                    draw_text(
                                        marked,
                                        origin,
                                        cell,
                                        font,
                                        rgb(0xf7cf8b).into(),
                                        None,
                                        window,
                                        cx,
                                    );
                                }
                            }
                        });
                    },
                )
                .size_full(),
            )
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    text: String,
    origin: Point<Pixels>,
    cell: Size<Pixels>,
    font: Font,
    color: Hsla,
    width: Option<Pixels>,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    if text.trim().is_empty() {
        return;
    }
    let run = TextRun {
        len: text.len(),
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text.into(), FONT_SIZE, &[run], width);
    let _ = line.paint(origin, cell.height, TextAlign::Left, None, window, cx);
}

impl EntityInputHandler for Terminal {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text: Vec<_> = self.marked.encode_utf16().collect();
        let range = range.start.min(text.len())..range.end.min(text.len());
        let part = text.get(range.clone())?;
        *actual = Some(range);
        Some(String::from_utf16_lossy(part))
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.marked_selection.clone(),
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.marked.is_empty()).then(|| 0..self.marked.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked.clear();
        self.marked_selection = 0..0;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.unmark_text(window, cx);
        self.session.write(text.as_bytes().to_vec());
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let len = text.encode_utf16().count();
        self.marked = text.to_owned();
        self.marked_selection = selection.unwrap_or(len..len);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let cursor = self.cursor?;
        Some(Bounds::new(
            self.origin
                + point(
                    self.cell.width * cursor.column as f32,
                    self.cell.height * cursor.row as f32,
                ),
            self.cell,
        ))
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
