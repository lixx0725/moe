# moe

moe 是使用 Rust 构建的原生终端模拟器，面向 Windows、Linux 和 macOS。打开一个窗口即可使用本机 Shell 执行命令。终端支持基本 ANSI 输出、键盘及输入法输入、粘贴、窗口尺寸同步和会话退出。

终端使用平台等宽字体。Windows 使用 Consolas，macOS 使用 Menlo，Linux 使用 DejaVu Sans Mono。

## 运行

在项目目录执行：

```text
cargo run --locked
```

使用 `cargo build --locked` 构建后，可直接运行 `target/debug/moe`。Windows 的可执行文件为 `target\debug\moe.exe`。Shell 从当前目录启动。执行 `exit` 或关闭窗口会结束会话。使用 `moe --version` 可在无图形环境下查询版本。

| 平台 | 构建依赖 | 默认 Shell |
| --- | --- | --- |
| Windows | MSVC C++ 工具和含 `fxc.exe` 的 Windows SDK | 优先 PowerShell 7，随后使用 Windows PowerShell |
| Linux | C/C++ 编译工具、pkg-config、Fontconfig、FreeType、X11、XCB、XKB 和 Wayland 开发包 | 当前账户的 Shell，回退到 `/bin/sh` |
| macOS | Xcode 构建工具 | 当前账户的 Shell，依次回退到 `/bin/zsh` 和 `/bin/sh` |

Ubuntu 使用的具体开发包见 [CI 配置](.github/workflows/ci.yml)。Rust 工具链以 `rust-toolchain.toml` 为准。

## 开发和验证

```text
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

真实 PTY 验收覆盖默认 Shell、Unicode、实际行列数、`Ctrl+C` 后继续使用、自然退出和关闭清理，可以单独运行：

```text
cargo test --locked session::tests::pty_ -- --test-threads=1
```

图形验收时检查中文输入法、窗口缩放、字体对齐及运行命令时关窗。可用 `stty size` 或 PowerShell 的 `[Console]::WindowHeight` 与 `[Console]::WindowWidth` 检查 Shell 看到的尺寸。

CI 默认使用 Windows、Linux 和 macOS 的托管 runner。在 `main` 手动运行 **CI** 时，可以选一个已注册的自托管操作系统。自托管机器需提前准备相应的构建依赖。

## 许可

moe 的自有源码采用 [BSD Zero Clause License（0BSD）](LICENSE)。依赖包按各自许可证使用。
