#Requires -Version 7.2

$moeInnoVersion = '7.1.1'
$moeInnoUri = 'https://github.com/jrsoftware/issrc/releases/download/is-7_1_1/innosetup-7.1.1-x64.exe'
$moeInnoSha256 = '8A5A6711C3766A6E5FB74F280BA0E3551E719CE94538D901FDBD1F8F9920B3F0'

# Inno Setup 7 的 PE 版本字段可能全为零，优先查询可运行的编译引擎。
function Test-MoeInnoCompiler([string] $Path) {
    if (-not $Path -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $false
    }

    try {
        $output = (& $Path --version 2>&1 | Out-String).Trim()
        $reported = $null
        if ($LASTEXITCODE -eq 0 -and [version]::TryParse($output, [ref] $reported)) {
            return ($reported -ge [version]'6.3')
        }
        Write-Verbose "无法读取 Inno 编译引擎版本：$Path（退出码 $LASTEXITCODE）"
    } catch {
        Write-Verbose "无法查询 $Path 的编译引擎：$($_.Exception.Message)"
    }

    # Inno Setup 6 没有 --version。仅这条旧版路径使用 PE 文件版本。
    try {
        $info = (Get-Item -LiteralPath $Path).VersionInfo
        return ($info.FileMajorPart -eq 6 -and $info.FileMinorPart -ge 3)
    } catch {
        Write-Verbose "无法读取 $Path 的文件版本：$($_.Exception.Message)"
        return $false
    }
}

# 注册表覆盖自定义安装目录。PATH 和默认目录也可以提供编译器。
function Get-MoeInnoCandidates {
    Get-Command ISCC.exe -CommandType Application -All -ErrorAction SilentlyContinue |
        ForEach-Object { $_.Source }

    $uninstallRoots = @(
        'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
        'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
        'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall'
    )
    $names = @('Inno Setup 7', 'Inno Setup 7 (32-bit)', 'Inno Setup 6')
    foreach ($registryRoot in $uninstallRoots) {
        foreach ($name in $names) {
            $key = Join-Path $registryRoot "${name}_is1"
            $directory = Get-ItemPropertyValue -LiteralPath $key -Name InstallLocation -ErrorAction SilentlyContinue
            if ($directory) { Join-Path $directory 'ISCC.exe' }
        }
    }

    foreach ($root in @($env:ProgramW6432, $env:ProgramFiles, ${env:ProgramFiles(x86)}, "$env:LOCALAPPDATA\Programs")) {
        if ($root) {
            foreach ($name in $names) { Join-Path $root "$name\ISCC.exe" }
        }
    }
}

# 校验完成后才替换下载缓存。中断或校验失败时清理临时文件。
function Get-MoeInnoInstaller([string] $Destination) {
    if ((Test-Path -LiteralPath $Destination -PathType Leaf) -and
        (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash -eq $moeInnoSha256) {
        return $Destination
    }

    $partial = "$Destination.$([guid]::NewGuid().ToString('N')).download"
    try {
        Invoke-WebRequest -Uri $moeInnoUri -OutFile $partial -TimeoutSec 60 -MaximumRetryCount 2 -RetryIntervalSec 2
        if ((Get-FileHash -LiteralPath $partial -Algorithm SHA256).Hash -ne $moeInnoSha256) {
            throw '官方 Inno Setup 安装程序的 SHA-256 与固定值不符。'
        }
        [IO.File]::Move($partial, $Destination, $true)
    } catch {
        throw "无法准备 Inno Setup $moeInnoVersion。可从 https://jrsoftware.org/inno-setup/downloads 获取工具并用 -IsccPath 指定编译器。`n$($_.Exception.Message)"
    } finally {
        if (Test-Path -LiteralPath $partial) { Remove-Item -LiteralPath $partial -Force }
    }
    return $Destination
}

# 便携版先装入临时目录，验证通过后才进入供下次重用的缓存。
function Resolve-MoeInnoCompiler([string] $IsccPath, [switch] $Unattended) {
    if ($IsccPath) {
        $path = (Resolve-Path -LiteralPath $IsccPath).ProviderPath
        if (-not (Test-MoeInnoCompiler $path)) {
            throw "-IsccPath 需要指向 Inno Setup 6.3 或更新版的有效 ISCC.exe：$path"
        }
        return $path
    }

    # 工具缓存独立于源码目录和应用数据，清理 Cargo 的 target 后仍可复用。
    if (-not $env:LOCALAPPDATA) {
        throw '无法定位当前用户的 LOCALAPPDATA。可用 -IsccPath 指定已有编译器。'
    }
    $cache = Join-Path $env:LOCALAPPDATA 'moe-build-tools'
    $directory = Join-Path $cache "inno-$moeInnoVersion"
    $compiler = Join-Path $directory 'ISCC.exe'
    if (Test-MoeInnoCompiler $compiler) { return $compiler }
    if (-not $Unattended) {
        foreach ($path in (Get-MoeInnoCandidates | Select-Object -Unique)) {
            if (Test-MoeInnoCompiler $path) { return $path }
        }
    }

    New-Item -Path $cache -ItemType Directory -Force | Out-Null
    $lockPath = Join-Path $cache ".inno-$moeInnoVersion.lock"
    try {
        $lock = [IO.File]::Open($lockPath, [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    } catch [IO.IOException] {
        throw "无法锁定 Inno Setup 缓存：$lockPath。若另一个打包进程正在准备工具，请等它结束后重试。"
    }

    $stage = Join-Path $cache ('.inno-' + [guid]::NewGuid().ToString('N'))
    try {
        if (Test-MoeInnoCompiler $compiler) { return $compiler }
        $archive = Join-Path $cache "innosetup-$moeInnoVersion-x64.exe"
        Write-Host "准备便携版 Inno Setup $moeInnoVersion。"
        $installer = Get-MoeInnoInstaller $archive
        # 本地和 CI 都静默准备用户级便携工具，保留日志供失败时诊断。
        $log = Join-Path $cache "inno-$moeInnoVersion-setup.log"
        $arguments = @(
            '/PORTABLE=1', '/CURRENTUSER', '/NORESTART', '/NOICONS',
            '/SP-', '/VERYSILENT', '/SUPPRESSMSGBOXES', ('/DIR="{0}"' -f $stage), ('/LOG="{0}"' -f $log)
        )
        $process = Start-Process -FilePath $installer -ArgumentList $arguments -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "Inno Setup 安装程序退出码为 $($process.ExitCode)。日志：$log"
        }
        $stageCompiler = Join-Path $stage 'ISCC.exe'
        if (-not (Test-Path -LiteralPath $stageCompiler -PathType Leaf)) {
            throw "Inno Setup 安装程序已成功退出，但没有生成 $stageCompiler。日志：$log"
        }
        if (-not (Test-MoeInnoCompiler $stageCompiler)) {
            throw "已找到 $stageCompiler，但无法验证编译引擎版本。可用 -Verbose 查看详情。日志：$log"
        }

        if (Test-Path -LiteralPath $directory) {
            # 保留以前未完成的目录，避免移除用户放入缓存目录的其他文件。
            Move-Item -LiteralPath $directory -Destination "$directory.incomplete-$([guid]::NewGuid().ToString('N'))"
        }
        Move-Item -LiteralPath $stage -Destination $directory
        return $compiler
    } finally {
        try {
            if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
        } finally {
            $lock.Dispose()
        }
    }
}
