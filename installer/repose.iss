; repose per-user installer (per-user-installer). Built by scripts/build-installer.ps1 with Inno
; Setup 7.1.1 from the gated files in target\dist; the version comes from Cargo.toml as /DAppVersion.
; It installs for the current user only, with no UAC prompt, to the folder 1.4.0 and earlier were
; copied into, so an upgrade lands in place. Settings live in %APPDATA%\repose and are never touched.
#ifndef AppVersion
  #error Pass /DAppVersion=<version> (scripts/build-installer.ps1 does)
#endif

[Setup]
AppId={{D4E5F741-368E-47BC-81F5-3FEAB058CFB0}
AppName=repose
AppVersion={#AppVersion}
AppVerName=repose {#AppVersion}
AppPublisher=NomadicDaddy
AppPublisherURL=https://github.com/NomadicDaddy/repose
AppSupportURL=https://github.com/NomadicDaddy/repose/issues
DefaultDirName={localappdata}\Programs\repose
DisableDirPage=yes
DefaultGroupName=repose
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesEnvironment=yes
CloseApplications=no
LicenseFile=..\target\dist\LICENSE
OutputDir=..\target\installer
OutputBaseFilename=repose-setup
SetupIconFile=..\assets\icon\repose.ico
UninstallDisplayIcon={app}\repose.exe
UninstallDisplayName=repose
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Files]
Source: "..\target\dist\repose.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\dist\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\dist\THIRD-PARTY-NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\dist\RUST-STD-COPYRIGHT.html"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\repose"; Filename: "{app}\repose.exe"; WorkingDir: "{userdocs}"

[Code]
const
  EnvKey = 'Environment';
  MarkKey = 'Software\repose\installer';

{ True when a repose.exe runs from the install folder, by WMI, so setup can stop rather than
  close it or leave the folder half replaced. }
function ReposeRunning(): Boolean;
var
  Locator, Service, Found: Variant;
  Target: String;
begin
  Result := False;
  Target := ExpandConstant('{app}\repose.exe');
  StringChangeEx(Target, '\', '\\', True);
  try
    Locator := CreateOleObject('WbemScripting.SWbemLocator');
    Service := Locator.ConnectServer('.', 'root\cimv2');
    Found := Service.ExecQuery('SELECT ProcessId FROM Win32_Process WHERE ExecutablePath = ''' + Target + '''');
    Result := Found.Count > 0;
  except
    Result := False;
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if ReposeRunning() then
    Result := 'repose is running from ' + ExpandConstant('{app}') + '. Close it, then run setup again.';
end;

{ An entry with each %NAME% replaced by that environment variable, for comparison only; an unknown
  or unclosed name is left as written. }
function ExpandEnv(const S: String): String;
var
  Rest, Name: String;
  Open, Close: Integer;
begin
  Result := '';
  Rest := S;
  while True do
  begin
    Open := Pos('%', Rest);
    if Open = 0 then Break;
    Close := Pos('%', Copy(Rest, Open + 1, Length(Rest)));
    if Close = 0 then Break;
    Name := Copy(Rest, Open + 1, Close - 1);
    if (Name <> '') and (GetEnv(Name) <> '') then
      Result := Result + Copy(Rest, 1, Open - 1) + GetEnv(Name)
    else
      Result := Result + Copy(Rest, 1, Open + Close);
    Rest := Copy(Rest, Open + Close + 1, Length(Rest));
  end;
  Result := Result + Rest;
end;

{ The folder's place in the raw user PATH, compared without a trailing backslash and ignoring case;
  -1 when absent. Entries that hold %variables% are expanded only for the comparison. }
function PathIndex(const Raw, Dir: String; var Parts: TArrayOfString): Integer;
var
  Rest, Entry: String;
  Count, Cut, I: Integer;
begin
  Result := -1;
  Count := 0;
  SetArrayLength(Parts, 0);
  Rest := Raw;
  while True do
  begin
    Cut := Pos(';', Rest);
    if Cut = 0 then Entry := Rest else Entry := Copy(Rest, 1, Cut - 1);
    SetArrayLength(Parts, Count + 1);
    Parts[Count] := Entry;
    Count := Count + 1;
    if Cut = 0 then Break;
    Rest := Copy(Rest, Cut + 1, Length(Rest));
  end;
  for I := 0 to Count - 1 do
    if (Parts[I] <> '') and (CompareText(RemoveBackslashUnlessRoot(ExpandEnv(Parts[I])), RemoveBackslashUnlessRoot(Dir)) = 0) then
    begin
      Result := I;
      Exit;
    end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Raw, Dir: String;
  Parts: TArrayOfString;
begin
  if CurStep <> ssPostInstall then Exit;
  Dir := ExpandConstant('{app}');
  if not RegQueryStringValue(HKCU, EnvKey, 'Path', Raw) then Raw := '';
  if PathIndex(Raw, Dir, Parts) >= 0 then Exit;
  { Appended to the value as it is, so every existing byte survives; the value is written as
    REG_EXPAND_SZ, the type Windows gives the user PATH. }
  if Raw = '' then Raw := Dir
  else if Raw[Length(Raw)] = ';' then Raw := Raw + Dir
  else Raw := Raw + ';' + Dir;
  if RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Raw) then
    RegWriteStringValue(HKCU, MarkKey, 'AddedPath', Dir);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Raw, Added, Kept: String;
  Parts: TArrayOfString;
  At, I: Integer;
  First: Boolean;
begin
  if CurUninstallStep <> usPostUninstall then Exit;
  { Only the entry this installer added comes out; a folder the user put on PATH stays. }
  if RegQueryStringValue(HKCU, MarkKey, 'AddedPath', Added) and RegQueryStringValue(HKCU, EnvKey, 'Path', Raw) then
  begin
    At := PathIndex(Raw, Added, Parts);
    if At >= 0 then
    begin
      { Every other entry, empty ones included, rejoined in order. }
      Kept := '';
      First := True;
      for I := 0 to GetArrayLength(Parts) - 1 do
        if I <> At then
        begin
          if not First then Kept := Kept + ';';
          Kept := Kept + Parts[I];
          First := False;
        end;
      RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Kept);
    end;
  end;
  RegDeleteKeyIncludingSubkeys(HKCU, MarkKey);
  RegDeleteKeyIfEmpty(HKCU, 'Software\repose');
end;
