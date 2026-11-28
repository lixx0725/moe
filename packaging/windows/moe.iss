; 版本来自 Cargo，AppId 和互斥体名称跨版本保持稳定。
#define AppId "io.github.lixx0725.moe"
#ifndef AppVersion
  #error Run scripts/package-windows.ps1 to build this installer.
#endif

[Setup]
AppId={#AppId}
AppName=Moe
AppPublisher=Moe contributors
AppVersion={#AppVersion}
VersionInfoVersion={#AppVersion}
AppSupportURL=https://github.com/lixx0725/moe/issues
DefaultDirName={localappdata}\Programs\Moe
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
UsePreviousAppDir=yes
UsePreviousTasks=yes
DisableDirPage=auto
DisableProgramGroupPage=yes
DisableReadyPage=no
UninstallDisplayIcon={app}\moe.ico
SetupIconFile=moe.ico
OutputDir={#OutputDir}
OutputBaseFilename=moe-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
AppMutex=Local\{#AppId}-app
SetupMutex=Local\{#AppId}-setup
CloseApplications=no
RestartApplications=no
ChangesEnvironment=yes

[Tasks]
Name: "userpath"; Description: "Add moe to the current user's PATH"; Flags: unchecked

[Files]
Source: "{#MoeExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MoeCli}"; DestDir: "{app}"; DestName: "moe.com"; Flags: ignoreversion
Source: "moe.ico"; DestDir: "{app}"
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "upgrade-notes.txt"; Flags: dontcopy noencryption

[Icons]
Name: "{userprograms}\Moe"; Filename: "{app}\moe.exe"; WorkingDir: "{%USERPROFILE}"; IconFilename: "{app}\moe.ico"

[Code]
const
  UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{#AppId}_is1';
  OwnershipKey = 'Software\{#AppId}\Installer';
  EnvironmentKey = 'Environment';

var
  // 现有版本与目录从本安装身份的卸载记录读取。
  PreviousVersion, PreviousDirectory: String;

procedure DeleteOwnedRegistryValue(const Name: String);
begin
  if RegValueExists(HKCU, OwnershipKey, Name) and
     not RegDeleteValue(HKCU, OwnershipKey, Name) then
    RaiseException('Cannot remove PATH ownership value: ' + Name);
end;

#include "path.iss"

function RejectSetup(const Reason: String): Boolean;
begin
  Log(Reason);
  SuppressibleMsgBox(Reason, mbInformation, MB_OK, IDOK);
  Result := False;
end;

// 在任何写入之前决定安装、升级或拒绝。版本按数值比较。
function InitializeSetup: Boolean;
var
  Installed, Incoming: Int64;
  Comparison: Integer;
begin
  Result := True;
  if not RegKeyExists(HKCU, UninstallKey) then exit;
  if not RegQueryStringValue(HKCU, UninstallKey, 'DisplayVersion', PreviousVersion) or
     not StrToVersion(PreviousVersion, Installed) or
     not RegQueryStringValue(HKCU, UninstallKey, 'InstallLocation', PreviousDirectory) or
     (PreviousDirectory = '') then begin
    Result := RejectSetup('Cannot read the version or directory of the existing Moe installation.');
    exit;
  end;
  if not StrToVersion('{#AppVersion}', Incoming) then
    RaiseException('Cannot read the version of this package.');
  Comparison := ComparePackedVersion(Installed, Incoming);
  if Comparison = 0 then
    Result := RejectSetup('Moe {#AppVersion} is already installed.')
  else if Comparison > 0 then
    Result := RejectSetup(Format('Moe %s is already installed. This older package (%s) cannot replace it.', [
      PreviousVersion, '{#AppVersion}']));
end;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  // 升级沿用 Inno 从现有安装读取的 PATH 选项。
  Result := (PreviousVersion <> '') and (PageID = wpSelectTasks);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Directory: String;
begin
  Result := '';
  Directory := RemoveBackslashUnlessRoot(ExpandFileName(ExpandConstant('{app}')));
  if (PreviousDirectory <> '') and
     (CompareText(Directory, RemoveBackslashUnlessRoot(ExpandFileName(PreviousDirectory))) <> 0) then
    Result := 'Upgrade must use the existing installation directory: ' + PreviousDirectory
  else if WizardIsTaskSelected('userpath') and
     ((Pos(';', Directory) > 0) or (Pos('%', Directory) > 0)) then
    Result := 'Choose a directory without semicolons or percent signs to add it to PATH.';
end;

// 更新说明与安装包一起构建，在确认写入前展示版本区间及本次变动。
function UpdateReadyMemo(Space, NewLine, MemoUserInfo, MemoDirInfo, MemoTypeInfo,
  MemoComponentsInfo, MemoGroupInfo, MemoTasksInfo: String): String;
var
  Notes: TArrayOfString;
  NotesFile: String;
  Index: Integer;
begin
  Result := MemoDirInfo + NewLine + NewLine + MemoTasksInfo;
  if PreviousVersion = '' then exit;
  Result := 'Update Moe: ' + PreviousVersion + ' -> {#AppVersion}' + NewLine + NewLine + Result;
  NotesFile := ExpandConstant('{tmp}\upgrade-notes.txt');
  if not FileExists(NotesFile) then
    ExtractTemporaryFile('upgrade-notes.txt');
  if LoadStringsFromFile(NotesFile, Notes) then begin
    Result := Result + NewLine + 'Changes in this package:' + NewLine;
    for Index := 0 to GetArrayLength(Notes) - 1 do
      Result := Result + Space + Notes[Index] + NewLine;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    UpdateOwnedPath(WizardIsTaskSelected('userpath'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then begin
    // 用户数据在 AppPaths 目录中，卸载只撤销安装器持有的 PATH 归属。
    RemoveOwnedPath;
    RegDeleteKeyIfEmpty(HKCU, OwnershipKey);
    RegDeleteKeyIfEmpty(HKCU, 'Software\{#AppId}');
  end;
end;
