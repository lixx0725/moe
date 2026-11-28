//! 提供平台系统查询和终端字体选择。

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(crate) use unix::account_shell;

/// 返回当前平台的终端等宽字体族名。
pub(crate) fn terminal_font() -> &'static str {
    if cfg!(windows) {
        "Consolas"
    } else if cfg!(target_os = "macos") {
        "Menlo"
    } else {
        "DejaVu Sans Mono"
    }
}
