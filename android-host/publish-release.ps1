<#
Publishes dist/GoHost.apk as a GitHub release that Go Host can install itself.

Release contract (checked by GitHubUpdates.java):
  tag      android-v<versionName>        e.g. android-v2.3.0
  assets   GoHost.apk, GoHost.apk.sha256 ("<sha256>  GoHost.apk")
  state    published, not draft, not prerelease

Every phone pressing Start afterwards downloads it, checks the SHA-256 and the
signing certificate, stops its games, installs and restarts them.

Usage (needs a GitHub token with "contents: write" on the repository):
  $env:GITHUB_TOKEN = '<token>'
  ./android-host/publish-release.ps1 -Notes 'What changed'
Add -DryRun to validate everything without contacting GitHub.
#>
param([string]$Notes = '', [string]$Repository = 'fuad-ismayil/DominoGo', [switch]$DryRun)
$ErrorActionPreference = 'Stop'
$apk = Join-Path $PSScriptRoot 'dist/GoHost.apk'
if (!(Test-Path -LiteralPath $apk)) { throw 'Build first: ./android-host/build.ps1' }

$sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { $env:ANDROID_SDK_ROOT }
$aapt = Join-Path $sdk 'build-tools/35.0.1/aapt2.exe'
$badging = (& $aapt dump badging $apk | Select-Object -First 1)
if ($badging -notmatch "package: name='games\.dominogo\.host' versionCode='(\d+)' versionName='(\d+\.\d+(?:\.\d+)?)'") { throw "Unexpected APK identity: $badging" }
$versionCode = $Matches[1]; $version = $Matches[2]
$tag = "android-v$version"

$verify = Join-Path $PSScriptRoot 'verify_apk.py'
python $verify | Out-Null
if ($LASTEXITCODE) { throw 'verify_apk.py rejected the APK; fix it before publishing.' }

$hash = (Get-FileHash -LiteralPath $apk -Algorithm SHA256).Hash.ToLowerInvariant()
$sumFile = Join-Path $PSScriptRoot 'dist/GoHost.apk.sha256'
[IO.File]::WriteAllText($sumFile, "$hash  GoHost.apk`n")
$commit = (git -C (Split-Path $PSScriptRoot -Parent) rev-parse HEAD).Trim()
$dirty = git -C (Split-Path $PSScriptRoot -Parent) status --porcelain --untracked-files=no
$body = @"
Go Host $version (version code $versionCode).

$Notes

SHA-256 ``$hash``
Built from $commit$(if ($dirty) { ' with local changes' })
"@
Write-Host "Release $tag - $([math]::Round((Get-Item $apk).Length / 1MB, 1)) MB - sha256 $hash"
if ($dirty) { Write-Warning 'The working tree has uncommitted changes; the release notes say so.' }
if ($DryRun) { Write-Host 'Dry run: nothing was published.'; return }

if (!$env:GITHUB_TOKEN) { throw 'Set $env:GITHUB_TOKEN to a token with contents:write on the repository.' }
$headers = @{ Authorization = "Bearer $env:GITHUB_TOKEN"; Accept = 'application/vnd.github+json'; 'X-GitHub-Api-Version' = '2022-11-28' }
$api = "https://api.github.com/repos/$Repository"
try { Invoke-RestMethod -Headers $headers "$api/releases/tags/$tag" | Out-Null; throw "Release $tag already exists. Bump versionName/versionCode in app/build.gradle first." }
catch { if ($_.Exception.Response.StatusCode.value__ -ne 404) { throw } }

$release = Invoke-RestMethod -Method Post -Headers $headers "$api/releases" -ContentType 'application/json' -Body (@{
    tag_name = $tag; target_commitish = $commit; name = "Go Host $version"; body = $body; draft = $true; prerelease = $false
} | ConvertTo-Json)
$upload = $release.upload_url -replace '\{.*\}$', ''
Invoke-RestMethod -Method Post -Headers $headers "$($upload)?name=GoHost.apk" -ContentType 'application/vnd.android.package-archive' -InFile $apk | Out-Null
Invoke-RestMethod -Method Post -Headers $headers "$($upload)?name=GoHost.apk.sha256" -ContentType 'text/plain' -InFile $sumFile | Out-Null
# Publish only after both assets exist, so phones never see a half-uploaded release.
$published = Invoke-RestMethod -Method Patch -Headers $headers "$api/releases/$($release.id)" -ContentType 'application/json' -Body (@{ draft = $false; make_latest = 'true' } | ConvertTo-Json)
Write-Host "Published $($published.html_url)"
