//! 通过真实二进制验证命令行入口。

use std::process::Command;

#[test]
fn version_succeeds_without_a_display() {
    let output = Command::new(env!("CARGO_BIN_EXE_moe"))
        .arg("--version")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .expect("运行 moe --version");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).expect("版本输出为 UTF-8"),
        concat!("moe ", env!("CARGO_PKG_VERSION"), "\n")
    );
    assert!(output.stderr.is_empty());
}
