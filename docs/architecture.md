# Architecture and rules

## Boundaries

The Rust workspace has a pure rules crate, authoritative server, browser client, and an executable integration harness. Network payloads use camelCase JSON for compatibility with saved accounts and game states. The browser only receives its own hand; opponent counts and market counts replace private tiles.

The server serializes match mutations, persists authoritative state in SQLite, and schedules bots and deadlines against the current match. Stale deadlines cannot advance a newer turn. Session revocation stops both incoming actions and outgoing private snapshots. Existing SHA-256 password hashes are upgraded to salted Argon2id after successful authentication.

Finished matches record statistics once. Rematches archive public summaries under separate identities before resetting the active table, preserving earlier results and each player's independent history visibility.

## Game rules

Two to four players receive seven tiles from a shuffled double-six set. First-round opening follows the lowest-double priority 1–1 through 6–6, then 0–0; later rounds begin with the previous winner. Play attaches a matching value to an open end. The creation-only `drawRule` table setting chooses voluntary drawing (`anytime`, the default) or drawing only without a legal move (`when_blocked`). It is persisted in the lobby and authoritative match snapshot, retained through rounds and rematches, and never accepted from move payloads. Passing still requires no legal move. Drawing takes up to two tiles for two players, one for three players; four-player games have no market. Drawing retains the turn. Passing requires no legal move and no drawable market.

A player empties their hand to win a round. Consecutive passes by all players block the round; the lowest hand total wins, with table order resolving ties. A lone blank double counts as ten. In 101 mode, the winner collects the opponents' remaining pips. First points remain temporary until consecutive wins take the total above twelve, then become permanent. Losing clears temporary points. Once a player has permanent points, later wins bank directly. Reaching 101 permanent points wins the match.

Bot decisions use only the acting player's hand and public board information. The strategy weighs pip disposal, doubles, and follow-up options. Random deals use operating-system entropy; deterministic seeds support repeatable simulations.

## Browser interface

All application behavior is Rust compiled to WebAssembly. Standalone CSS handles layout, focus states, reduced-motion preferences, and small screens. The full-screen emerald table uses a fitted snake layout, compact opponent capsules and an anchored dimensional ivory hand. Captured pointer dragging supports mouse and touch, with a lifted DOM ghost and legal endpoint feedback. Tile selection followed by an endpoint activation remains available for tapping and keyboard input. Countdown updates are isolated from table rendering. Menus use locally served original fonts, clear glass controls, and phone navigation capsules. Recorded tabletop foley is decoded once into a WebAudio mixer with gesture unlocking and persistent mute/volume. Transport mutations are rejected while disconnected; acknowledgments are bounded and room events carry table identities to discard stale updates.

Dioxus/wasm-bindgen generate browser JavaScript bindings in the web build. There is no handwritten JavaScript/TypeScript application, package manifest, npm command, React client or Node server in the project.

## Verification

Core tests exercise rule regressions, scoring, deal invariants, and hundreds of complete simulated matches. Server tests cover privacy, persistence, migrations, bot scheduling, timers, password migration and history archives. `domino-e2e` exercises a running server over HTTP and WebSocket, including a complete match. Browser layout and interactions require separate visual checks.

Original source and earlier planning files are preserved outside the project in `C:/Users/Fuad/Desktop/dominogo-before-rust` and `C:/Users/Fuad/Desktop/dominogo-retired-stack`. The existing local database is preserved.


Table invitation links use the current page origin/path with `?join=TABLEID`, so links copied from the public test address retain that address. Startup consumes the invitation parameter, restores the current account or creates a guest for a first-time visitor, and calls the existing authenticated, serialized join operation. Successful visitors enter the waiting room; joining never starts a game. The server counts bots and humans toward capacity and returns `TABLE_FULL` or `TABLE_STARTED` for unavailable tables. Invite failures land on the main menu with a focused, dismissible dialog; existing members can rejoin their own occupied seat without duplicating membership. Link codes are restricted to seven ASCII letters/digits and duplicate join parameters are rejected.
