# Shared host integration

The existing `domino-server` binary owns both listeners and one SQLite database.
The implementation lives in `unogo/server`; the existing host loads this module.
The UNO listener merges the same account and friend route handlers used by
DominoGo. It never creates a second identity store. Separate browser origins
still require separate sign-in; automatic cross-origin sign-in is not promised.

## Storage and boundaries

`schema.sql` is an additive, idempotent migration applied by the existing database
opener. Existing DominoGo tables remain the authority for DominoGo. New
`uno_tables` and `uno_results` have a fixed `game_id='unogo'`. UNO seats reference
the shared users, while bots are table-local seats without user accounts.
Triggers prevent code collisions between the two games. A join through the wrong
listener returns `WRONG_GAME`.

UNO room mutations execute within SQLite transactions under the same connection
lock as existing database operations. Table membership, host permissions and
session validity are checked before each accepted mutation. The client cannot
select the game, acting seat or authoritative state in its command body.

Snapshots contain only the authenticated member's hand, public counts and state.
Raw `state_json` must never be sent to a browser. The challenge receipt is returned
only to the command sender. All UNO API responses carry `Cache-Control: no-store`.

A 250 ms watchdog resolves deadlines and schedules bots with a 1.5 s delay. Bots
consume only their private view. Idle turns are checkpointed at least once per
second. Startup rebases remaining deadlines from the last checkpoint; host
downtime does not consume decision windows. Paused games retain their remaining
deadlines and do not execute bots. Recovery may restore up to one extra second
after an abrupt process failure, matching the checkpoint interval.

Completion and participant result rows are committed atomically with the finished
state under a unique match ID. Statistics derive from those rows, so retries and
reloads cannot add duplicate wins. No-contest rows have no win/loss. Rematches
receive new IDs; archived summaries survive rematches and table deletion.
History hiding is per user and does not affect statistics. Stale guests in active
or paused games are protected; expired completed guests follow the shared guest
cleanup without deleting another participant's archived result.

## Endpoints on the UNO listener

All routes except health and account creation/sign-in require the existing
`Authorization: Bearer TOKEN` session. Reuse `/api/auth/*` and `/api/friends*`
for account and friendship operations.

| Method | Route | Body / purpose |
|---|---|---|
| GET | `/api/health` | Game and rules version |
| POST | `/api/lobbies` | `{ "maxPlayers": 4 }`, supports 2–10 |
| POST | `/api/lobbies/{code}/join` | Join or recover one's existing seat |
| GET | `/api/lobbies/{code}` | Private snapshot; poll while viewing the room |
| POST | `/api/lobbies/{code}/leave` | Leave waiting room; live games keep the seat for recovery |
| POST | `/api/lobbies/{code}/add-bot` | Host, waiting room only |
| DELETE | `/api/lobbies/{code}/seats/{slot}` | Host removes a seat, waiting room only |
| POST | `/api/lobbies/{code}/start` | Host; at least two occupied seats |
| POST | `/api/lobbies/{code}/commands` | Match ID, per-seat sequence and engine action |
| POST | `/api/lobbies/{code}/pause` | Match ID; vote, all humans required |
| POST | `/api/lobbies/{code}/cancel-pause` | Match ID; withdraw one's pending vote |
| POST | `/api/lobbies/{code}/resume` | Match ID; host resumes a paused game |
| POST | `/api/lobbies/{code}/rematch` | Finished match ID; host starts a new round |
| POST | `/api/lobbies/{code}/invite` | `{ "userId": "friend-id" }`; accepted friends, waiting rooms only |
| GET | `/api/invitations` | Own pending UNO invitations with game destination |
| DELETE | `/api/invitations/{id}` | Decline one's invitation |
| GET | `/api/reactions` | Shared usage and original reaction order |
| POST | `/api/lobbies/{code}/reactions` | Unique `requestId` and supported `emoji`; successful-send deduplication |
| GET | `/api/matches/history` | Own active rooms and visible completed summaries |
| DELETE | `/api/matches/{matchId}` | Hide one's completed history entry |
| GET | `/api/stats/{userId}` | UNO wins, losses, games played and win rate |
| GET | `/api/leaderboard?period=all` | UNO-only day/week/month/all; UTC boundaries |

A snapshot includes `id`, `gameId`, `matchId`, `localSeat`, `acceptedSequence`,
`players`, `status`, `pauseVotes`, `inviteUrl` and `game` (the private engine view).
`players` is in engine seat order; `slot` is the waiting-room removal identifier.
Actions refer to the unique card ID in `game.hand`, never its screen index.

Example command:

```json
{
  "matchId": "the-current-match-id",
  "sequence": 1,
  "action": { "Play": { "card": 42, "color": null, "uno": true } }
}
```

Ordinary cards use `color: null`; Wilds specify `Red`, `Yellow`, `Green` or
`Blue`. Other actions include `Draw`, `Keep`, `CallUno`, `CatchUno`, `AcceptFour`,
`ChallengeFour`, and `{ "ChooseColor": "Blue" }` for an opening Wild.
Stage Wild selection locally and submit the card and color together. Arming UNO
locally sets `uno: true` on the next play; the server never declares for a human.

Successful commands return `{ "table": SNAPSHOT, "receipt": RECEIPT }`. Each
authenticated seat starts sequences at 1 per match. On a lost response, fetch a
snapshot and compare `acceptedSequence` before retrying. An already accepted
sequence returns a conflict and never executes again. Match IDs prevent old-tab
commands from affecting a rematch. Polling currently supplies updates; Socket.IO
push subscriptions will be added with the browser integration.

## Verification

From the repository root:

```powershell
cargo test --manifest-path unogo/Cargo.toml --workspace
cargo test --manifest-path rust/Cargo.toml -p domino-server --lib
cargo clippy --manifest-path rust/Cargo.toml -p domino-server --all-targets -- -D warnings
cargo build --manifest-path rust/Cargo.toml -p domino-server
python unogo/tests/live_host.py
```

The black-box test starts the real binary on two temporary ports with a temporary
database. It verifies cross-listener credentials, guest upgrade identity, shared
friends, wrong-game joins, a complete two-session round, restart/rejoin, rankings,
session revocation and a concurrent DominoGo table. It terminates its own test
server and removes its temporary files afterward. It does not open the real
account database or interfere with an existing running host.

## Remaining release work

The Navy browser client, four-language UI, shared friend operations and UNO
invitations/reactions now connect to these endpoints. HTTP activity contributes
to friend presence with a short expiry. Active-history hiding, abandoned waiting
room cleanup, swipe navigation and richer completed-hand review remain.
History is currently capped at 100 active and 100 completed entries per request;
pagination is not implemented. The leaderboard is a casual win-count list.

Independent Start/Stop controls now freeze and resume each game's saved timers
and bots without stopping the other game. Android Go Host packages both clients
and the shared engine. The sync helper understands UNO saves and merges each room
with its ordered seats from the same winning host. Upgrade both hosts before
syncing this schema; an old phone build cannot host UNO.

Desktop lifecycle integration tests pass. Go Host 2.1.0 is installed in place on
the Galaxy S9, with real PC/phone sync and both public routes verified against
the phone runtime. Physical Android touch acceptance used an isolated WebView
and temporary server/database. Extended background/sleep/tunnel-loss checks and
real iPhone/Safari verification remain.

For optional desktop control, set `GO_HOST_CONTROL_PORT` and a private
`GO_HOST_CONTROL_TOKEN` of at least 32 characters. The authenticated control
listener binds only to loopback: GET `/status`, POST `/games/dominogo` or
`/games/unogo` with `{ "enabled": true }` or false. It is never part of either
public game router. `GO_HOST_INITIAL_GAMES=none` starts both games disabled.
Health responses identify `gameId` and the shared runtime `hostId`, allowing the
Android link check to reject a public address routed to a different host.
