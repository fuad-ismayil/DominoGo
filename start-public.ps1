param([int]$Port = 3000)
$ErrorActionPreference = 'Stop'
$projectDir = $PSScriptRoot
$stateDir = Join-Path $env:USERPROFILE '.codex/domino-remote-test'
New-Item -ItemType Directory -Path $stateDir -Force | Out-Null
$launchMutex = New-Object System.Threading.Mutex($false, 'Local\DominoGoPublicLauncher')
if (!$launchMutex.WaitOne(0)) { throw 'DominoGo is already starting. Wait for that launcher to finish.' }
try {
    $databaseSyncLock = [System.IO.File]::Open((Join-Path $projectDir '.domino-sync.lock'), 'OpenOrCreate', 'ReadWrite', 'None')
    $serverExe = Join-Path $projectDir 'rust/target/release/domino-server.exe'
    $staticDir = Join-Path $projectDir 'rust/target/dx/domino-client/release/web/public'
    if (!(Test-Path $serverExe) -or !(Test-Path "$staticDir/index.html")) {
        throw 'Release build missing. Run ./build.ps1 -Release first.'
    }
    $cloudflared = (Get-Command cloudflared -ErrorAction Stop).Source
    $localUrl = "http://127.0.0.1:$Port"
    function Test-GameServer {
        try {
            $response = Invoke-WebRequest "$localUrl/api/leaderboard" -UseBasicParsing -TimeoutSec 3
            $homePage = Invoke-WebRequest $localUrl -UseBasicParsing -TimeoutSec 3
            return ($response.StatusCode -eq 200 -and $homePage.Content -match '<title>DominoGo</title>')
        } catch { return $false }
    }
    if (!(Test-GameServer)) {
        if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) {
            throw "Port $Port is in use by another service. Stop it or choose another -Port."
        }
        Write-Host 'Starting DominoGo engine...'
        $env:DOMINO_PORT = "$Port"
        $env:DOMINO_DB_PATH = Join-Path $projectDir 'database.sqlite'
        $env:DOMINO_STATIC_DIR = $staticDir
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss-fff'
        $serverProcess = Start-Process -FilePath $serverExe -WorkingDirectory $projectDir -WindowStyle Hidden -PassThru -RedirectStandardOutput "$stateDir/server-$stamp.stdout.log" -RedirectStandardError "$stateDir/server-$stamp.stderr.log"
        Set-Content "$stateDir/server.pid" $serverProcess.Id
        $ready = $false
        for ($attempt = 0; $attempt -lt 30; $attempt++) {
            if (Test-GameServer) { $ready = $true; break }
            if ($serverProcess.HasExited) { throw "Engine exited. Check logs in $stateDir" }
            Start-Sleep -Milliseconds 500
        }
        if (!$ready) { throw "Engine did not become ready. Check logs in $stateDir" }
    } else { Write-Host 'DominoGo engine is already running.' }

    $tunnelStatePath = Join-Path $stateDir 'launcher-tunnel.json'
    $publicUrl = $null
    if (Test-Path $tunnelStatePath) {
        try {
            $saved = Get-Content $tunnelStatePath -Raw | ConvertFrom-Json
            $existingProcess = Get-Process -Id $saved.pid -ErrorAction Stop
            if ($existingProcess.Path -eq $cloudflared -and $saved.port -eq $Port) {
                $check = Invoke-WebRequest $saved.url -UseBasicParsing -TimeoutSec 8
                if ($check.StatusCode -eq 200 -and $check.Content -match '<title>DominoGo</title>') { $publicUrl = $saved.url }
            }
        } catch { }
    }
    if (!$publicUrl) {
        Write-Host 'Opening public Cloudflare link...'
        # Dedicated empty config avoids touching the user's named tunnel.
        $configPath = Join-Path $stateDir 'launcher-quick-tunnel.yml'
        Set-Content -LiteralPath $configPath -Value "# Isolated DominoGo quick tunnel`nno-autoupdate: true" -Encoding ascii
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss-fff'
        $tunnelLog = Join-Path $stateDir "tunnel-$stamp.stderr.log"
        $tunnelProcess = Start-Process -FilePath $cloudflared -ArgumentList @('tunnel', '--config', ('"' + $configPath + '"'), '--url', $localUrl, '--no-autoupdate') -WindowStyle Hidden -PassThru -RedirectStandardOutput "$stateDir/tunnel-$stamp.stdout.log" -RedirectStandardError $tunnelLog
        Set-Content "$stateDir/tunnel.pid" $tunnelProcess.Id
        for ($attempt = 0; $attempt -lt 120; $attempt++) {
            if (Test-Path $tunnelLog) {
                $logText = [string](Get-Content $tunnelLog -Raw)
                $found = [regex]::Match($logText, 'https://[a-z0-9-]+\.trycloudflare\.com')
                if ($found.Success) { $publicUrl = $found.Value; break }
            }
            if ($tunnelProcess.HasExited) { throw "Public tunnel exited. Check $tunnelLog" }
            Start-Sleep -Milliseconds 500
        }
        if (!$publicUrl) { throw "No public URL received. Check $tunnelLog" }
        @{pid=$tunnelProcess.Id;url=$publicUrl;port=$Port;log=$tunnelLog} | ConvertTo-Json | Set-Content $tunnelStatePath
    }
    Set-Content "$stateDir/url.txt" $publicUrl
    Write-Host "`nLocal game: $localUrl"
    Write-Host "Public game: $publicUrl" -ForegroundColor Green
    Write-Host 'Keep this computer awake and online. Closing this launcher leaves the game running.'
    Write-Host 'A new tunnel usually gets a new address. Run the desktop shortcut again to see the current link.'
} finally {
    if ($databaseSyncLock) { $databaseSyncLock.Dispose() }
    $launchMutex.ReleaseMutex()
    $launchMutex.Dispose()
}
