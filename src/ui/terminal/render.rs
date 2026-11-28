//! 将网格数据转成 GPUI 绘制使用的字形和颜色。

use super::{FONT_SIZE, TerminalView};
use crate::terminal::palette;
use alacritty_terminal::{
    event::WindowSize,
    term::{cell::Flags, color::Colors},
    vte::ansi::{Color, CursorShape, NamedColor, Rgb},
};
use gpui::{
    Bounds, Context, ElementInputHandler, Font, FontWeight, Hsla, IntoElement, Pixels, Point, Size,
    TextAlign, TextRun, Window, canvas, div, fill, point, prelude::*, px, rgb, size,
};

impl TerminalView {
    /// 同步尺寸并读取一帧所需的绘制数据。
    fn capture(&mut self, bounds: Bounds<Pixels>, scale: f32) -> Vec<CellRun> {
        let aligned = |value: Pixels| px((f32::from(value) * scale).round() / scale);
        self.origin = point(aligned(bounds.origin.x), aligned(bounds.origin.y));
        let columns = ((bounds.size.width / self.cell.width).floor() as u16).clamp(2, 1024);
        let rows = ((bounds.size.height / self.cell.height).floor() as u16).clamp(1, 512);
        self.workspace.session_mut().fit(WindowSize {
            num_cols: columns,
            num_lines: rows,
            cell_width: (f32::from(self.cell.width) * scale).round().clamp(1., 63.) as u16,
            cell_height: (f32::from(self.cell.height) * scale)
                .round()
                .clamp(1., 127.) as u16,
        });
        let (cursor, runs) = self.workspace.session().read_screen(|term| {
            let content = term.renderable_content();
            let cursor = content.cursor;
            let cursor = usize::try_from(cursor.point.line.0)
                .ok()
                .filter(|row| *row < usize::from(rows))
                .map(|row| Caret {
                    row,
                    column: cursor.point.column.0,
                    shape: cursor.shape,
                    color: resolve(Color::Named(NamedColor::Cursor), content.colors),
                });

            let mut runs: Vec<CellRun> = Vec::new();
            for entry in content.display_iter {
                let Ok(row) = usize::try_from(entry.point.line.0) else {
                    continue;
                };
                let column = entry.point.column.0;
                let cell = entry.cell;
                if cell.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }
                let mut fg = resolve(cell.fg, content.colors);
                let mut bg = resolve(cell.bg, content.colors);
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
                    runs.push(CellRun {
                        row,
                        column,
                        width,
                        text,
                        style,
                        single,
                    });
                }
            }
            (cursor, runs)
        });
        self.cursor = cursor;
        runs
    }
}

/// 连续单元格共用的绘制样式。
#[derive(Clone, Copy, PartialEq)]
struct Style {
    /// 前景色。
    fg: Hsla,
    /// 背景色。
    bg: Hsla,
    /// 是否使用粗体。
    bold: bool,
}

/// 当前光标的网格位置、形状与有效颜色。输入法使用同一份位置。
#[derive(Clone, Copy)]
pub(super) struct Caret {
    /// 可见网格行。
    pub(super) row: usize,
    /// 起始网格列。
    pub(super) column: usize,
    /// 终端协议指定的光标形状。
    shape: CursorShape,
    /// 有效绘制颜色。
    color: Hsla,
}

/// 样式一致的一段网格内容。
struct CellRun {
    /// 可见网格行。
    row: usize,
    /// 起始网格列。
    column: usize,
    /// 占用的网格列数。
    width: usize,
    /// 连续单元格的文字。
    text: String,
    /// 连续单元格的绘制样式。
    style: Style,
    /// 宽字符和组合字符单独整形。
    single: bool,
}

impl Render for TerminalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let capture = cx.entity();
        let typing = capture.clone();
        let focus = self.focus.clone();
        let cell = self.cell;
        let font = self.font.clone();
        let background = display_color(palette::background());
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

fn display_color(color: Rgb) -> Hsla {
    rgb(u32::from_be_bytes([0, color.r, color.g, color.b])).into()
}

fn resolve(color: Color, overrides: &Colors) -> Hsla {
    display_color(palette::resolve(color, overrides))
}
