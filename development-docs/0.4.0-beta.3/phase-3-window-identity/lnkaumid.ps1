$sh = New-Object -ComObject Shell.Application
$targets = @(
  @{d="$env:APPDATA\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar"; f="Hodos Browser.lnk"},
  @{d="$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Hodos Browser"; f=$null}
)
foreach ($t in $targets) {
  if (-not (Test-Path $t.d)) { Write-Output "MISSING DIR: $($t.d)"; continue }
  $ns = $sh.NameSpace($t.d)
  $files = if ($t.f) { @($t.f) } else { (Get-ChildItem -Path $t.d -Filter *.lnk).Name }
  foreach ($f in $files) {
    $item = $ns.ParseName($f)
    if (-not $item) { Write-Output "MISSING: $f"; continue }
    $aumid = $item.ExtendedProperty("System.AppUserModel.ID")
    $wsh = New-Object -ComObject WScript.Shell
    $lnk = $wsh.CreateShortcut((Join-Path $t.d $f))
    Write-Output ("LNK   : " + (Join-Path $t.d $f))
    Write-Output ("TARGET: " + $lnk.TargetPath)
    Write-Output ("AUMID : " + $(if ([string]::IsNullOrEmpty($aumid)) { "<NONE>" } else { $aumid }))
    Write-Output "---"
  }
}
