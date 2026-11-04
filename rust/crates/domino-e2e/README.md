# Network acceptance checks

This Rust executable tests a real running server through HTTP and Socket.IO WebSocket packets. It never reads SQLite or bypasses authentication.

Run the server against a temporary database, then run:

```powershell
$env:DOMINO_E2E_URL = 'http://127.0.0.1:3010'
cargo run --manifest-path rust/Cargo.toml -p domino-e2e
```

Use an isolated test server: the checks create accounts and matches. The harness uses plain local HTTP/WebSocket connections and does not require browser automation or a JavaScript runtime.

It verifies guest account upgrade and password login, session revocation, lobby validation and capacity, host permissions, room membership, private hands and market contents, reconnect restoration, legal turn enforcement, a complete match against a bot through 101 points, match history, and exactly-once statistics. Every received game state checks tile conservation and board continuity. A match has a 20-minute progress limit; socket silence has a 45-second limit. Nonzero exit indicates failure.

The default run also verifies two-human private views, the actual 30-second turn watchdog, fresh/resumed deadlines, unanimous stop votes, host-only continuation, friendships, online presence, game invitations, kicking, host transfer, and simultaneous duplicate turn submissions. To retest these systems without replaying the full bot match:

```powershell
cargo run --manifest-path rust/Cargo.toml -p domino-e2e -- --systems-only
```
