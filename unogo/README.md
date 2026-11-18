# UnoGo

Implementation begins here, following [the game design document](unogo-gdd.md).
The rules engine now connects to the shared DominoGo host. UNO tables use the
existing account/session/friend store, persist private rounds, and record their
own results. The Navy Rust browser application now supports private tables,
invites, complete rounds, account controls, friends, history and the leaderboard.
A terminal playground is also available.

From this directory:

```powershell
cargo test --workspace
cargo run -p uno-core --example play -- 4
```

The optional number selects 2–10 seats. You play seat 1 against bots. The
playground prints commands and playable cards. Add `uno` to a play to arm the
declaration; Wild cards also require `r`, `y`, `g`, or `b` for their color.
The playground processes elapsed deadlines when you submit input.

## Play in a browser

From the repository root:

```powershell
.\unogo\build.ps1
.\unogo\run.ps1
```

Open `http://localhost:3001`. Create a table, add bots or open the invite link in
another browser, then start the round. Invitations always stop in the waiting
room. Existing DominoGo credentials work; each origin has its own sign-in.
The run script also serves the existing compiled DominoGo browser on port 3000
when available. Both listeners use the existing shared account database.
Do not start a second host against the same ports. For an isolated preview:

```powershell
.\unogo\run.ps1 -UnoPort 3101 -DominoPort 3100 -DatabasePath .\unogo\target\preview.sqlite
```

This is a development browser build. `build.ps1 -Release` creates an optimized
build in `target/dx/uno-client/release/web/public`; set `UNOGO_STATIC_DIR` to
that folder when launching the shared server directly.

## Implemented browser

- Familiar entry, home, setup and waiting-room composition in Navy, with the
  same local fonts, glass controls and account authority.
- Guest-first links, 2–10 seats, host controls, bots, code/link copying and
  selectable fallback; registered friends can receive UNO table invitations.
- Touch and keyboard card selection, draw/keep, staged Wild colors, explicit
  UNO arming/recovery/catches and +4 responses with private challenge evidence.
- Server deadlines, offline control locks, bounded requests, stale snapshot
  protection, paused-game recovery and one-time results.
- Readable scrolling hands, compact ten-seat opponents, four languages,
  keyboard focus, reduced motion, sound/volume and fullscreen.
- Shared friends and reaction usage; UNO-only history, completed-history hiding,
  statistics and UTC calendar-period leaderboards.

## Friends playtest rules (November 17, 2026)

New rounds allow ordered same-number groups, an optional remaining-hand swap on
0, and an everyone-tap-center challenge on 5. Grouped 0s/5s resolve once after the
whole selection. Keep is the default after 15 seconds; the 5 challenge uses a
shared ready countdown and a 6-second response window. The server orders taps,
rejects duplicates and assigns just one penalty; missing players cannot freeze
the round. Existing saved Classic rounds keep their original behavior.

Tap same-number cards directly to select a row; no Group button is needed.
At least one selected card must match the discard by color or number. A different
number replaces the selection. Move a card earlier if needed, then Play or drag
the selected row to the discard. The last card sets
the color. Arm UNO explicitly when the selection leaves one card. Single cards
still support lift/second tap, keyboard and dragging. Draw/keep lives on the
smaller deck; emotes are always visible. The account language selector reflects
the current language on every reopening.

See the dated [design amendment](unogo-gdd.md) and
[playtest verification](docs/friends-playtest.md).

## Implemented rules foundation

- Classic 108-card deck with unique physical IDs, random dealer, opening actions,
  two-player rules, reverse direction, skips and non-stackable penalties.
- Voluntary one-card drawing, drawn-card-only play and keep decisions.
- Atomic Wild/color submission; challengeable +4 declarations, private pre-play
  evidence, successful/failed challenges and provisional final-card victory.
- Atomic UNO declaration, recovery/catch windows and UNO-before-penalty ordering.
- Saved phases and deadlines; pause/resume, timeouts, pile recycling,
  insufficient-pile notices and no-progress no-contest results.
- Per-seat private views and a bot policy that consumes only those views.
- Per-seat monotonic command sequences and transactional rejection of invalid
  actions. Replays cannot execute an accepted action twice.
- Deterministic simulations and an operating-system-seeded production constructor.

## Host integration contract

`Game` contains secrets and is for trusted persistence only. Send `Game::view(seat)`
to an authenticated table member. Return `Receipt` only to the command sender:
challenge evidence must never enter a room broadcast. The engine returns card
counts but never other players' hands in a view. The current rules version is
`classic-108-single-v1`.

The host must serialize mutations, bind authenticated identities to seats, assign
each match a fresh identity, and reject commands for another match. Sequences are
strictly increasing **within a seat and match**, starting at 1. Invalid commands
do not consume a sequence; clients may retry with a newer sequence after syncing.
Duplicate responses require a confirming snapshot rather than replaying an action.

Supply monotonic milliseconds, invoke `tick` before commands and on scheduled
deadlines, and persist accepted mutations atomically. `rebase_clock` restores the
remaining decision time after a process restart.
Pause-vote permissions and bot scheduling belong to the host; `pause` and `resume`
only freeze and restore engine timing. Finished matches must be accounted for
once by the shared database, not by the engine or browser.

## Shared server

From the repository root:

```powershell
cargo run --manifest-path rust/Cargo.toml -p domino-server
```

The shared process serves DominoGo on port 3000 and the UNO API on port 3001.
Both use `DOMINO_DB_PATH` (default `database.sqlite`). Set `DOMINO_PORT` and
`UNOGO_PORT` to change the listeners. `UNOGO_STATIC_DIR` selects the compiled
UNO browser directory. `run.ps1` supplies the development build automatically.

See [the server contract](server/README.md) for endpoints, privacy, recovery,
known boundaries and verification commands.

## Next integration steps

1. Finish remaining release parity: active-table hiding, pagination, abandoned
   waiting-room cleanup, completed-hand review and swipe navigation.
2. Finish extended phone acceptance: longer simultaneous/background hosting,
   sleep and tunnel-loss recovery, plus real iPhone/Safari card gestures.

UnoGo is served from the phone at https://uno.driveatbaku.games. The combined
Go Host 2.1.2 APK is installed in place, with successful real PC/phone sync and
both public games verified against the phone's runtime identity. The desktop
server is stopped. Physical Android touch-drag acceptance passed in an isolated
WebView test; real iPhone and extended background acceptance remain.
UNO source lives here; the shared host loads `server/mod.rs` and depends on `crates/uno-core`.
Existing DominoGo games and statistics retain their original tables.

## Verification

From the repository root:

```powershell
cargo test --manifest-path unogo/Cargo.toml --workspace
cargo test --manifest-path rust/Cargo.toml -p domino-server --lib
cargo check --manifest-path unogo/Cargo.toml -p uno-client --target wasm32-unknown-unknown
cargo build --manifest-path rust/Cargo.toml -p domino-server
python unogo/tests/live_host.py
python unogo/tests/browser_flow.py
```

The real-browser check requires Playwright (`python -m pip install playwright`)
and an installed Microsoft Edge. It uses isolated accounts, database and ports,
verifies complete rounds and social flows, and saves desktop/phone screenshots
under `docs/screenshots`. The large 40-card case uses a conserved fixture only
in the test database; no privileged test endpoint ships.
See [browser design and validation](docs/browser-design.md).
