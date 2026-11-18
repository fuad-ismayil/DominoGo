param([string]$Serial = '2365de5706057ece', [switch]$InstallStartup, [switch]$Stop)
$ErrorActionPreference = 'Stop'
$state = Join-Path $env:USERPROFILE '.codex/domino-phone-sync'
New-Item -ItemType Directory -Force $state | Out-Null
$script = Join-Path $PSScriptRoot 'sync_pc.py'
$python = (Get-Command python -ErrorAction Stop).Source
$adb = (Get-Command adb -ErrorAction Stop).Source
$existing = Get-CimInstance Win32_Process | Where-Object { $_.Name -like 'python*' -and $_.CommandLine -like "*$script*" -and $_.CommandLine -like "*--serial $Serial*" }
$shortcutPath = Join-Path ([Environment]::GetFolderPath('Startup')) 'DominoGo phone sync.lnk'
if ($Stop) {
    foreach ($process in $existing) { Stop-Process -Id $process.ProcessId }
    if (Test-Path -LiteralPath $shortcutPath) { Remove-Item -LiteralPath $shortcutPath }
    Write-Host 'Phone sync stopped and removed from Windows startup.'
    exit
}
if ($InstallStartup) {
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($shortcutPath)
    $shortcut.TargetPath = (Get-Command powershell.exe).Source
    $shortcut.Arguments = '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + $PSCommandPath + '" -Serial "' + $Serial + '"'
    $shortcut.WorkingDirectory = $PSScriptRoot
    $shortcut.WindowStyle = 7
    $shortcut.Description = 'Sync the paired DominoGo phone when both game servers are stopped.'
    $shortcut.Save()
}
if (!$existing) {
    Start-Process -FilePath $python -ArgumentList @(('"' + $script + '"'), '--serial', $Serial, '--adb', ('"' + $adb + '"'), '--watch') -WindowStyle Hidden -PassThru -RedirectStandardOutput "$state/watcher.stdout.log" -RedirectStandardError "$state/watcher.stderr.log" | Select-Object -ExpandProperty Id | Set-Content "$state/watcher.pid"
}
Write-Host 'Phone database sync is enabled. It waits while either game server is running.'
