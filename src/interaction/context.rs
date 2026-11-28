//! 描述一次输入发生时的交互状态。

use alacritty_terminal::term::TermMode;

/// 当前输入的焦点、输入法及终端协议状态。
pub(crate) struct InteractionContext {
    /// 当前终端窗格拥有键盘焦点。
    pub(crate) focused: bool,
    /// 原生输入法仍在组合文字。
    pub(crate) composing: bool,
    /// 当前终端模式，用于应用光标和括号粘贴。
    pub(crate) mode: TermMode,
}
