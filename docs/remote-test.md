# Remote test link

Current test URL (2026-11-03): https://poster-rogers-cos-output.trycloudflare.com

Double-click **Start DominoGo** on the Windows desktop to start the engine and public tunnel and display the current link. The shortcut runs `Start DominoGo.sh` through Git Bash, which invokes the project's `start-public.ps1`. It reuses a healthy server and launcher-managed tunnel. Closing the launcher window leaves both running.

From a terminal, run `powershell.exe -NoProfile -ExecutionPolicy Bypass -File ./start-public.ps1`. This uses the existing release build and database. Build changed source separately with `./build.ps1 -Release`.

The latest URL is also saved in `C:/Users/Fuad/.codex/domino-remote-test/url.txt`. Public HTML, leaderboard HTTP and the WebSocket opening handshake were verified on 2026-11-03. The desktop Bash command was exercised successfully.

This temporary Cloudflare quick tunnel forwards to the Rust game at http://127.0.0.1:3000, including its authenticated WebSocket connection. The page and live gameplay were verified through this public address.

The computer must remain awake and online, with the Rust server and cloudflared running. The address lasts only for this tunnel process; restarting a quick tunnel normally creates a different address. This is a test link, not permanent hosting.

The tunnel uses an isolated configuration and does not modify the existing named Cloudflare tunnel. Its process ID, URL and logs are stored under `C:/Users/Fuad/.codex/domino-remote-test/`.
