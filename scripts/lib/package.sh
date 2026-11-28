# macOS 和 Linux 的共享打包步骤。入口负责检查本机目标和格式依赖。

package_error() {
    printf '打包失败：%s\n' "$*" >&2
    exit 1
}

require_tools() {
    local tool
    for tool in "$@"; do
        command -v "$tool" >/dev/null 2>&1 ||
            package_error "找不到 $tool。请按 docs/building.md 准备构建依赖。"
    done
}

# 从 Cargo 的锁定包身份读取版本，检出目录和源码归档走同一路径。
read_package_version() {
    local package_id
    package_id=$(cargo pkgid --locked) || package_error '无法读取 Cargo 包版本。'
    package_version=${package_id##*#}
    package_version=${package_version##*@}
    [[ $package_version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
        package_error "需要形如 0.0.4 的版本，当前值：$package_version"
}

# 显式指定本机目标，避免把 Cargo 配置里的其他目标误打成本机包。
build_package_binary() {
    local build_dir version_line
    local platform=$1 architecture=$2 target=$3
    read_package_version
    build_dir=${CARGO_TARGET_DIR:-"$PWD/target"}
    package_output="$PWD/dist/$package_version/$platform-$architecture"

    cargo build --release --locked --target "$target" --target-dir "$build_dir" ||
        package_error 'Cargo Release 构建未完成。'
    package_binary="$build_dir/$target/release/moe"
    version_line=$("$package_binary" --version) || package_error '无法运行构建出的 moe。'
    [[ $version_line == "moe $package_version" ]] || package_error "二进制版本不匹配：$version_line"

    mkdir -p "$package_output"
    package_stage=$(mktemp -d "$package_output/.package.XXXXXX")
    trap 'rm -rf -- "$package_stage"' EXIT
}

# 清单路径相对于产物目录，重打 Linux 的单个格式时继续校验其他格式。
publish_package() (
    local artifact=$1 checksum=${2:-sha256sum}
    local asset lock="$package_output/.package-lock"
    mkdir "$lock" ||
        package_error "产物目录正在写入。确认没有脚本运行后，可移除 $lock 再重试。"
    trap 'rmdir "$lock"' EXIT
    mv -f "$package_stage/$artifact" "$package_output/$artifact"
    (
        cd "$package_output"
        for asset in moe-*.dmg moe-*.tar.gz moe_*.deb; do
            [[ -f $asset ]] || continue
            if [[ $checksum == shasum ]]; then
                shasum -a 256 "$asset" || exit
            else
                sha256sum "$asset" || exit
            fi
        done
    ) > "$package_stage/SHA256SUMS.txt"
    mv -f "$package_stage/SHA256SUMS.txt" "$package_output/SHA256SUMS.txt"
    printf '%s\n' "$package_output/$artifact" "$package_output/SHA256SUMS.txt"
)
