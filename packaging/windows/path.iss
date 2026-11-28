// 精确管理自己追加的一个用户 PATH 段；不重建整份 PATH、不使用 setx。
function RegOpenKeyExW(hKey: HKEY; lpSubKey: String; ulOptions, samDesired: LongWord; var phkResult: HKEY): Longint;
  external 'RegOpenKeyExW@advapi32.dll stdcall';
function RegQueryValueExW(hKey: HKEY; lpValueName: String; lpReserved: NativeUInt; var lpType: LongWord; lpData: NativeUInt; var lpcbData: LongWord): Longint;
  external 'RegQueryValueExW@advapi32.dll stdcall';
function RegCloseKey(hKey: HKEY): Longint;
  external 'RegCloseKey@advapi32.dll stdcall';

function ReadUserPath(var Value: String; var Kind: LongWord): Boolean;
var
  Key: HKEY;
  Size: LongWord;
begin
  Value := '';
  Kind := 2;
  Result := RegValueExists(HKCU, EnvironmentKey, 'Path');
  if not Result then exit;
  if RegOpenKeyExW(HKCU, EnvironmentKey, 0, $0001, Key) <> 0 then
    RaiseException('Cannot read the user PATH registry key.');
  try
    Size := 0;
    if RegQueryValueExW(Key, 'Path', 0, Kind, 0, Size) <> 0 then
      RaiseException('Cannot query the user PATH value type.');
  finally
    RegCloseKey(Key);
  end;
  // 保留 REG_SZ / REG_EXPAND_SZ 及未展开的 %变量%，拒绝覆盖非字符串值。
  if ((Kind <> 1) and (Kind <> 2)) or
     not RegQueryStringValue(HKCU, EnvironmentKey, 'Path', Value) then
    RaiseException('The user PATH is not a readable string.');
end;

procedure WriteUserPath(const Value: String; Kind: LongWord);
var
  Success: Boolean;
begin
  if Kind = 1 then
    Success := RegWriteStringValue(HKCU, EnvironmentKey, 'Path', Value)
  else
    Success := RegWriteExpandStringValue(HKCU, EnvironmentKey, 'Path', Value);
  if not Success then RaiseException('Cannot update the user PATH.');
end;

function NormalizePathToken(Value: String): String;
begin
  Value := Trim(Value);
  if (Length(Value) >= 2) and (Value[1] = '"') and (Value[Length(Value)] = '"') then
    Value := Copy(Value, 2, Length(Value) - 2);
  while (Length(Value) > 3) and (Value[Length(Value)] = '\') do
    Delete(Value, Length(Value), 1);
  Result := Lowercase(Value);
end;

function FindPathToken(const Value, Token: String; var TokenStart, TokenLength: Integer): Boolean;
var
  I, Start: Integer;
  Scan: String;
begin
  Result := False;
  Start := 1;
  Scan := Value + ';';
  // 找最后一个完全相等的段；只删自己追加的一个，不改动其他段及其格式。
  for I := 1 to Length(Scan) do begin
    if Scan[I] = ';' then begin
      if NormalizePathToken(Copy(Value, Start, I - Start)) = NormalizePathToken(Token) then begin
        TokenStart := Start;
        TokenLength := I - Start;
        Result := True;
      end;
      Start := I + 1;
    end;
  end;
end;

procedure RemoveOwnedPath;
var
  Owned, Value: String;
  Kind, HadPath, ReusedSeparator: LongWord;
  Start, Count: Integer;
begin
  if not RegValueExists(HKCU, OwnershipKey, 'AddedPath') then begin
    DeleteOwnedRegistryValue('HadPath');
    DeleteOwnedRegistryValue('ReusedPathSeparator');
    exit;
  end;
  if not RegQueryStringValue(HKCU, OwnershipKey, 'AddedPath', Owned) then
    RaiseException('Invalid PATH ownership record.');
  ReadUserPath(Value, Kind);
  if FindPathToken(Value, Owned, Start, Count) then begin
    // 老版本没有该记录，按原策略撤销自己附加的分隔符。
    ReusedSeparator := 0;
    RegQueryDWordValue(HKCU, OwnershipKey, 'ReusedPathSeparator', ReusedSeparator);
    if (Start > 1) and (ReusedSeparator = 0) then begin
      Start := Start - 1;
      Count := Count + 1;
    // 复用原有尾分号时保留它。若用户后来加了新项，移除目录后的分号。
    end else if Count < Length(Value) - Start + 1 then
      Count := Count + 1;
    Delete(Value, Start, Count);
    HadPath := 1;
    RegQueryDWordValue(HKCU, OwnershipKey, 'HadPath', HadPath);
    if (Value = '') and (HadPath = 0) then begin
      if not RegDeleteValue(HKCU, EnvironmentKey, 'Path') then
        RaiseException('Cannot remove the PATH value created by this installer.');
    end else
      WriteUserPath(Value, Kind);
  end;
  DeleteOwnedRegistryValue('AddedPath');
  DeleteOwnedRegistryValue('HadPath');
  DeleteOwnedRegistryValue('ReusedPathSeparator');
end;

procedure UpdateOwnedPath(Enabled: Boolean);
var
  Value, Target, Owned: String;
  Kind: LongWord;
  HadPath, ReusedSeparator: Boolean;
  Start, Count: Integer;
begin
  Target := ExpandConstant('{app}');
  // 自定义位置变化时先撤销旧归属；相同位置的覆盖安装复用原记录。
  if RegQueryStringValue(HKCU, OwnershipKey, 'AddedPath', Owned) and
     (not Enabled or (NormalizePathToken(Owned) <> NormalizePathToken(Target))) then
    RemoveOwnedPath;
  if not Enabled then exit;
  HadPath := ReadUserPath(Value, Kind);
  if FindPathToken(Value, Target, Start, Count) then exit;
  ReusedSeparator := Copy(Value, Length(Value), 1) = ';';
  try
    if not RegWriteDWordValue(HKCU, OwnershipKey, 'HadPath', Ord(HadPath)) or
       not RegWriteDWordValue(HKCU, OwnershipKey, 'ReusedPathSeparator', Ord(ReusedSeparator)) or
       not RegWriteStringValue(HKCU, OwnershipKey, 'AddedPath', Target) then
      RaiseException('Cannot record PATH ownership.');
    if (Value <> '') and not ReusedSeparator then Value := Value + ';';
    WriteUserPath(Value + Target, Kind);
  except
    RegDeleteValue(HKCU, OwnershipKey, 'AddedPath');
    RegDeleteValue(HKCU, OwnershipKey, 'HadPath');
    RegDeleteValue(HKCU, OwnershipKey, 'ReusedPathSeparator');
    RaiseException(GetExceptionMessage);
  end;
end;
