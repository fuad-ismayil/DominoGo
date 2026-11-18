# 5-lik (Telefon)

This is the group's agreed variant, confirmed on 15 November 2026. It does not
claim to be the universal Azerbaijani ruleset.

- Two to four players, individual scores, seven double-six tiles per player.
- First round must open with 2–3 and awards five points. Redeal if it is in the
  market. Later rounds start with the previous round winner and any tile.
- Add all exposed ends after every placement. A positive multiple of five
  awards that entire total immediately. An exposed double counts both halves.
- **Any** double connected on both normal sides can open either of its two
  sideways branches, only when that placement scores a positive multiple of
  five. Unused side ports count zero. Once opened, a branch extends normally;
  extensions do not have to score. A double within a branch follows the same rule.
- Emptying a hand wins the round. Other hands are summed, with a lone 0–0
  counting ten, and the total rounded to the nearest five. No pending points.
- A full lap of legal passes blocks the round; lowest hand wins, table order
  breaks ties, and the winner receives the rounded sum of the other hands.
- Existing table draw settings apply: two per draw for two players, one for
  three, no market for four. Drawing retains the turn. Passing requires no
  legal placement (including branches) and no drawable market tiles.
- First to at least 365 wins immediately, including during a round. No 35-point
  instant win, partnership scores, 300-point restriction or simultaneous moves.

## Implementation

Telefon uses stable chronological tile IDs and explicit parent/port connections
in the saved `telefon` array. Child port 0 is its incoming connection; port 1 is
its continuation. Doubles also have side ports 2 and 3. The board array contains
the same tiles chronologically; it is not interpreted as a linear 101 chain.
Missing topology defaults to empty for old 101 saves. No database schema change
is required; the existing lobby `game_mode` and saved JSON hold the selection.

The core calculates legal placements, scoring and bot choices. Both the client
and server use the same functions. End payloads retain `"left"` / `"right"` for
101 and use `{"telefon":[nodeId,port]}` for Telefon. DOM drag targets use stable
`telefon-N-P` tokens. Each side port can be used once, and a child only refers
to an earlier node, so the graph is always a tree.

The Telefon board packs matching halves edge to edge, bends chains around
other branches, and automatically fits the whole board. It uses the same
32-unit portrait / 34-unit desktop nominal tile size and shrink-to-fit behavior
as 101, with no connector lines, scrolling or manual zoom. Packing tries
readable tile sizes as hard bounds and reserves branch controls before extending
the chain. It tries both control-first and chronological packing orders before
using a last-resort contact layout for a dense junction. Long-row regressions
require wrapping and a readable tile scale. Latest-tile and
round-finishing highlights remain visible. Rules, feedback and new controls are localized
in English, Azerbaijani, Turkish and Russian. Reactions share one core allowlist.

## Verification

Layout regressions include 16-, 21- and 28-tile valid linear trails with internal
doubles. They require visible turns and a minimum readable scale on portrait
phones and desktops, alongside contact, overlap and endpoint-control checks.

Core tests cover 300 opening deals, multiple internal doubles and both side
ports, rejected branches, continuation without scoring, exposed-double totals,
mid-round victory, blocked rounds, round bonuses, serialization and 60 complete
seeded bot matches across 2–4 players. Existing 101 regressions remain in place.

`domino-e2e --telefon-only` plays a full match over real HTTP/WebSocket clients,
checks private hands, immutable mode, reconnect, history, winner statistics,
rematch and every supported emote. Run against an isolated test database.

The Android host must be rebuilt with the new browser assets and native library.
Installing the signed update over the existing app preserves its private data;
uninstalling removes that copy. Source changes alone do not update the phone.
