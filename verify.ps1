param([string]$BaseUrl = 'http://localhost:3000')
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
$localTools = Join-Path $env:USERPROFILE '.domino-tools/mingw64/bin'
if (Test-Path $localTools) { $env:PATH = "$localTools;$env:PATH" }
cargo test --manifest-path rust/Cargo.toml --workspace
if ($LASTEXITCODE) { throw 'Rust tests failed' }
cargo check --manifest-path rust/Cargo.toml -p domino-client --target wasm32-unknown-unknown
if ($LASTEXITCODE) { throw 'Browser target check failed' }
function Api($Method, $Path, $Body, $Token) {
    $headers = @{}
    if ($Token) { $headers.Authorization = "Bearer $Token" }
    $request = @{ Method = $Method; Uri = "$BaseUrl$Path"; Headers = $headers }
    if ($null -ne $Body) {
        $request.Body = $Body | ConvertTo-Json -Compress
        $request.ContentType = 'application/json'
    }
    Invoke-RestMethod @request
}
$guest = Api POST '/api/auth/guest' @{prefix='Verify'} $null
if (!$guest.token) { throw 'Guest session missing' }
$me = Api GET '/api/auth/me' $null $guest.token
if (!$me.user) { throw 'Session lookup failed' }
$lobby = Api POST '/api/lobbies' @{maxPlayers=2} $guest.token
if (!$lobby.id) { throw 'Lobby creation failed' }
Api POST "/api/lobbies/$($lobby.id)/add-bot" @{} $guest.token | Out-Null
Api POST "/api/lobbies/$($lobby.id)/start" @{mode='classic'} $guest.token | Out-Null
$joined = Api POST "/api/lobbies/$($lobby.id)/join" @{} $guest.token
if ($joined.players.Count -ne 2 -or $joined.status -ne 'playing') { throw 'Started game membership or status incorrect' }
Api GET '/api/matches/history' $null $guest.token | Out-Null
Api GET '/api/leaderboard' $null $guest.token | Out-Null
Api POST '/api/auth/logout' @{} $guest.token | Out-Null
Write-Host 'Rust tests, browser compilation, session and match HTTP smoke checks passed.'
