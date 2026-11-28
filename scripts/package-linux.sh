#!/usr/bin/env bash
set -euo pipefail

cd "$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"
# shellcheck source=scripts/lib/package.sh
source scripts/lib/package.sh

if [[ $# == 1 && $1 == --help ]]; then
    printf '%s\n' '用法：bash scripts/package-linux.sh --format deb|tar.gz'
    exit 0
fi
if [[ $# != 2 || $1 != --format || ( $2 != deb && $2 != tar.gz ) ]]; then
    package_error '请使用 --format deb 或 --format tar.gz。'
fi
format=$2
[[ $(uname -s) == Linux ]] || package_error '请在目标 Linux 环境运行此脚本。'

require_tools cargo rustc pkg-config cc sha256sum
target=$(rustc -vV | sed -n 's/^host: //p')
case $target in
    x86_64-unknown-linux-gnu) architecture=x64 ;;
    aarch64-unknown-linux-gnu) architecture=arm64 ;;
    *) package_error "当前仅支持本机 GNU/Linux x64 或 arm64 构建：$target" ;;
esac
if [[ $format == deb ]]; then
    require_tools dpkg dpkg-deb dpkg-shlibdeps
    [[ $architecture == x64 && $(dpkg --print-architecture) == amd64 ]] ||
        package_error 'DEB 目前面向 amd64。可使用 --format tar.gz。'
else
    require_tools tar
fi
pkg-config --exists fontconfig freetype2 wayland-client x11 x11-xcb xcb xkbcommon xkbcommon-x11 ||
    package_error '缺少图形系统的开发包。请按 docs/building.md 安装依赖。'

build_package_binary linux "$architecture" "$target"
if [[ $format == tar.gz ]]; then
    portable_name="moe-$package_version-linux-$architecture"
    install -Dm755 "$package_binary" "$package_stage/$portable_name/moe"
    install -m644 LICENSE "$package_stage/$portable_name/LICENSE"
    artifact="$portable_name.tar.gz"
    tar -C "$package_stage" -czf "$package_stage/$artifact" "$portable_name"
else
    package_root="$package_stage/deb"
    artifact="moe_${package_version}_amd64.deb"
    install -Dm755 "$package_binary" "$package_root/usr/bin/moe"
    install -Dm644 packaging/linux/moe.desktop "$package_root/usr/share/applications/moe.desktop"
    install -Dm644 packaging/linux/moe.png "$package_root/usr/share/icons/hicolor/128x128/apps/moe.png"
    install -Dm644 LICENSE "$package_root/usr/share/doc/moe/copyright"
    mkdir -p "$package_root/DEBIAN" "$package_stage/debian"
    printf 'Source: moe\n\nPackage: moe\nArchitecture: any\n' > "$package_stage/debian/control"
    # ELF 中的共享库约束由 dpkg 计算，再声明图形后端按需加载的运行库。
    dependencies=$(cd "$package_stage" && dpkg-shlibdeps -O -e"$package_root/usr/bin/moe") ||
        package_error '无法读取系统库包的依赖信息。请在已安装运行库的 Debian／Ubuntu 环境打包。'
    dependencies=${dependencies#shlibs:Depends=}
    [[ -n $dependencies ]] || package_error '无法计算 DEB 的运行时依赖。'
    cat > "$package_root/DEBIAN/control" <<CONTROL
Package: moe
Version: $package_version
Section: x11
Priority: optional
Architecture: amd64
Maintainer: Moe contributors <lixx0725@users.noreply.github.com>
Depends: $dependencies, libfontconfig1, libwayland-client0, libvulkan1, fonts-dejavu-core
Recommends: mesa-vulkan-drivers
Homepage: https://github.com/lixx0725/moe
Description: Native terminal emulator
 Moe opens a native window for the local shell with a PTY.
CONTROL
    dpkg-deb --root-owner-group --build "$package_root" "$package_stage/$artifact"
fi

publish_package "$artifact"
