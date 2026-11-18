param([switch]$Install, [string]$Serial, [string]$CloudflaredSource)
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
$sdk = $env:ANDROID_HOME
if (!$sdk) { $sdk = $env:ANDROID_SDK_ROOT }
if (!$sdk) { throw 'Set ANDROID_HOME to your Android SDK directory.' }
$ndk = $env:ANDROID_NDK_HOME
if (!$ndk) { $ndk = Join-Path (Split-Path $sdk -Parent) 'android-ndk-r27d' }
$bin = Join-Path $ndk 'toolchains/llvm/prebuilt/windows-x86_64/bin'
if (!(Test-Path "$bin/aarch64-linux-android26-clang.cmd")) { throw 'Install Android NDK r27d or set ANDROID_NDK_HOME.' }
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = "$bin/aarch64-linux-android26-clang.cmd"
$env:CC_aarch64_linux_android = $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER
$env:AR_aarch64_linux_android = "$bin/llvm-ar.exe"
$env:RUSTFLAGS = '-C link-arg=-Wl,-z,max-page-size=16384'
rustup target add aarch64-linux-android
if ($LASTEXITCODE) { throw 'Could not install Rust Android target.' }
cargo build --manifest-path "$project/rust/Cargo.toml" -p domino-server --lib --release --target aarch64-linux-android
if ($LASTEXITCODE) { throw 'Rust Android build failed.' }
$libs = Join-Path $PSScriptRoot 'app/src/main/jniLibs/arm64-v8a'
New-Item -ItemType Directory -Force $libs | Out-Null
Copy-Item "$project/rust/target/aarch64-linux-android/release/libdomino_server.so" $libs -Force
if (!(Test-Path "$libs/libcloudflared.so") -or $CloudflaredSource) {
    $go = Get-Command go -ErrorAction SilentlyContinue
    $goExe = if ($go) { $go.Source } else { Join-Path (Split-Path $sdk -Parent) 'go/bin/go.exe' }
    if (!(Test-Path $goExe)) { throw 'Install Go 1.27.4 or newer to build the public tunnel.' }
    if (!$CloudflaredSource) {
        $CloudflaredSource = Join-Path $env:USERPROFILE '.codex/tools/domino-cloudflared'
        if (!(Test-Path "$CloudflaredSource/.git")) {
            git init $CloudflaredSource
            git -C $CloudflaredSource remote add origin https://github.com/cloudflare/cloudflared.git
        }
        git -C $CloudflaredSource fetch --depth 1 origin b0297f0b24c60a4cef743b7b878e2da3798cb839
        if ($LASTEXITCODE) { throw 'Could not fetch cloudflared source.' }
        git -C $CloudflaredSource checkout --detach FETCH_HEAD
    }
    $env:GOOS = 'android'; $env:GOARCH = 'arm64'; $env:CGO_ENABLED = '1'; $env:CC = $env:CC_aarch64_linux_android
    Copy-Item "$PSScriptRoot/cloudflared/domino_dns_android.go" "$CloudflaredSource/cmd/cloudflared/domino_dns_android.go" -Force
    Push-Location $CloudflaredSource
    try {
        & $goExe build -trimpath -ldflags '-s -w -extldflags=-Wl,-z,max-page-size=16384 -X main.Version=2026.11.13-domino -X github.com/cloudflare/cloudflared/cmd/cloudflared/updater.BuiltForPackageManager=DominoGo' -o "$libs/libcloudflared.so" ./cmd/cloudflared
        if ($LASTEXITCODE) { throw 'Cloudflared Android build failed.' }
    } finally { Pop-Location }
}
python "$PSScriptRoot/prepare_assets.py"
if ($LASTEXITCODE) { throw 'Build the release browser client before packaging the APK.' }
Push-Location $PSScriptRoot
try { & "$PSScriptRoot/gradlew.bat" --no-daemon assembleDebug; if ($LASTEXITCODE) { throw 'Android packaging failed.' } }
finally { Pop-Location }
New-Item -ItemType Directory -Force "$PSScriptRoot/dist" | Out-Null
$apk = "$PSScriptRoot/dist/GoHost.apk"
Copy-Item "$PSScriptRoot/app/build/outputs/apk/debug/app-debug.apk" $apk -Force
Copy-Item -LiteralPath $apk -Destination "$PSScriptRoot/dist/DominoGo-Host.apk" -Force
if ($Install) {
    $adbArgs = if ($Serial) { @('-s', $Serial) } else { @() }
    adb @adbArgs install -r $apk
    if ($LASTEXITCODE) { throw 'Connect and authorize the Android phone, then try installing again.' }
}
Write-Host "APK: $apk"
