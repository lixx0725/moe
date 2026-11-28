//! 将原生键盘事件转成剪贴板或终端输入操作。

use alacritty_terminal::term::TermMode;
use gpui::Keystroke;

use super::InteractionContext;
use crate::terminal::input::{self, TerminalKey};

/// 终端视图可以执行的键盘操作。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum KeyboardInput {
    /// 读取系统剪贴板。
    Paste {
        /// 是否使用括号粘贴协议。
        bracketed: bool,
    },
    /// 向 PTY 写入已编码的字节。
    Write(Vec<u8>),
}

/// 决定是否接收按键，其他按键继续交给原生输入路径。
pub(crate) fn route_key(stroke: &Keystroke, context: &InteractionContext) -> Option<KeyboardInput> {
    if !context.focused || context.composing {
        return None;
    }
    let modifiers = stroke.modifiers;
    let is_paste = stroke.key == "v"
        && if cfg!(target_os = "macos") {
            modifiers.platform && !modifiers.control && !modifiers.alt
        } else {
            modifiers.control && modifiers.shift && !modifiers.alt && !modifiers.platform
        };
    if is_paste {
        return Some(KeyboardInput::Paste {
            bracketed: context.mode.contains(TermMode::BRACKETED_PASTE),
        });
    }
    if modifiers.platform {
        return None;
    }
    let key = TerminalKey {
        name: &stroke.key,
        text: stroke.key_char.as_deref(),
        shift: modifiers.shift,
        alt: modifiers.alt,
        control: modifiers.control,
    };
    input::encode(&key, context.mode.contains(TermMode::APP_CURSOR)).map(KeyboardInput::Write)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_cursor_keys_and_interrupt_for_the_pty() {
        let context = InteractionContext {
            focused: true,
            composing: false,
            mode: TermMode::APP_CURSOR,
        };
        let input = |key| route_key(&Keystroke::parse(key).unwrap(), &context);
        assert_eq!(input("up"), Some(KeyboardInput::Write(b"\x1bOA".to_vec())));
        assert_eq!(input("ctrl-c"), Some(KeyboardInput::Write(vec![3])));
        assert_eq!(input("a"), None);
    }

    #[test]
    fn focus_and_composition_own_keyboard_input() {
        let key = Keystroke::parse("enter").unwrap();
        let mut context = InteractionContext {
            focused: false,
            composing: false,
            mode: TermMode::empty(),
        };
        assert_eq!(route_key(&key, &context), None);
        context.focused = true;
        context.composing = true;
        assert_eq!(route_key(&key, &context), None);
        context.composing = false;
        assert_eq!(
            route_key(&key, &context),
            Some(KeyboardInput::Write(vec![b'\r']))
        );
    }

    #[test]
    fn platform_paste_preserves_terminal_bracketed_mode() {
        let paste = if cfg!(target_os = "macos") {
            "cmd-v"
        } else {
            "ctrl-shift-v"
        };
        let context = InteractionContext {
            focused: true,
            composing: false,
            mode: TermMode::BRACKETED_PASTE,
        };
        assert_eq!(
            route_key(&Keystroke::parse(paste).unwrap(), &context),
            Some(KeyboardInput::Paste { bracketed: true })
        );
    }
}
