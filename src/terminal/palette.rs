//! 将 ANSI 颜色和程序设置的调色板覆盖映射到窗口颜色。

use alacritty_terminal::{
    term::color::{COUNT, Colors},
    vte::ansi::{Color, NamedColor, Rgb},
};

const FOREGROUND: u32 = 0xe2e8f0;
const BACKGROUND: u32 = 0x171b24;
const CURSOR: u32 = 0xa8d5ff;
const ANSI16: [u32; 16] = [
    0x242a36, 0xef727d, 0x84cf9c, 0xe5ba68, 0x7da9f4, 0xc494df, 0x65c9c5, 0xd8dee9, 0x606b7c,
    0xff9fa7, 0xa8e5ba, 0xffdb8e, 0xaacbff, 0xe0b7f5, 0x9de6e3, 0xf5f7fb,
];

/// 默认终端背景。
pub(crate) fn background() -> Rgb {
    unpack(BACKGROUND)
}

/// 解析 ANSI 颜色和程序临时设置的调色板。
pub(crate) fn resolve(color: Color, overrides: &Colors) -> Rgb {
    match color {
        Color::Spec(value) => value,
        Color::Indexed(index) => reported(usize::from(index), overrides),
        Color::Named(name) => reported(name as usize, overrides),
    }
}

/// OSC 查询和绘制共用颜色解析，保持程序读回的值与显示一致。
pub(crate) fn reported(index: usize, overrides: &Colors) -> Rgb {
    let index = if index < COUNT {
        index
    } else {
        NamedColor::Foreground as usize
    };
    if let Some(value) = overrides[index] {
        return value;
    }
    match index {
        0..=15 => unpack(ANSI16[index]),
        16..=231 => {
            const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
            let n = index - 16;
            Rgb {
                r: LEVELS[n / 36],
                g: LEVELS[(n / 6) % 6],
                b: LEVELS[n % 6],
            }
        }
        232..=255 => {
            let level = (8 + 10 * (index - 232)) as u8;
            Rgb {
                r: level,
                g: level,
                b: level,
            }
        }
        256 => unpack(FOREGROUND),
        257 => unpack(BACKGROUND),
        258 => unpack(CURSOR),
        259..=266 => dim(reported(index - 259, overrides)),
        267 => reported(NamedColor::Foreground as usize, overrides),
        _ => dim(reported(NamedColor::Foreground as usize, overrides)),
    }
}

fn unpack(packed: u32) -> Rgb {
    let [_, r, g, b] = packed.to_be_bytes();
    Rgb { r, g, b }
}

fn dim(color: Rgb) -> Rgb {
    let shade = |value| (u16::from(value) * 2 / 3) as u8;
    Rgb {
        r: shade(color.r),
        g: shade(color.g),
        b: shade(color.b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_colors_cover_the_cube_and_gray_ramp() {
        let palette = Colors::default();
        assert_eq!(reported(21, &palette), Rgb { r: 0, g: 0, b: 255 });
        assert_eq!(reported(232, &palette), Rgb { r: 8, g: 8, b: 8 });
        assert_eq!(
            reported(255, &palette),
            Rgb {
                r: 238,
                g: 238,
                b: 238
            }
        );
    }

    #[test]
    fn shell_palette_updates_are_visible_when_read_back() {
        let mut palette = Colors::default();
        palette[NamedColor::Background] = Some(Rgb { r: 7, g: 13, b: 31 });
        assert_eq!(reported(257, &palette), Rgb { r: 7, g: 13, b: 31 });
        assert_eq!(
            resolve(Color::Named(NamedColor::Background), &palette),
            Rgb { r: 7, g: 13, b: 31 }
        );
    }
}
