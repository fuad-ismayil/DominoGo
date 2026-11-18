param([switch]$Release)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$dxCommand = Get-Command dx -ErrorAction SilentlyContinue
$dxPath = if ($dxCommand) { $dxCommand.Source } else { Join-Path $env:USERPROFILE '.codex/tools/dioxus/dx.exe' }
if (!(Test-Path -LiteralPath $dxPath)) { throw 'Dioxus CLI is required. Use the existing DominoGo tool installation.' }
Push-Location (Join-Path $PSScriptRoot 'crates/uno-client')
try {
    $buildArgs = @('build','--web','--package','uno-client')
    if ($Release) { $buildArgs += '--release' }
    & $dxPath @buildArgs
    if ($LASTEXITCODE) { throw 'UnoGo browser build failed' }
} finally { Pop-Location }
$profileName = if ($Release) { 'release' } else { 'debug' }
$publicDir = Join-Path $PSScriptRoot "target/dx/uno-client/$profileName/web/public"
if (!(Test-Path -LiteralPath (Join-Path $publicDir 'index.html'))) { throw 'Browser output is missing' }
Copy-Item -LiteralPath (Join-Path $repoRoot 'public/fonts') -Destination $publicDir -Recurse -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'crates/uno-client/assets/unogo.svg') -Destination $publicDir -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'crates/uno-client/assets/brand') -Destination $publicDir -Recurse -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'crates/uno-client/assets/manifest.webmanifest') -Destination $publicDir -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'crates/uno-client/assets/brand/favicon-v2.ico') -Destination (Join-Path $publicDir 'favicon.ico') -Force
Write-Host "UnoGo browser built: $publicDir"
