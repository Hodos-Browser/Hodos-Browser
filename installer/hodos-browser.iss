; Hodos Browser Installer - Inno Setup Script
; Build with: ISCC.exe /DAppVersion=0.1.0-alpha.1 /DProjectRoot=... /DStagingDir=... /DDistDir=... hodos-browser.iss

#ifndef AppVersion
  #define AppVersion "0.3.0-beta"
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
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: checked

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

; --- Clean stale runtime files on install/upgrade ---
[InstallDelete]
Type: files; Name: "{app}\debug.log"
Type: files; Name: "{app}\debug_output.log"
Type: files; Name: "{app}\startup_log.txt"
Type: files; Name: "{app}\test_debug.log"

; --- Always clean runtime logs on uninstall ---
[UninstallDelete]
Type: files; Name: "{app}\debug.log"
Type: files; Name: "{app}\debug_output.log"
Type: files; Name: "{app}\startup_log.txt"
Type: files; Name: "{app}\test_debug.log"
Type: files; Name: "{app}\*.log"

[Code]
var
  DeleteDataCheckbox: TNewCheckBox;
  DeleteDataPage: TWizardPage;

function IsAppRunning(): Boolean;
var
  ResultCode: Integer;
begin
  // Use tasklist to check if HodosBrowserShell.exe is running
  Exec('cmd.exe', '/c tasklist /FI "IMAGENAME eq HodosBrowserShell.exe" 2>NUL | find /I "HodosBrowserShell.exe" >NUL', '',
       SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Result := (ResultCode = 0);
end;

function GetAppDataPath(): String;
begin
  Result := ExpandConstant('{userappdata}\HodosBrowser');
end;

function WalletExists(): Boolean;
begin
  Result := FileExists(GetAppDataPath() + '\wallet\wallet.db');
end;

function BrowsingDataExists(): Boolean;
begin
  Result := DirExists(GetAppDataPath() + '\Default') or
            FileExists(GetAppDataPath() + '\profiles.json');
end;

procedure InitializeUninstallProgressForm();
var
  UninstallPage: TSetupForm;
begin
  // The DeleteDataCheckbox is created in InitializeUninstall and shown
  // via CurUninstallStepChanged
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

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  ShouldDelete: Boolean;
  WalletWarning: Integer;
  ProfileDirs: TStringList;
  SearchRec: TFindRec;
  AppDataPath: String;
begin
  if CurUninstallStep = usUninstall then
  begin
    AppDataPath := GetAppDataPath();
    ShouldDelete := False;

    // Ask if user wants to delete browsing data
    if BrowsingDataExists() then
    begin
      if MsgBox('Do you want to delete your browsing data (history, bookmarks, settings, cookies)?' + #13#10 + #13#10 +
                'This does not affect your wallet. Click No to keep your data for future installs.',
                mbConfirmation, MB_YESNO) = IDYES then
      begin
        ShouldDelete := True;
      end;
    end;

    if ShouldDelete then
    begin
      // Wallet safety check — separate explicit warning
      if WalletExists() then
      begin
        WalletWarning := MsgBox(
          'WARNING: A wallet was found in your browser data.' + #13#10 + #13#10 +
          'If you have funds in this wallet and have NOT backed up your recovery phrase, ' +
          'deleting this data will result in PERMANENT LOSS of funds.' + #13#10 + #13#10 +
          'Do you want to delete your wallet as well?' + #13#10 + #13#10 +
          'Click Yes to delete EVERYTHING (including wallet).' + #13#10 +
          'Click No to keep your wallet safe.',
          mbCritical, MB_YESNO);
        if WalletWarning = IDNO then
        begin
          // Delete browsing data but preserve wallet
          // Delete Default profile (minus wallet)
          DelTree(AppDataPath + '\Default\cache', True, True, True);
          DelTree(AppDataPath + '\Default\Default', True, True, True);
          DeleteFile(AppDataPath + '\Default\bookmarks.db');
          DeleteFile(AppDataPath + '\Default\cookie_blocks.db');
          DeleteFile(AppDataPath + '\Default\HodosHistory');
          DeleteFile(AppDataPath + '\Default\settings.json');
          DeleteFile(AppDataPath + '\Default\adblock_settings.json');
          DeleteFile(AppDataPath + '\Default\fingerprint_settings.json');
          DeleteFile(AppDataPath + '\Default\session.json');
          DeleteFile(AppDataPath + '\Default\profile.lock');

          // Delete additional profile directories (Profile_1, Profile_2, etc.)
          if FindFirst(AppDataPath + '\Profile_*', SearchRec) then
          begin
            try
              repeat
                if SearchRec.Attributes and FILE_ATTRIBUTE_DIRECTORY <> 0 then
                  DelTree(AppDataPath + '\' + SearchRec.Name, True, True, True);
              until not FindNext(SearchRec);
            finally
              FindClose(SearchRec);
            end;
          end;

          // Delete app-level files (but not wallet dir)
          DeleteFile(AppDataPath + '\profiles.json');

          // Delete adblock data
          DelTree(AppDataPath + '\adblock', True, True, True);
        end else
        begin
          // User chose to delete everything including wallet
          DelTree(AppDataPath, True, True, True);
        end;
      end else
      begin
        // No wallet — safe to delete everything
        DelTree(AppDataPath, True, True, True);
      end;
    end;

    // Always clean WinSparkle registry entries
    RegDeleteKeyIncludingSubkeys(HKEY_CURRENT_USER, 'Software\Marston Enterprises\Hodos Browser');
  end;
end;
