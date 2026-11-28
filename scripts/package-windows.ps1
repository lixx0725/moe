#Requires -Version 7.2
[CmdletBinding()]
param(
    [string] $IsccPath,
    [switch] $Unattended
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'tools/inno-tools.ps1')

if (-not $IsWindows -or [Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') {
    throw '请在 Windows x64 环境运行此脚本。'
}
foreach ($tool in @('cargo.exe', 'rustc.exe')) {
    if (-not (Get-Command $tool -CommandType Application -ErrorAction SilentlyContinue)) {
        throw "找不到 $tool。请按 docs/building.md 安装构建依赖。"
    }
}
$root = Split-Path -Parent $PSScriptRoot
$iscc = Resolve-MoeInnoCompiler -IsccPath $IsccPath -Unattended:$Unattended
Write-Verbose "使用 Inno Setup 编译器：$iscc"
$target = 'x86_64-pc-windows-msvc'
$stage = $null
Push-Location -LiteralPath $root
try {
    $compiler = & rustc -vV
    if ($LASTEXITCODE -ne 0 -or $compiler -notcontains "host: $target") {
        throw '需要原生 x64 MSVC Rust 工具链。'
    }
    $packageId = & cargo pkgid --locked
    if ($LASTEXITCODE -ne 0) {
        throw '无法读取锁定的 Cargo 包版本。'
    }
    $version = ($packageId -split '[#@]')[-1]
    if ($version -notmatch '^\d+\.\d+\.\d+$' -or
        ($version.Split('.') | Where-Object { [decimal]$_ -gt 65535 })) {
        throw "安装包需要三个不大于 65535 的版本段，当前值：$version"
    }

    $targetDir = Join-Path $root 'target'
    if ($env:CARGO_TARGET_DIR) {
        $targetDir = [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR, $root)
    }
    & cargo build --release --locked --target $target --target-dir $targetDir --bin moe --bin moe-cli
    if ($LASTEXITCODE -ne 0) {
        throw 'Cargo Release 构建未完成。请检查 MSVC C++ 工具和 Windows SDK。'
    }
    $binary = Join-Path $targetDir "$target\release\moe.exe"
    $launcher = Join-Path $targetDir "$target\release\moe-cli.exe"
    $versionLine = & $launcher --version
    if ($LASTEXITCODE -ne 0 -or $versionLine -cne "moe $version") {
        throw "Windows 命令行入口报告的版本不匹配：$versionLine"
    }

    $output = Join-Path $root "dist\$version\windows-x64"
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    # 在同一个产物目录内构建，成功后才替换安装包与校验文件。
    $stage = Join-Path $output ('.package-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $stage | Out-Null
    $setupScript = Join-Path $root 'packaging\windows\moe.iss'
    & $iscc "/DAppVersion=$version" "/DMoeExe=$binary" "/DMoeCli=$launcher" "/DOutputDir=$stage" $setupScript
    if ($LASTEXITCODE -ne 0) {
        throw "Inno Setup 编译失败，退出码为 $LASTEXITCODE。"
    }
    $name = "moe-$version-windows-x64-setup.exe"
    $installer = Join-Path $stage $name
    $hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
    $sums = Join-Path $stage 'SHA256SUMS.txt'
    Set-Content -LiteralPath $sums -Value "$hash  $name" -Encoding ascii
    # 同一版本的包和校验文件按顺序发布，避免并行打包互相覆盖。
    $key = [Text.Encoding]::UTF8.GetBytes($output.ToUpperInvariant())
    $keyHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($key))
    $publisher = [Threading.Mutex]::new($false, "Local\moe-package-$keyHash")
    $ownsMutex = $false
    try {
        try {
            $ownsMutex = $publisher.WaitOne([TimeSpan]::FromSeconds(30))
        } catch [Threading.AbandonedMutexException] {
            $ownsMutex = $true
        }
        if (-not $ownsMutex) { throw "产物目录正在写入：$output" }
        Move-Item -LiteralPath $installer, $sums -Destination $output -Force
    } finally {
        if ($ownsMutex) { $publisher.ReleaseMutex() }
        $publisher.Dispose()
    }
    Write-Output (Join-Path $output $name)
    Write-Output (Join-Path $output 'SHA256SUMS.txt')
} finally {
    if ($stage -and (Test-Path -LiteralPath $stage)) {
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
    Pop-Location
}
