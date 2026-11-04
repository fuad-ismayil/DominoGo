param([switch]$Release)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
$localTools = Join-Path $env:USERPROFILE '.domino-tools/mingw64/bin'
if (Test-Path $localTools) { $env:PATH = "$localTools;$env:PATH" }
$dx = Get-Command dx -ErrorAction SilentlyContinue
$dxPath = if ($dx) { $dx.Source } else { $null }
if (!$dx) {
    $bundledDx = Join-Path $env:USERPROFILE '.codex/tools/dioxus/dx.exe'
    if (Test-Path $bundledDx) { $dxPath = $bundledDx }
    else { throw 'Install Dioxus CLI 0.7.10 and add dx to PATH.' }
}
rustup target add wasm32-unknown-unknown
if ($LASTEXITCODE) { throw 'Unable to install WebAssembly target' }
Push-Location rust/crates/domino-client
try {
    $buildArgs = @('build', '--web', '--package', 'domino-client')
    if ($Release) { $buildArgs += '--release' }
    & $dxPath @buildArgs
    if ($LASTEXITCODE) { throw 'Browser build failed' }
} finally { Pop-Location }
$profileName = if ($Release) { 'release' } else { 'debug' }
$publicDir = Join-Path $PSScriptRoot "rust/target/dx/domino-client/$profileName/web/public"
if (!(Test-Path "$publicDir/index.html")) { throw 'Browser output index.html missing' }
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'public/sounds') -Destination $publicDir -Recurse -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'public/fonts') -Destination $publicDir -Recurse -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'public/icons') -Destination $publicDir -Recurse -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'public/site.webmanifest') -Destination $publicDir -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'public/favicon.ico') -Destination $publicDir -Force
$serverArgs = @('build', '--manifest-path', 'rust/Cargo.toml', '-p', 'domino-server')
if ($Release) { $serverArgs += '--release' }
cargo @serverArgs
if ($LASTEXITCODE) { throw 'Server build failed' }
