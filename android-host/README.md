# Go Host for Android

## Current deployment: 2.3.3, version code 18

Self-updated November 19, 2026, 02:08 from
[android-v2.3.3](https://github.com/fuad-ismayil/DominoGo/releases/tag/android-v2.3.3)
**without any prompt**: Stop all, Start both, and the phone downloaded,
verified, installed and restarted hosting within about a minute. DominoGo menus
scroll less: Home drops the duplicate Friends/Leaderboard buttons, the bottom
capsule is Home/Friends/Ranking with a friend-request badge, and the phone
create-table page fits one screen.

### Previous 2.3.2 deployment

First self-update, November 19, 2026, 01:31: Start found release
[android-v2.3.2](https://github.com/fuad-ismayil/DominoGo/releases/tag/android-v2.3.2),
downloaded and verified it, stopped both games, installed after the one-time
"Install unknown apps" permission and Android confirmation, and restarted both
games automatically within seconds. Go Host is now its own installer
(`installerPackageName=games.dominogo.host`), so on Android 12+ later releases
install without a confirmation prompt. Both public domains served the new
version immediately. APK SHA-256:
`f2979c5693eaeee1084a9584c8c278aa25065c69afd631bc4d182baf00aa1d43`.

### Previous 2.3.1 deployment

Installed over ADB November 19, 2026. GitHub's anonymous API allows 60
requests per hour per public IP, shared by every device on the connection. A
limited check no longer stops hosting: the app starts the installed version
and says when checks work again. Answers are cached with ETags; GitHub does
not count "not modified" replies, and the last answer is used if GitHub
refuses. Release APK downloads are not API requests and are not limited this way.

### Previous 2.3.0 deployment

Installed over ADB November 19, 2026. Go Host can now update itself from
GitHub releases (see [Self-update](#self-update-from-github-releases)). The
server also registers each socket's session and events before answering the
Socket.IO connect, fixing a race where a `join_lobby` sent right after
connecting was silently dropped. No published release exists yet, so Start
still starts normally. APK SHA-256:
`2f29063dd22492c05a783cabab9db60f0fbbd725c8f246a9fb3b0b48a0d64291`.

### Previous 2.2.3 deployment

Installed in place November 18, 2026. DominoGo hosts can switch the 30-second
turn timer off when creating a table (stored as `lobbies.turn_timer`, default
on). The phone server adds the column on start; PC sync adds it to the PC
database and upgrades an older baseline, so sync keeps working once both
servers have run the new version. UnoGo is unchanged. Both public domains
serve this build from the phone. APK SHA-256:
`85d349b1130b3be447566dd09e19f16e0587f0a047cfa793e515e72169da56bf`.

### Previous 2.2.2 deployment

Direct same-number selection replaces the Group button. At least one selected
card must match the discard's color or number; a different number or unmatched
pair replaces the selection. Chosen order determines the final color. Selected
cards keep the fan's stacking order so neighboring tap targets remain reachable.
Browser mouse, keyboard and physical Android selection/play checks passed.
Both public games run on the phone; desktop production hosting stays stopped.
See [direct selection verification](../unogo/docs/direct-selection.md).

### Previous 2.2.1 deployment

The visual follow-up removes card color captions, keeps the discard centered,
moves a smaller deck to the upper-left and separates the table edge from status
text. Seat-anchored animated emotes now include the sender and escape clipping.
The UNO toggle stays readable and shows a checkmark when armed. Browser geometry,
toggle, emote lifecycle and physical Android touch/emote checks passed. Both
public games are running from the phone. See
[visual verification](../unogo/docs/visual-feedback.md).

### Previous 2.2.0 deployment

Installed in place November 17, 2026, after real PC/phone database sync. Both
public games run on the phone; desktop production hosting remains stopped.
UnoGo adds ordered number groups, optional 0 swaps, a shared 5 tap challenge,
refined cards, visible emotes and the smaller direct draw deck. Existing saved
Classic rounds keep their rules. Start a new round for Friends rules.

Physical Android OS touches passed card dragging and the 5 challenge, using
isolated data. All 19 native updater checks passed; the real GitHub check
succeeded. Served HTML/WASM match the final APK. See
[playtest verification](../unogo/docs/friends-playtest.md) and
the checksum-bound `dist/GoHost.device-check.json`.

### Previous 2.1.2 deployment

Local development builds now start normally after a successful GitHub app-update
check. Missing published-source metadata is informational, not a network failure.
Verified newer APKs still prompt for an update; genuine connection failures retain
Retry and Start installed. Five JVM tests and 19 on-device updater checks passed.
The real phone GitHub check succeeded and both public games are running.

The 2.1.1 patch repairs re-encoded UTF-8 separators/messages and restores result
screens across reloads. Live and archived results passed repeated-reload checks.
The patch is installed on the phone, with both games publicly available again.

Installed in place on the Galaxy S9 on November 16, 2026. The old 1.5 host was
upgraded through the shared schema before syncing; private recovery copies were
retained. Real PC/phone sync completed. Both games run on the phone, and both
https://domino.driveatbaku.games and https://uno.driveatbaku.games identify that
phone runtime. The desktop games and dedicated desktop UNO tunnel are stopped.

The new UNO table supports hold/drag/drop, tap to lift then tap to play, direct
deck drawing, card movement animations and compact reaction/control disclosures.
Physical Android WebView acceptance used OS touch events and an isolated test
database; a legal card moved to the pile exactly once without viewport overflow.
Desktop browser checks also cover rejected/cancelled drops, Wild color selection,
touch panning, failure rollback and reduced motion. Real Safari/iPhone and longer
background/sleep/tunnel-interruption checks remain. See
[the interaction report](../unogo/docs/table-interaction.md).

## Earlier combined build status — November 16, 2026

**Go Host 2.0.0 (version code 7) packages both DominoGo and UnoGo. This build has
not been installed or tested on a phone.** Earlier Domino-only hardware checks
below are historical and do not validate the combined app.

Each game now has independent Start, Stop, Open, Copy and Share controls, plus
Start both and Stop all. Stopping one freezes its saved clocks and bots while
the other continues. Both games share the existing private account database.
The package remains `games.dominogo.host`; its certificate matches the previous
APK to prepare an in-place upgrade. No database or credentials ship in the APK.

Current build output is `dist/GoHost.apk`, with a byte-identical
`dist/DominoGo-Host.apk` compatibility alias. Current GitHub update checks require
a stable newer `android-v<version>` release containing an uploaded `GoHost.apk`.
Unpublished source changes are labelled honestly. Starting a second game never
stops the first during the update check.

Permanent link settings now accept two hostnames and a shared phone tunnel token.
The tunnel needs DominoGo routed to localhost:3000 and UnoGo to localhost:3001;
remote-managed tunnel routes also need configuring in Cloudflare. A public link
is ready only when its game ID and runtime identity match the phone.

https://uno.driveatbaku.games currently routes to the running **desktop** through
a dedicated `unogo-desktop` tunnel. HTTPS health and the browser page were checked
against the desktop runtime. The existing Domino phone route was preserved.
Move the UNO hostname to the phone tunnel later when phone hosting is intended.
The PC and tunnel must remain running; reboot startup for this new tunnel is not
configured.

Offline sync now includes UNO saves and results. Each saved room and its ordered
seats merge from the same winning host. Conflicting deletion of an active
participant stops sync instead of corrupting seat order. Transient reaction
bubbles do not sync. Upgrade both hosts before syncing the new schema. No phone
sync or ADB connection was attempted for this build.

Build both release browser clients before packaging; see their build scripts.
Run `./android-host/build.ps1` without `-Install` to prepare only, followed by
`python android-host/verify_apk.py`. The verifier writes `dist/GoHost.build.json`.
It checks signing, version, both browser assets, JNI exports, ARM64/16 KB ELF
alignment and absence of embedded private data.

Passed: 29 server tests, 13 sync tests, four Android updater unit tests, Rust
Clippy and desktop independent lifecycle integration. Android instrumentation
tests compile but were not executed. See [build status](docs/build-status.md)
for checksum and remaining phone acceptance.

## Historical Domino-only setup

The APK runs the existing Rust/Axum server and Dioxus browser game **on the phone**. It does not need a running PC. Start opens a public Cloudflare quick tunnel; Stop closes the tunnel and game engine. The foreground notification also has Stop. Game data stays in the app's private storage. Uninstalling the app removes that phone copy.

Build target: ARM64 Android 8 or newer; tested on the connected Galaxy S9 with Android 17. APK is signed with the local Android debug key, so authorized ADB can access app-private files for database syncing. Keep your signing key to install future updates without uninstalling. The APK contains no account database, PC tunnel credentials, or tunnel token. The PC database is transferred privately over the explicitly authorized ADB connection after installation.

## Database sync

Run `./android-host/start-sync.ps1 -InstallStartup` on the PC to enable the background watcher and Windows sign-in startup. `-Stop` stops it and removes its startup shortcut. The paired device serial is a parameter. USB and already-connected wireless ADB are supported; merely joining the same Wi-Fi network does not pair devices. Keep USB debugging authorized. No public database download or admin endpoint is exposed.

Sync runs every 15 seconds when the PC sees the paired phone and **both servers are stopped**. It merges the entire database, including accounts/password hashes, sessions, saved matches, results, friendships, and per-account hidden history. The PC and Android launchers coordinate file locks with the sync process. A currently running match is never rewritten under a live game engine. Live multiplayer sessions do not migrate between hosts: reconnect to the host you intend to use after stopping and syncing.

The first sync copies the PC database only if the phone has no database. Subsequent syncs compare both databases with their last common baseline. The newer `updated_at` wins for conflicting saved matches; PC wins equal-date ties. A record deleted since the baseline stays deleted. History deletion in the game is a per-account hide, not global deletion for the other participants; that hidden status syncs. Match statistics are rebuilt from distinct tracked completions plus legacy totals to avoid double-counting. Separately allocated integer friendship/invitation IDs are preserved when they represent different records. Schema mismatches, duplicate account names, and failed integrity checks stop the sync instead of replacing valid data.

Recovery copies and the baseline are in `%USERPROFILE%/.codex/domino-phone-sync/<serial>/`. Do not delete the baseline: it records shared history and deletions. Recovery copies contain private account data. Android also retains `database-before-sync.sqlite` privately. SQLite's backup API includes committed WAL data; databases are never copied by reading a live `.sqlite` file alone. If a transfer is interrupted, the next pass retries from the unchanged baseline.

Emote usage is saved per account and syncs as unique use records, so offline phone/PC activity merges without losing uses or counting them twice. The helper applies the additive PC table migration and accepts an older baseline once both apps have the new table.

## Permanent public link

Quick tunnels create a new random address on restart. For a fixed link, create a dedicated Cloudflare named tunnel with a public hostname routed to `http://localhost:3000`, then enter that hostname and tunnel token in **Permanent link settings**. Credentials are stored only in the phone's private settings and passed to cloudflared through its environment. Do not use a token for a PC tunnel serving another application. Cloudflare setup/account/domain is not included.

This phone was provisioned with a dedicated `dominogo-phone` tunnel at **https://domino.driveatbaku.games**. It uses the existing account's domain and does not alter the existing `driveatbaku` tunnel. The credentials were transferred privately after installation; other installations of the APK start with quick tunnels until configured.

## GitHub check before hosting

Every tap on Start checks the public repository's `main` branch and its latest published release before starting the engine. If nothing newer is published, hosting starts automatically. New commits show their latest summary and a GitHub comparison link; they are not silently treated as an installed update. A stable release tagged `android-v<version>` with an uploaded `DominoGo-Host.apk` asset is offered when its version is newer than the installed app. View update opens its GitHub release page. Installing an updated APK remains an Android user action; it must have the same package ID and signing key to retain this installation's database and tunnel settings.

GitHub failure, offline access or rate limiting offers Retry or Start installed. Cancel leaves the engine stopped. Last-check details remain visible in the app. Checks run directly from the phone, without the PC or an embedded GitHub token. This repository currently has no published Android release; new native Rust/server code needs a newly built APK, rather than downloading Rust source to execute on the phone.

## Self-update from GitHub releases

From 2.3.0, a newer published release is installed by the app itself:

1. **Start** checks GitHub (as before). The latest release must be tagged
   `android-v<versionName>`, be published (not draft or prerelease) and carry
   two assets: `GoHost.apk` and `GoHost.apk.sha256`. Without the checksum file
   the app only offers the GitHub page, as in earlier versions.
2. The **Updating Go Host** popup downloads the APK with progress. The app
   rejects it unless its SHA-256 matches the published file, its package is
   `games.dominogo.host`, its version code is newer, and its signing
   certificate equals the installed app's. Android enforces the signature too.
3. Running games are saved and stopped cleanly, and the app records which
   games to restart (`files/resume-after-update.json`, valid for 15 minutes).
4. Android's PackageInstaller installs the APK. **The first self-update asks
   once** to allow "Install unknown apps" for Go Host and shows Android's
   confirmation, because earlier versions were installed by ADB. After Go Host
   has installed itself, Android 12+ applies later updates without a prompt
   (`USER_ACTION_NOT_REQUIRED`, plus update ownership on Android 14+).
5. After installation `MY_PACKAGE_REPLACED` starts the recorded games again.
   Cancel, a failed download or a failed install keeps the installed version
   and restarts any games that were stopped for the update.

Publishing: bump `versionCode`/`versionName` in `app/build.gradle`, run
`./android-host/build.ps1`, then
`./android-host/publish-release.ps1 -Notes 'What changed'` with
`$env:GITHUB_TOKEN` set to a token that has `contents: write`. The script
verifies the APK, writes the checksum, uploads both assets to a draft and only
then publishes it, so phones never see a half-uploaded release. `-DryRun`
validates without publishing. Updates are signed with the same key as every
earlier APK (currently the local Android debug key); keep it safe and backed
up, because a different key can never update this installation.

## Build

Prerequisites: Rust with `aarch64-linux-android`, Android SDK API 35 extension 15/build-tools 35.0.1, NDK r27d (`ANDROID_NDK_HOME` can override), JDK 21, Python, and Go 1.27.4 for cloudflared. `ANDROID_HOME` selects the SDK. Build the game's release browser client with the repository's `build.ps1 -Release` first.

Run `./android-host/build.ps1 -Install -Serial <adb-serial>`. It outputs `android-host/dist/DominoGo-Host.apk`. This is a direct-install personal APK, not a Play Store release. Browser assets and native binaries are generated and excluded from Git. The ARM64 tunnel binary is built from upstream cloudflared commit `b0297f0b24c60a4cef743b7b878e2da3798cb839` (Apache-2.0); `-CloudflaredSource <path>` can use an existing checkout. The Rust engine is linked for 16 KB pages. The browser client is identical to the PC release build.

Checks: `python -m unittest discover -s android-host -p test_sync.py -v` and `cargo test --manifest-path rust/Cargo.toml -p domino-server`. Hardware testing also verifies the public URL, server restart, notification stop, and real two-way database transfer/deletion.
