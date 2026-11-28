#!/usr/bin/env bash
set -euo pipefail

# Debian／Ubuntu 的打包依赖由本机包管理器安装，供本地和 CI 共用。
if [[ $# == 1 && $1 == --help ]]; then
    printf '%s\n' '用法：bash scripts/tools/setup-linux.sh [--yes]'
    exit 0
fi
if [[ $# -gt 1 || ( $# == 1 && $1 != --yes ) ]]; then
    printf '%s\n' '用法：bash scripts/tools/setup-linux.sh [--yes]' >&2
    exit 2
fi
if [[ $(uname -s) != Linux ]] || ! command -v apt-get >/dev/null 2>&1; then
    printf '%s\n' '此脚本面向使用 APT 的 Debian／Ubuntu。其他发行版请按 docs/building.md 准备系统库。' >&2
    exit 1
fi

apt=(apt-get)
if [[ $EUID != 0 ]]; then
    command -v sudo >/dev/null 2>&1 || {
        printf '%s\n' '需要 sudo 或 root 权限来安装系统构建依赖。' >&2
        exit 1
    }
    apt=(sudo apt-get)
fi

packages=(
    build-essential pkg-config dpkg-dev
    libfontconfig1-dev libfreetype-dev libwayland-dev
    libx11-dev libx11-xcb-dev libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev
)
install_options=(install)
if [[ ${1-} == --yes ]]; then
    install_options+=(--yes)
fi

"${apt[@]}" update
"${apt[@]}" "${install_options[@]}" "${packages[@]}"
