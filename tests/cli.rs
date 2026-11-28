//! 通过真实二进制验证命令行入口。

use std::process::Command;

/// 创建无需显示服务器的命令行测试进程。
fn moe() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_moe"));
    command.env_remove("DISPLAY").env_remove("WAYLAND_DISPLAY");
    command
}

#[test]
fn version_succeeds_without_a_display() {
    let output = moe().arg("--version").output().expect("运行 moe --version");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).expect("版本输出为 UTF-8"),
        concat!("moe ", env!("CARGO_PKG_VERSION"), "\n")
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn help_describes_shell_selection_without_a_display() {
    let output = moe().arg("--help").output().expect("运行 moe --help");
    assert!(output.status.success(), "{output:?}");
    let help = String::from_utf8(output.stdout).expect("帮助输出为 UTF-8");
    assert!(help.contains("--shell"), "{help}");
}

#[test]
fn unknown_option_reports_cli_error_before_launch() {
    let output = moe()
        .arg("--moe-unknown-option")
        .output()
        .expect("运行带未知参数的 moe");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let error = String::from_utf8(output.stderr).expect("诊断输出为 UTF-8");
    assert!(error.contains("--moe-unknown-option"), "{error}");
}

#[test]
fn missing_explicit_shell_reports_its_path_without_falling_back() {
    let directory = tempfile::tempdir().expect("创建空的 Shell 查找目录");
    let missing = directory.path().join("missing-moe-shell.exe");
    let output = moe()
        .arg("--shell")
        .arg(&missing)
        .output()
        .expect("运行带无效 Shell 路径的 moe");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let error = String::from_utf8(output.stderr).expect("诊断输出为 UTF-8");
    assert!(error.contains("cannot use shell"), "{error}");
    assert!(error.contains("missing-moe-shell.exe"), "{error}");
}
