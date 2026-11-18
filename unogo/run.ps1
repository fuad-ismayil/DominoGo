param([int]$UnoPort=3001,[int]$DominoPort=3000,[string]$DatabasePath)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$publicDir = Join-Path $PSScriptRoot 'target/dx/uno-client/debug/web/public'
if (!(Test-Path -LiteralPath (Join-Path $publicDir 'index.html'))) { & (Join-Path $PSScriptRoot 'build.ps1') }
$env:UNOGO_STATIC_DIR = $publicDir
$env:UNOGO_PORT = "$UnoPort"
$env:DOMINO_PORT = "$DominoPort"
$dominoPublicDir = Join-Path $repoRoot 'rust/target/dx/domino-client/debug/web/public'
if (!$env:DOMINO_STATIC_DIR -and (Test-Path -LiteralPath (Join-Path $dominoPublicDir 'index.html'))) { $env:DOMINO_STATIC_DIR = $dominoPublicDir }
if ($DatabasePath) { $env:DOMINO_DB_PATH = [IO.Path]::GetFullPath($DatabasePath) }
Set-Location $repoRoot
Write-Host "UnoGo: http://localhost:$UnoPort"
cargo run --manifest-path (Join-Path $repoRoot 'rust/Cargo.toml') -p domino-server
if ($LASTEXITCODE) { throw 'Shared host stopped with an error' }
