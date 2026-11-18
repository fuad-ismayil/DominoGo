# Go Host

## Platform
Android, ARM64, Android 8 or newer.

## Purpose
Host DominoGo and UnoGo from one phone app without a PC. Both use the same
private account database while retaining separate games and statistics.

## Workflow
Each game has Start, Stop, Open, Copy and Share controls. Start both and Stop all
are available. Stopping one freezes its saved clocks and bots while the other
stays available. A foreground notification exposes Stop all. Each Start checks
GitHub; review and installed-version fallback do not interrupt the other game.
A stable newer GoHost.apk is offered for normal Android installation.

## Links and storage
Quick tunnels are separate per game. Permanent settings accept two hostnames
and a dedicated shared phone tunnel token. Links must identify the current
phone runtime before appearing ready. No database or credentials ship inside
the APK. Offline sync requires authorized ADB and stopped engines; each UNO
save and its ordered seats merge together. Existing package and signing identity
are retained to prepare an in-place upgrade.

## Validation boundary
Go Host 2.1.2 is installed in place on the Galaxy S9. Real PC/phone sync, both
phone listeners, permanent public routes and a physical Android WebView card
drag passed. Desktop lifecycle, sync, updater and static APK checks also pass.
Extended background/sleep/tunnel-loss checks and real iPhone/Safari interaction
verification remain. The desktop server stays stopped while the phone hosts.
