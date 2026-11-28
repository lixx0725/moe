# 从源码构建安装包

下载源码 ZIP 或检出仓库。在目标系统准备构建工具，解压后从源码根目录运行对应命令。Rustup 根据 `rust-toolchain.toml` 选择编译器，Cargo 依照 `Cargo.lock` 构建。安装包和 `SHA256SUMS.txt` 生成到 `dist/<版本>/<平台>-<架构>/`。版本来自当前源码。

| 目标 | 命令 | 产物 |
| --- | --- | --- |
| Windows 11 x64 | `.\scripts\package-windows.ps1` | `moe-<版本>-windows-x64-setup.exe` |
| macOS Apple Silicon | `bash scripts/package-macos.sh` | `moe-<版本>-macos-arm64.dmg` |
| Debian／Ubuntu amd64 | `bash scripts/package-linux.sh --format deb` | `moe_<版本>_amd64.deb` |
| GNU/Linux x64 或 arm64 | `bash scripts/package-linux.sh --format tar.gz` | `moe-<版本>-linux-<架构>.tar.gz` |

## Windows x64

准备 PowerShell 7.2 或更新版、Rustup 的 MSVC 工具链、Visual Studio Build Tools 的桌面 C++ 工作负载和含 `fxc.exe` 的 Windows SDK。然后从源码根目录执行：

```powershell
pwsh -File .\scripts\package-windows.ps1
```

打包脚本会调用 `scripts/tools/inno-tools.ps1`，查找缓存或系统中已有的 Inno Setup 6.3 或更新版。第一次找不到编译器时，脚本下载并校验固定版本的官方安装程序，静默准备用户级便携版，然后继续打包。工具缓存在 `%LOCALAPPDATA%\moe-build-tools`，删除项目的 `target` 或运行 `cargo clean` 后仍可复用。

已有编译器也可以显式指定：

```powershell
.\scripts\package-windows.ps1 -IsccPath 'C:\Tools\Inno Setup\ISCC.exe'
```

双击生成的安装包，按当前用户安装。Moe 会出现在开始菜单和「已安装的应用」。开始菜单调用图形入口 `moe.exe`，只打开 Moe 窗口。安装时可选将目录加入用户 PATH，之后在新的 CMD 或 pwsh 中通过 `moe.com` 执行 `moe -h`、`moe --version` 或 `moe --shell pwsh`。

需要升级时，用新版源码打包。关闭所有 Moe 窗口后运行新版安装器，确认版本和更新说明。安装器沿用已有安装目录和上一次的 PATH 选项。相同版本会提示已安装，低版本包无法覆盖高版本。从「已安装的应用」卸载会移除程序、快捷方式和安装器添加的 PATH 条目，用户数据保留。

## macOS Apple Silicon

准备原生 arm64 Rustup。运行 `bash scripts/tools/setup-macos.sh` 检查 Xcode 命令行工具，缺少时会打开 macOS 自带的安装向导。向导完成后再运行打包脚本。打包使用系统的 `codesign` 和 `hdiutil`，为本地构建的应用创建临时签名。

打开 DMG，把 `Moe.app` 拖入「应用程序」。升级时先退出 Moe，再用新版 DMG 中的 App 替换旧 App。卸载时将 `Moe.app` 移到废纸篓。

## Debian／Ubuntu 和 GNU/Linux

在 Debian 或 Ubuntu 上运行仓库里的工具准备脚本，用 APT 安装编译、图形和 DEB 打包依赖：

```sh
bash scripts/tools/setup-linux.sh
```

在生成包的系统或相容的 Debian／Ubuntu 环境中使用包管理器安装：

```sh
sudo apt install ./dist/0.0.4/linux-x64/moe_0.0.4_amd64.deb
moe --version
```

覆盖升级时安装更高版本的同名包。卸载用 `sudo apt remove moe`。程序安装到 `/usr/bin/moe`，系统菜单项和图标随包管理。

其他 GNU/Linux 发行版可安装本机对应的开发包和运行库，然后生成 `tar.gz`。解压到所需目录，运行其中的 `moe`。便携包使用构建环境的系统库，请在目标环境构建。终端图形界面需要可用的 X11 或 Wayland、Vulkan 驱动和系统等宽字体。

## 验证与数据

- Windows 可用 `Get-FileHash -Algorithm SHA256 <安装包>` 对照 `SHA256SUMS.txt`。macOS 在产物目录运行 `shasum -a 256 -c SHA256SUMS.txt`，Linux 运行 `sha256sum -c SHA256SUMS.txt`。
- [Package 工作流](../.github/workflows/package.yml) 从源码 ZIP 开始，覆盖 Windows x64、macOS arm64 和 Ubuntu 24.04 amd64。它调用同一套本地打包脚本，并验证可自动执行的安装、升级、卸载和启动步骤。
- 本机打包可使用平台的本地签名方式和工具，无需购买发布者证书、打 tag 或创建 GitHub Release。
- 安装器保留用户数据。未来的数据格式如果需要手动重置，会在对应版本的更新说明里标出具体文件、目录和操作时机。
