; Hodos Browser Installer - Inno Setup Script
; Build with: ISCC.exe /DAppVersion=0.1.0-alpha.1 /DProjectRoot=... /DStagingDir=... /DDistDir=... hodos-browser.iss

#ifndef AppVersion
  #define AppVersion "0.1.1-alpha.1"
#endif

#ifndef ProjectRoot
  #define ProjectRoot ".."
#endif

#ifndef StagingDir
  #define StagingDir ProjectRoot + "\staging\HodosBrowser"
#endif

#ifndef DistDir
  #define DistDir ProjectRoot + "\dist"
#endif

[Setup]
AppId={{F7A8D3E1-9B2C-4F5E-A6D0-3E7C8B1F2A4D}
AppName=Hodos Browser
AppVersion={#AppVersion}
AppVerName=Hodos Browser {#AppVersion}
AppPublisher=Hodos
DefaultDirName={localappdata}\HodosBrowser
DefaultGroupName=Hodos Browser
AllowNoIcons=yes
OutputDir={#DistDir}
OutputBaseFilename=HodosBrowser-{#AppVersion}-setup
Compression=lzma2/ultra
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DisableProgramGroupPage=yes
SetupIconFile={#ProjectRoot}\cef-native\hodos.ico
UninstallDisplayIcon={app}\HodosBrowserShell.exe

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; Main executable and helpers
Source: "{#StagingDir}\HodosBrowserShell.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StagingDir}\hodos-wallet.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StagingDir}\hodos-adblock.exe"; DestDir: "{app}"; Flags: ignoreversion

; CEF DLLs and runtime files
Source: "{#StagingDir}\*.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StagingDir}\*.bin"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StagingDir}\*.dat"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StagingDir}\*.pak"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StagingDir}\*.json"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

; Locales
Source: "{#StagingDir}\locales\*"; DestDir: "{app}\locales"; Flags: ignoreversion recursesubdirs

; Frontend
Source: "{#StagingDir}\frontend\*"; DestDir: "{app}\frontend"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\Hodos Browser"; Filename: "{app}\HodosBrowserShell.exe"
Name: "{group}\{cm:UninstallProgram,Hodos Browser}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Hodos Browser"; Filename: "{app}\HodosBrowserShell.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\HodosBrowserShell.exe"; Description: "{cm:LaunchProgram,Hodos Browser}"; Flags: nowait postinstall skipifsilent

[Code]
function IsAppRunning(): Boolean;
var
  ResultCode: Integer;
begin
  // Use tasklist to check if HodosBrowserShell.exe is running
  Exec('cmd.exe', '/c tasklist /FI "IMAGENAME eq HodosBrowserShell.exe" 2>NUL | find /I "HodosBrowserShell.exe" >NUL', '',
       SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Result := (ResultCode = 0);
end;

function InitializeUninstall(): Boolean;
begin
  Result := True;
  if IsAppRunning() then
  begin
    if MsgBox('Hodos Browser is currently running. Please close it before uninstalling.' + #13#10 + #13#10 +
              'Click OK to try again, or Cancel to abort.', mbError, MB_OKCANCEL) = IDCANCEL then
    begin
      Result := False;
    end else
    begin
      // Check again
      if IsAppRunning() then
      begin
        MsgBox('Hodos Browser is still running. Please close it and try again.', mbError, MB_OK);
        Result := False;
      end;
    end;
  end;
end;
