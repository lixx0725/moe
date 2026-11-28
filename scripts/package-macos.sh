#!/usr/bin/env bash
set -euo pipefail

cd "$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"
# shellcheck source=scripts/lib/package.sh
source scripts/lib/package.sh

if [[ $# == 1 && $1 == --help ]]; then
    printf '%s\n' '用法：bash scripts/package-macos.sh'
    exit 0
fi
[[ $# == 0 ]] || package_error '用法：bash scripts/package-macos.sh'
[[ $(uname -s) == Darwin && $(uname -m) == arm64 ]] ||
    package_error '请在 Apple Silicon macOS 的原生终端运行此脚本。'
bash scripts/tools/setup-macos.sh --check
require_tools cargo rustc codesign hdiutil shasum lipo

target=aarch64-apple-darwin
[[ $(rustc -vV | sed -n 's/^host: //p') == "$target" ]] ||
    package_error '需要原生 arm64 Rust 工具链。'
build_package_binary macos arm64 "$target"
lipo -verify_arch arm64 "$package_binary"

app="$package_stage/image/Moe.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
install -m755 "$package_binary" "$app/Contents/MacOS/moe"
install -m644 packaging/macos/Info.plist "$app/Contents/Info.plist"
install -m644 packaging/macos/moe.icns "$app/Contents/Resources/moe.icns"
install -m644 LICENSE "$app/Contents/Resources/LICENSE"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $package_version" \
    -c "Set :CFBundleVersion $package_version" "$app/Contents/Info.plist"

# 应用包内容就位后签名，DMG 内便可校验完整的应用。
codesign --force --sign - "$app"
codesign --verify --strict "$app"
ln -s /Applications "$package_stage/image/Applications"
artifact="moe-$package_version-macos-arm64.dmg"
hdiutil create -fs HFS+ -format UDZO -volname Moe -srcfolder "$package_stage/image" \
    "$package_stage/$artifact"
publish_package "$artifact" shasum
