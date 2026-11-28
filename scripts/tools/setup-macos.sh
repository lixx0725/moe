#!/usr/bin/env bash
set -euo pipefail

# 普通模式打开系统工具安装向导，打包时只确认工具已准备好。
if [[ $# == 1 && $1 == --help ]]; then
    printf '%s\n' '用法：bash scripts/tools/setup-macos.sh [--check]'
    exit 0
fi
if [[ $# -gt 1 || ( $# == 1 && $1 != --check ) ]]; then
    printf '%s\n' '用法：bash scripts/tools/setup-macos.sh [--check]' >&2
    exit 2
fi
if [[ $(uname -s) != Darwin ]]; then
    printf '%s\n' '请在 macOS 上准备 Xcode 命令行工具。' >&2
    exit 1
fi

if xcode-select -p >/dev/null 2>&1 && xcrun --find clang >/dev/null 2>&1; then
    printf '%s\n' 'Xcode 命令行工具已就绪。'
    exit 0
fi
if [[ ${1-} == --check ]]; then
    printf '%s\n' '请先运行 bash scripts/tools/setup-macos.sh 并完成系统安装向导。' >&2
    exit 1
fi

xcode-select --install
printf '%s\n' '请完成系统安装向导，再运行打包脚本。'
