param([int]$Port = 3000, [switch]$Release)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
$profileName = if ($Release) { 'release' } else { 'debug' }
$publicDir = Join-Path $PSScriptRoot "rust/target/dx/domino-client/$profileName/web/public"
if (!(Test-Path "$publicDir/index.html")) { throw 'Build the browser client first with ./build.ps1' }
$localTools = Join-Path $env:USERPROFILE '.domino-tools/mingw64/bin'
if (Test-Path $localTools) { $env:PATH = "$localTools;$env:PATH" }
$env:DOMINO_STATIC_DIR = $publicDir
$env:DOMINO_PORT = "$Port"
$runArgs = @('run', '--manifest-path', 'rust/Cargo.toml', '-p', 'domino-server')
if ($Release) { $runArgs += '--release' }
cargo @runArgs
exit $LASTEXITCODE
