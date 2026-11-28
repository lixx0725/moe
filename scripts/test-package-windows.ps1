#Requires -Version 7.2
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $Package,
    [Parameter(Mandatory)] [string] $UpgradePackage,
    [Parameter(Mandatory)] [string] $Version,
    [Parameter(Mandatory)] [string] $UpgradeVersion
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\io.github.lixx0725.moe_is1'
if (Test-Path -LiteralPath $uninstallKey) {
    throw '安装测试需要该账户没有安装 Moe，请使用干净的 Windows runner 或测试账户。'
}

# 用含空格的目录覆盖静默安装参数和路径记录。
$testRoot = Join-Path $env:TEMP ('Moe install test ' + [guid]::NewGuid().ToString('N'))
$installDir = Join-Path $testRoot 'app'
$dataDir = Join-Path $env:LOCALAPPDATA 'moe'
$marker = Join-Path $dataDir ([guid]::NewGuid().ToString('N') + '.package-test')
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Moe.lnk'
$Package = (Resolve-Path -LiteralPath $Package).Path
$UpgradePackage = (Resolve-Path -LiteralPath $UpgradePackage).Path

function Invoke-SetupTest {
    param([string] $Executable, [string[]] $Options = @())
    $log = Join-Path $testRoot ([guid]::NewGuid().ToString('N') + '.log')
    $arguments = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', ('/LOG="{0}"' -f $log)) + $Options
    $result = Start-Process -FilePath $Executable -ArgumentList $arguments -Wait -PassThru
    if ($result.ExitCode -ne 0 -and (Test-Path -LiteralPath $log)) {
        Get-Content -LiteralPath $log -Tail 35 | Write-Host
    }
    return $result.ExitCode
}

function Test-UserPathEntry {
    $key = Get-Item -LiteralPath 'HKCU:\Environment'
    try {
        $value = $key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        return ($value -split ';') -contains $installDir
    } finally {
        $key.Dispose()
    }
}

function Assert-PeSubsystem {
    param([string] $Path, [int] $Subsystem)
    $reader = [IO.BinaryReader]::new([IO.File]::OpenRead($Path))
    try {
        # PE32+ 可选头中的 Subsystem 区分 GUI（2）和控制台（3）。
        $reader.BaseStream.Position = 0x3C
        $header = $reader.ReadUInt32()
        $reader.BaseStream.Position = $header
        if ($reader.ReadUInt32() -ne 0x4550 -or $reader.ReadUInt16() -ne 0x8664) {
            throw "安装文件不是 x64 PE：$Path"
        }
        $reader.BaseStream.Position = $header + 24
        if ($reader.ReadUInt16() -ne 0x20B) {
            throw "安装文件没有 PE32+ 头：$Path"
        }
        $reader.BaseStream.Position = $header + 24 + 68
        if ($reader.ReadUInt16() -ne $Subsystem) {
            throw "安装入口的 Windows 子系统不匹配：$Path"
        }
    } finally {
        $reader.Dispose()
    }
}

function Assert-InstalledVersion {
    param([string] $Expected)
    $record = Get-ItemProperty -LiteralPath $uninstallKey
    if ($record.DisplayVersion -ne $Expected -or $record.InstallLocation.TrimEnd('\') -ne $installDir) {
        throw '系统记录的安装版本或安装目录不匹配。'
    }
    $installedExe = Join-Path $installDir 'moe.exe'
    $installedCli = Join-Path $installDir 'moe.com'
    Assert-PeSubsystem -Path $installedExe -Subsystem 2
    Assert-PeSubsystem -Path $installedCli -Subsystem 3

    $previousPath = $env:Path
    try {
        $env:Path = "$installDir;$previousPath"
        $entry = Get-Command moe -CommandType Application
        if ($entry.Source -ne $installedCli) {
            throw "PowerShell 未选中 moe.com：$($entry.Source)"
        }
        $help = & $entry.Source -h
        if ($LASTEXITCODE -ne 0 -or ($help | Out-String) -notmatch '--shell') {
            throw 'PowerShell 没有收到 Moe 帮助输出。'
        }
        $reported = & cmd.exe /d /c 'moe --version'
        if ($LASTEXITCODE -ne 0 -or $reported -cne "moe $Expected") {
            throw "CMD 中的 Moe 没有报告预期版本 $Expected。"
        }
    } finally {
        $env:Path = $previousPath
    }
    if (-not (Test-Path -LiteralPath $shortcut) -or -not (Test-UserPathEntry)) {
        throw '未保留开始菜单快捷方式或用户 PATH 选项。'
    }
}

New-Item -ItemType Directory -Path $testRoot, $dataDir -Force | Out-Null
$uninstaller = Join-Path $installDir 'unins000.exe'
try {
    Set-Content -LiteralPath $marker -Value 'keep during upgrade and uninstall'
    if ((Invoke-SetupTest -Executable $Package -Options @(('/DIR="{0}"' -f $installDir), '/TASKS=userpath')) -ne 0) {
        throw '首次安装失败。'
    }
    Assert-InstalledVersion -Expected $Version
    if ((Invoke-SetupTest -Executable $UpgradePackage) -ne 0) {
        throw '覆盖升级失败。'
    }
    Assert-InstalledVersion -Expected $UpgradeVersion
    if ((Invoke-SetupTest -Executable $Package) -eq 0 -or
        (Invoke-SetupTest -Executable $UpgradePackage) -eq 0) {
        throw '安装器未拒绝降级或同版本重复安装。'
    }
    Assert-InstalledVersion -Expected $UpgradeVersion

    if ((Invoke-SetupTest -Executable $uninstaller) -ne 0 -or
        (Test-Path -LiteralPath (Join-Path $installDir 'moe.exe')) -or
        (Test-Path -LiteralPath (Join-Path $installDir 'moe.com')) -or
        (Test-Path -LiteralPath $shortcut) -or (Test-UserPathEntry) -or
        (Test-Path -LiteralPath $uninstallKey)) {
        throw '卸载失败，或保留了安装器创建的入口。'
    }
    if ((Get-Content -LiteralPath $marker -Raw).Trim() -cne 'keep during upgrade and uninstall') {
        throw '安装器更改了测试用户数据。'
    }
} finally {
    if ((Test-Path -LiteralPath $uninstallKey) -and (Test-Path -LiteralPath $uninstaller)) {
        Invoke-SetupTest -Executable $uninstaller | Out-Null
    }
    if (Test-Path -LiteralPath $marker) {
        Remove-Item -LiteralPath $marker
    }
    Remove-Item -LiteralPath $testRoot -Recurse -Force
}
