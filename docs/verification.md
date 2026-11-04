# Port verification

## Swipeable leaderboard periods — 2026-11-03

Replaced pill-style period buttons with a single underlined tab strip and added touch/pen horizontal paging over the list. Today is the initial tab, followed by This week, This month, All time; swipe left advances, right returns, with no wrapping at either end. Tapping remains available. Horizontal intent requires 12px initial motion and a 1.7:1 horizontal/vertical ratio. Commit distance is 20% of panel width, bounded to 56–100 CSS pixels. Vertical/ambiguous first motion locks out paging for the rest of the gesture; small drags, cancellation, lost capture and secondary pointers cannot commit a page. Browser-native pan-y and pinch zoom remain enabled; no preventDefault blocks scrolling. Pointer motion updates only the panel transform rather than rerendering all rows. Modest drag resistance and a directional entrance provide feedback; reduced-motion disables transitions.

Validation: all 28 client tests pass, including deliberate swipes, vertical/diagonal locking, distance thresholds, wrong-pointer rejection and cancellation. Strict WebAssembly Clippy and optimized browser build pass. Static scan returns no findings with its documented coverage limits. Public HTML matches the rebuilt client. Browser/physical-phone swipe testing remains unavailable because browser access is disabled, so thresholds are reasoned initial tuning rather than measured device ergonomics.

## Club wording deferred; casual leaderboard periods — 2026-11-03

Saved the competitive-club investigation in docs/future-clubs-and-competitive-rankings.md with status deferred until player population, random matchmaking and abuse policy support competition. No club entities, membership features or competitive scoring system were implemented. Removed the home membership badge and live club/member copy across home, account, authentication, navigation, invitations and leaderboard. CSS names remain internal implementation details. New copy is translated in all four languages.

Leaderboard retains its wins/win-rate/score ordering and current eligibility. Today, This week, This month and All time use /api/leaderboard?period=day|week|month|all. Default remains All time. UTC calendar periods and Monday weeks are explicitly labeled. Lifetime totals are retained; newly recorded dated results begin at the stored trackingStartedAt timestamp, with no fabricated historical backfill. Completed matches have stable per-match identities and lifetime/dated completion writes are atomic and rollback on error. This accounting supports casual date filters, not a claim of fair competition.

Validation: all 68 workspace tests pass; strict native workspace/all-targets and WebAssembly client Clippy pass; optimized server/client builds pass. Tests cover midnight/month/week/leap/year bounds, half-open filtering, legacy migration preservation, rollback/retry and duplicate completion. Isolated --systems-only network acceptance passes real 30-second expiry, private views, reconnect/deadline, pause/resume, social invitations, host permissions and duplicate move handling. Test server was stopped. The live database was backed up using SQLite online backup and integrity-checked before migration. The updated release was restarted against the same database and tunnel. Public HTTPS checks pass all four filters and reject invalid periods; release HTML matches the served page. Static design scan returns no findings with its usual Rust coverage limits. Fresh rendered/physical-phone verification remains unavailable because browser access is disabled.

## Browser and home-screen icons — 2026-11-03

Added an SVG favicon, multi-size ICO, 32px PNG favicon, 180px Apple touch icon, 192/512px application icons and a 512px maskable icon derived from the existing ivory/gold domino emblem. The opaque emerald background and inset emblem leave room for launcher masking. The custom Dioxus index template declares icons, manifest, application name and theme color before WebAssembly loads. The manifest uses a clean root start URL so installed shortcuts do not retain invitation codes. build.ps1 copies the icon assets and manifest on future builds.

The emblem's 512px raster was visually inspected. PNG dimensions were checked, the optimized browser build passed, and public HTTPS checks returned 200 with correct MIME types for the page, manifest and every declared icon. Generated HTML contains the manifest and touch-icon links; the manifest parses and declares all three Android icon entries. The static design scan returned no findings. Physical-phone installation remains untested; an existing home-screen shortcut may need removing and re-adding to replace its cached icon. This adds installation metadata, not offline gameplay.

## Latest-play and round-ending highlights — 2026-11-03

The running release server predated lastPlayedTile/roundFinishingTile tracking. It was replaced with a freshly compiled release and restarted against the existing database; the tunnel address was retained. The server binary's hash matches the verified replacement build. All 12 server tests pass, including both-end tracking, clear-on-deal, and the added blocked-round case: a round ending by passes marks its last actual placement.

The client now uses a persistent gold face tint and 1.25px screen-space rim for the latest tile. Round-ending tiles use a stronger teal tint and 2px screen-space rim. Non-scaling strokes retain visibility when the board shrinks, and the brief confirmation tint settles to a persistent highlight. Reduced-motion keeps the static marks. The release browser build and WebAssembly compilation pass; the design scan reports no findings, with its documented coverage limitations. Rendered color/motion verification remains unavailable because browser access is disabled.

## Single placement sound — 2026-11-03

An opponent's placement that handed the turn to the local player triggered both the placement sound and the separate turn notification. The turn notification now requires unchanged board length, so placements produce one tile sound regardless of who played. Pass-driven handovers retain the turn cue. Round/match outcome sounds remain distinct from move notifications.

## Gameplay feedback pass — 2026-11-02

The client now previews a selected tile at legal endpoints, including logical pip orientation and reversed snake rows. A submitted tile remains at its target with its rack source subdued until an authoritative snapshot resolves it. Repeated/unrelated snapshots retain the preview. Rejection or connection errors restore the rack; late acknowledgements cannot clear a newer placement. Off-target pointer drops return toward the rack over 180ms, with cleanup that does not disturb a subsequent drag. Confirmed board pieces settle over 160ms and placement audio is scheduled at 120ms. Reduced-motion CSS disables these transitions and the new arrival animation.

Contextual rack guidance covers the required opening double, later-round opening, matching ends, actual draw quantities capped by remaining market, passing, confirmation, and reconnecting. Round results explain actual pending accumulation, promotion, banked additions, and local pending loss when a same-round prior snapshot is available. Rejoined results fall back to observable collected/pending/banked totals. All new text is localized in English, Azerbaijani, Turkish, and Russian. No rules or server state schema changed.

Validation: 63 workspace tests passed (25 client, 26 core, 12 server). New tests cover endpoint orientation, pending reconciliation, opening/draw/pass guidance, score arithmetic and insufficient-history fallback; localization coverage and placeholder tests pass. WebAssembly compilation and strict WebAssembly Clippy pass. The optimized Dioxus browser build passes. The running local server returns the new release index plus its generated JavaScript, WebAssembly and recorded placement sound with HTTP 200. The mechanical design scan returns `[]`, with the previously documented Rust/Dioxus coverage limitation.

Browser access was attempted but remains disabled in Work settings. Fresh rendered, audio, latency, keyboard and physical-touch verification of this pass was therefore not possible. No new visual screenshots are claimed. The previous screenshot fixtures cannot verify these new interactions. Production accounts and scores were not used for testing, and no server restart or database migration was needed.

Verified locally on Windows with Rust 1.99.1, Dioxus CLI 0.7.10 and Chromium browser automation.

- Core: 23 tests, including 432 complete seeded simulated matches and 150 deal checks. Tile uniqueness, conservation, connected chains, legal actions and match termination are asserted.
- Server: eight tests for private state, schema migration, bot draw chaining, human deadlines, statistics idempotence, password migration, and rematch archives.
- Client: seven control/layout tests, including the final market tile, opening double rules, ownership/end matching, no-overlap bounded snake geometry for counts 0–30 across device ratios, and a short-chain sizing regression. WebAssembly compilation is checked explicitly; native compilation alone does not validate the browser code.
- HTTP smoke: guest session, profile, lobby, bot addition, match start, membership, history, leaderboard and logout pass.
- Live acceptance: complete 101-point match, 246 updates, approximately 184 seconds. Account upgrade/login, session revocation, reconnect, private-hand restoration, illegal actions, round transitions, final statistics, rematch archives and independent hiding pass.
- Two-human systems: real 30-second turn expiry, fresh deadline, private snapshots, pause consensus, host-only resume, friend request/accept/presence, invitation acceptance, kicking, host transfer, friend removal and simultaneous duplicate move submissions pass. Exactly one duplicate move succeeds.
- Browser: production guest access, two/four-player lobby creation, bots, start, draw, selection, placement, pause, reload and rejoin verified. Phone widths 320 and 390, tablet 768, and desktop 1440 checked without horizontal page overflow. Screenshots are in `docs/screenshots`.
- Bot comparison: the improved strategy won 691 of 1,024 seeded paired games (67.5%) against the old highest-pip policy. Run `cargo run --manifest-path rust/Cargo.toml -p domino-core --example bot_arena` to reproduce.

## Practical limits

Browser visual checks use Chromium viewport emulation, not a physical iPhone/iPad/Android test lab. This is evidence for the tested layouts, not a promise of perfect rendering on every browser or device. Mobile fullscreen and audio remain subject to browser permission and gesture policies. Multiplayer needs a running server and network connection; offline/PWA play is not implemented.

There are no authored `.js`, `.ts`, `.tsx`, npm manifests, React components or Node server files in the source project. Dioxus/wasm-bindgen produce JavaScript browser bindings in the ignored compiled web output. These load WebAssembly and bridge browser rendering, events and APIs; the game rules, scoring, bots and application behavior are Rust. The user accepted this browser-support boundary provided JavaScript contains no game logic.

Original files were preserved outside this workspace before retiring the old stack; the existing database was retained. CI workflow is provided but has not been run on a remote CI service in this session.

## Restored UI verification

The original compiled interface was run on an isolated comparison server before restoring its table identity. The current browser build has passed actual mouse drag-and-drop at desktop and phone-sized viewports; legal drops advance the authoritative turn, off-target drops preserve the hand, and drag ghosts/capture classes are removed. Tap/keyboard placement remains available. The same Pointer Events path handles touch, including pointer cancellation, capture loss, visibility changes and state changes; physical-device touch testing remains outstanding.

Two- and four-player tables were inspected at 390×844, 320×568 and 844×390, with desktop/tablet checks at 1440×900 and 768×1024. The table fits the viewport with no horizontal page overflow. Every quick reaction remains accessible through a scrollable strip on narrow phones. Menu controls checked include naming a guest, seat selection, adding bots/capacity, copy-code success feedback, account upgrade navigation, pause/return/rejoin, account autofocus, keyboard focus containment and Escape. Audio mute and volume are persisted. No errors or warnings were observed in the production browser console during these checks.

Latest live acceptance: 16 rounds, 358 state updates, approximately 258 seconds, plus the actual 30-second two-human timeout and social/concurrency checks. Join and reaction acknowledgments are explicitly asserted. Native workspace tests: 38 passing; strict native and WebAssembly Clippy checks pass.

Sounds are recorded CC0 foley by Kenney, with licenses and source mapping in `public/sounds/SOURCES.md`. The glass treatment is a CSS interpretation of the requested Apple-style material, with transparency, rim reflections and depth rather than heavy frosted blur; it is not Apple's proprietary native renderer.

The final board observes actual container size rather than polling it. CSS caps prevent initial/rotated short chains from temporarily inflating. Account sheets autofocus and trap keyboard focus, close on Escape, and restore focus. In-game invitations remain above the table; reactions use seat bubbles rather than the error channel. Old table subscriptions are replaced and tagged frames are filtered.

## Connected-chain visual fix and remote test

Uniform tile slots were replaced by actual footprint packing. Ordinary tiles consume two horizontal units and perpendicular doubles one; paired touching vertical pieces bridge wrapped rows without overlapping doubles. No guide lines are needed to conceal physical breaks. Stable tile identities and a short opacity transition preserve clear placement feedback.

The regression suite checks shared-edge contact, non-overlap and bounds through all 28 tiles, using 24 shuffled double-six sets at five device proportions (3,480 staged layouts), plus explicit straight/double and wrapped-turn cases. All 38 native tests and strict native/WASM Clippy passed.

A temporary Cloudflare HTTPS tunnel was verified against the current build. Guest authentication, lobby creation, bot turns and live moves were exercised over the public HTTPS/WSS origin through a 17-tile chain. Desktop and 390x844 phone previews are in `docs/screenshots/connected-chain`; no browser warnings/errors were observed. Existing Cloudflare account/tunnel configuration was left unchanged.

## Score visibility after round one

The live server had retained later-round scores (including 11 pending points), but opponent badges and the round-result/history views rendered only permanent points. The local pending label could also disappear on small layouts. One shared score readout now shows banked and pending values in all these places, with an explicit pending-rule explanation on the result card.

A deterministic five-round regression verifies score retention while dealing, changing winners, pending reset on loss, pending-to-banked promotion and additional wins after banking. Browser verification used a separate database on port3012: 53 banked / 11 pending remained visible from round2 results into round3; the complementary local-pending case remained visible from round4 into round5 at320x568. History pending scores were checked too. Screenshots in `docs/screenshots/scoring` are isolated fixtures, not changes to real match scores. The production database was not modified by this fix. The updated browser bundle is served through the existing remote test link.


## Automatic tables and voluntary drawing

The menu refreshes history and player statistics every three seconds while visible, and when mounted. Polling is scoped to the menu, pauses while the document is hidden, and leaves unchanged data untouched. Responses from a previous account are discarded. In a browser against an isolated database on port 3012, a table created through a separate authenticated request appeared without a click or reload; adding a bot and starting the match then updated the players, status, scores and rejoin action automatically. The console stayed clear.

Drawing is allowed on the current player's turn even when a matching tile is available. Two-player draws still take up to two tiles, three-player draws one, and four-player games have no market. Drawing retains the turn; passing still requires no matching tile and no drawable market. Core and visible-control regressions cover voluntary draws, batch sizes, market exhaustion and ownership. Browser verification drew from 14 to 12 market tiles while retaining a playable 1–2 tile and the same turn.

All 40 native tests, formatting and strict native/WASM Clippy passed. The network acceptance suite completed a six-round 101 match with 164 state updates, including a successful voluntary draw, then passed history/statistics/rematch, two-human timeout, privacy, social and duplicate-submission checks. The optimized server and updated browser bundle are live through the existing remote test URL; local/public HTML match and both return HTTP 200. Tests used a separate database and did not alter real scores.


## Domino emblem and signed-in password reset

The header now uses a scalable overlapping ivory/gold domino emblem with inset pips, edge highlights and depth instead of a font-dependent symbol. Desktop (1440×900) and narrow-phone (320×568) browser previews are saved in `docs/screenshots/account-logo`.

Members can open Your account → Reset password and enter/confirm a new password without providing the old one. The authenticated endpoint hashes with Argon2id, updates only the session owner's account, rechecks session validity after hashing, and atomically revokes other sessions. This device stays signed in. Guests retain the existing Save my account flow. The form has password visibility controls, inline validation/errors, busy handling and success feedback, and fits the scrollable phone account sheet.

The isolated `--password-only` network acceptance run passed unauthenticated/guest rejection, missing/type/length validation, supplied-target isolation, old-password rejection, new-password login, current-session retention, and other-session/socket revocation. These checks also run in the default full acceptance suite. Browser checks passed mismatch feedback and successful reset on the narrow phone layout, with automatic input focus and no console warnings/errors. All 40 native tests, strict native/WASM Clippy and formatting passed. The new optimized server and browser build are deployed through the existing public test URL; both origins return identical HTML, and the live reset endpoint rejects unauthenticated requests with 401. No real user's password was changed during testing.


## Creation-time draw-rule toggle

Create table now offers Free drawing, on by default to preserve current behavior. Turning it off permits drawing only without a legal move. The waiting room displays the choice; in-game settings display it as locked. Four-player setup explains that no market exists. There is no rule-update endpoint. Join/start/move/next-round/rematch payloads cannot override the persisted creation choice.

A lobby schema migration defaults existing tables to `anytime`, and old snapshots without `drawRule` deserialize to the same behavior. New snapshots carry the typed rule; the core engine and visible Draw control enforce it consistently. Next-round dealing retains it, and rematches reload the persisted table setting.

All 44 native tests, formatting and strict native/WASM Clippy passed. The isolated `--draw-rule-only` network suite verified both choices, malformed option rejection, both players' private views, legal/illegal voluntary draws, full rounds, next rounds, reconnects, pause and rematches while repeatedly attempting payload overrides. The suite is included in the default network acceptance run. Browser checks at 320×568 verified toggle interaction, no horizontal overflow, a restricted waiting room, disabled Draw while holding a playable tile and the read-only in-game rule. Screenshot: `docs/screenshots/draw-rule/locked-phone.png`. The optimized server is live through the existing public link, both local/public HTML match, and existing real tables retain their previous drawing behavior.


## Copyable table invitation links

The waiting room now includes “A seat in one tap” below Share your code: a selectable invite URL, Copy invite link button, success feedback and a manual-copy fallback. It uses the page's actual origin/path, so a link copied from the public game points to the public game. Opening a link restores the visitor's account or automatically creates a guest, joins if admitted, and waits for the host. The query parameter is consumed so refreshing does not replay the invitation.

A full table redirects to the main menu and opens the club-styled “All seats are taken” dialog, with focused dismissal, keyboard containment, Escape/backdrop dismissal and focus restoration. Bots count toward capacity. Started, missing and malformed tables have corresponding calm explanations. Normal member rejoining remains idempotent.

All 45 native tests, strict native/WASM Clippy and formatting passed. The isolated `--invite-only` suite passed bot-filled admission rejection, two visitors racing for one remaining seat (exactly one admitted), waiting status, member rejoining, full started tables, started tables with empty seats, and missing tables. These checks are included in the default acceptance run.

Browser verification used an isolated database on port 3012: opening a link automatically entered the waiting room, copying succeeded, a bot-filled table redirected home with the full-table dialog, dismissal returned focus to home, and the invited client entered gameplay automatically after the host started. Desktop and 320×568 phone layouts had no horizontal overflow; no browser warnings/errors were observed. Screenshots are under `docs/screenshots/invite-links`. The optimized server/browser bundle are deployed through the existing public test link; local and public origins return matching HTML with HTTP 200. No real user/table was changed for these tests.


## Clear round review, endpoint geometry, natural draw sound and localization

Round/paused/match results now occupy a separate bottom row. There is no board-covering result layer, dark mask or background blur. The chain remains visible until the host explicitly continues. Decorative opponent backs are omitted during review to give the finished chain more room; remaining hand counts stay visible. Four-player scores use a compact grid, and optional explanations expand inside the bounded result panel. The results and board never overlap.

The old endpoint rectangle exceeded its reserved footprint, causing the opening-double overlap in the supplied image. Marker bounds are now computed by the shared layout model, inset inside their allotted cells, with roughly 1–2 px clearance at normal scale. A native regression covers a single double, ordinary tile and wrapped chain. The existing staged-chain tests still verify contact, non-overlap and bounds. Actual browser measurements on the single-double phone fixture found 1.8 px between rectangle bounds and the tile, and drag/drop onto the corrected marker advanced the authoritative move correctly.

The bazar feedback now uses brief, filtered natural wooden contacts, at most 160 ms, with gentle fades, conservative gain and a 180 ms repetition guard. The chips-rattling sounds are unused and archived outside the source tree. Source/license details are in `public/sounds/SOURCES.md`; the new WAVs are 16-bit mono and publicly served successfully. Global sound/volume preferences remain intact.

Azerbaijani, Turkish and Russian join English across the interface, including game rules, score explanations, accessibility labels, known errors and account/invite flows. There are 397 catalogue entries per language with complete key/placeholder parity. Each target catalogue received a separate wording/terminology review. Language selection is available on the welcome screen, in the account sheet and table settings; it applies immediately and persists on reload. A browser-discovered initial select-value mismatch was fixed with explicit selected options. Details: `docs/localization.md`.

All 48 native tests, formatting and strict native/WASM Clippy passed. Browser checks used an isolated database on port 3012, with two- and four-player round-end fixtures, a single-double playing fixture and a full-table invite fixture. At 320×568, 768×1024, 844×390 and 1440×900 every one of the 24 played tiles stayed within the viewport and above the results; computed board/result backdrop filters were `none`. All four scores fit on the narrow phone. Next round dealt round 3 while retaining banked scores. Bazar draw, mouse drag placement, all three language switches, saved language/selected-option restoration, localized dates/bot names, Turkish setup and Azerbaijani full-table popup were checked. No browser warnings/errors were observed. Screenshots are in `docs/screenshots/round-review`.

The current browser bundle and both new draw assets are live on the existing public test URL. Local/public HTML match with HTTP 200, and both draw WAVs return HTTP 200. No server restart, production-state fixture or real account/score change was needed for this UI update. Physical-device touch/audio perception remains outside viewport-emulation testing.


## Unstarted table lifecycle

Match history now inner-joins game_states, so a waiting room never appears as a played/resumable match. Abandoned waiting rooms with no game state are cleaned after 90 seconds without a subscribed participant, checked every 10 seconds. Any subscribed participant keeps a waiting room alive; bots and a user's separate home-page connection do not. Refresh/reconnect has a grace period. Explicit final-human leave still removes an unstarted room immediately. Started, paused, round-end and completed matches retain their saved state/history.

Cleanup and subscription/start operations are serialized so a start or resubscribe cannot race with deletion. The delete condition rechecks both waiting status and absence of a game state. Foreign-key cascades clear empty-room memberships and invitations. A native regression checks the grace boundary, protection once a state exists, and membership cleanup. The isolated --waiting-only acceptance scenario exercises real time, bot-filled abandonment, active subscription retention, history exclusion before start/inclusion after start, and explicit leaving. Native workspace tests: 49 passing; strict native Clippy passes.
