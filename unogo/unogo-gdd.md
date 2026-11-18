# UnoGo — Game Design Document

**Version:** 0.2 — design draft for review; Navy theme selected  
**Date:** 16 November 2026, Asia/Baku  
**Platforms:** mobile and desktop browsers; one shared Android hosting APK  
**Public addresses:** `uno.driveatbaku.games` and the existing `domino.driveatbaku.games`  
**Status:** planning document. UnoGo, the combined host and the new hostname are release requirements, not completed implementations.

## 1. Product definition

UnoGo is a private-table, multiplayer card game for the same friends who already
play DominoGo. A DominoGo player should recognize the entry screens, account
controls, table setup, invitations, friends, history and leaderboard immediately.
The card game needs its own engine and gameplay interface; its surrounding
product should remain the same family of application.

The first release has one format: **Single round**. The first player to empty
their hand wins after any pending final-card effects have been resolved. There
is no cumulative match score or 500-point target in this release.

### Confirmed requirements

| Decision | Contract |
|---|---|
| Identity | Shared accounts and friends with DominoGo |
| First release | Single-round games only; further formats can follow later |
| Pre-game experience | Preserve DominoGo's layout, navigation, interaction patterns and relevant features |
| Visual difference | Navy theme with primary accent `#98d0ff`; retain the recognizable glass controls and typography |
| Hosting | One Android APK manages both games on the existing phone |
| Updates | GitHub check before hosting, source-change review and published APK update links |
| Public UNO address | `https://uno.driveatbaku.games` |
| Rules | A named, documented Classic UNO baseline; digital timing policies explicitly identified |

### Remaining review decisions

- **Repository:** recommended initial approach is expanding the existing
  `fuad-ismayil/DominoGo` repository into a shared codebase. A family repository
  can replace it if the owner prefers. Freeze the actual repository and release
  asset configuration before shipping the updater.
- **Host display name:** working name **Go Host**. Keep the existing Android
  application identity for a safe in-place upgrade.

These decisions do not reopen the confirmed single-round format or shared
account requirement.

## 2. Audience and release boundaries

Primary users are a small group of friends on phones, including players who have
only played UNO occasionally. Secondary users play in desktop browsers or use
bots to learn. The product must explain legal actions without requiring the
player to know every card beforehand.

Release one supports **2–10 seats**, including mixed human/bot tables. This
extends DominoGo's seat-count control using the same styling. Default selection
is four seats; the host can start once at least two seats are occupied. Official
Classic instructions identify the game as supporting 2–10 players. [R1]

The first release excludes cumulative scoring modes, teams, stacking, jump-in,
7–0 hand swapping, custom cards, wagering, public matchmaking and competitive
rank certification. There is no forced tutorial, advertising flow or new
pre-game navigation structure.

## 3. DominoGo parity contract

"Identical before gameplay" means reuse the existing screen structure and
behavior, with the following explicit adaptations. Do not replace it with a
new dashboard or an unrelated card-game lobby.

| Area | Carry over | UnoGo adaptation |
|---|---|---|
| Entry | Guest name, Play as guest, sign-in, registration, language picker | UnoGo branding and theme; card-related introductory copy |
| Guests | Temporary account, inactivity expiry, save-account upgrade | Same global user ID survives upgrading; no separate UNO-only guest database |
| Home | Greeting, Create a table, Join with a code, shortcuts, Your tables | UNO history and statistics only on UnoGo's home |
| Account sheet | Identity, language, sound, volume, password reset, sign-out and sign-out-other-devices | Same account authority across both games; sound descriptions refer to cards |
| Account statistics | Wins, losses, win rate and completed games | Remove average/banked points from UnoGo; do not invent a replacement points currency |
| Table setup | Seat selection, create button, fixed-settings explanation | Seats 2–10; fixed Classic / Single round summary; no one-option mode dropdown |
| Drawing setting | Preserve the place where fixed table rules are explained | Remove DominoGo's free-drawing switch: Classic voluntary drawing is fixed for this ruleset |
| Waiting room | Seat rows, host label, empty seats, add/remove bots, remove player, host-controlled start | Same layout; scale the seat list to ten |
| Joining | Existing seven-character code pattern and clear unavailable/full/started errors | Validate the game associated with a code |
| Invite links | Copy code, copy link, selectable fallback, guest-first joining | Links use `https://uno.driveatbaku.games/?join=CODE` |
| Friend system | Requests, accept/decline/cancel/remove, presence, table invitations | One shared friend graph; invitations identify the destination game |
| History | Active/paused/completed tables, rejoin/results/rematch, per-user hiding | UNO entries and UNO results; hiding does not erase another player's history |
| Leaderboard | Same presentation, day/week/month/all-time filters, swipe navigation, retry/empty/loading states | UNO-only counts and tie-breakers; never mix DominoGo scores into UNO |
| Reactions | Same emotes, bubbles and successful-send tracking | Shared account preferences; most-used first, original order for ties |
| Settings | Existing language selector, fullscreen, sound and volume controls | Same four languages: Azerbaijani, English, Turkish and Russian |
| Connection handling | Reconnect status, bounded acknowledgements, saved-table recovery | Restore UNO-specific pending decisions and legal controls |
| Pausing | Vote/request/cancel/continue patterns and saved games | Freeze the full UNO decision state, not only whose turn it is |

A direct invite may create a guest and join the waiting room. It never starts a
match automatically. Rejoining one's own seat must not duplicate membership.

### Shared account semantics

- One identity store owns user IDs, password hashes, sessions, friendships and
  emote-use history. Reusing source code with two unrelated account databases
  does not satisfy this requirement.
- Existing DominoGo credentials work in UnoGo. Existing friends appear without
  re-adding them. Password resets and session revocation have account-wide scope.
- The two browser origins still have separate browser storage. Release one
  promises **shared accounts**, not automatic cross-domain sign-in. A registered
  user can sign into each site with the same credentials.
- A guest who needs to return on another origin/device should save the account
  first. Do not silently associate unrelated guest sessions by device or IP.
  Cross-domain guest handoff/SSO can be added as a separately designed feature.
- UI language and sound controls keep their existing behavior. Emote preferences
  are account-backed. Game results and leaderboards remain game-specific.

## 4. Rules baseline

### Edition choice

Use **Classic 108-card UNO**, with single-round victory as the selected format.
Mattel also publishes 112-card editions with additional cards; their own
instructions distinguish the classic deck from optional additions. Release one
does not include Swap Hands, Shuffle Hands or custom cards. [R1, R3, R4]

No authoritative Azerbaijan-only UNO standard was verified in this research.
Therefore, localization into Azerbaijani does not introduce unverified regional
rules. Any later house-rule mode must be named, versioned and visible before a
table starts.

### Core card and turn contract

The deck has 108 physical cards: four colors, each with one zero, two of each
1–9, and two each of Skip, Reverse and Draw Two; plus four Wild and four Wild
Draw Four cards. Deal seven cards per player. Match color, number or action;
an ordinary Wild chooses the next color. A player may draw one instead of
playing. After drawing, only that new card may be played; otherwise keep it and
end the turn. Draw Two forces two cards and skips its recipient; Skip skips one
seat; Reverse changes direction. Penalties are not stackable. [R1]

For two-player play, Reverse and Skip return play to their sender. After the
opponent completes a draw penalty, play also returns to its sender. [R2]

### Opening card and Wild Draw Four

An opening Skip skips the first seat; opening Draw Two gives that seat its
penalty; opening Reverse starts with the dealer in the reversed direction.
An opening Wild lets the first seat choose a color. An opening Wild Draw Four
is returned and a replacement is drawn. [R3]

Wild Draw Four is legitimate only without a card matching the current color;
matching a number/action alone does not disqualify it. Its recipient may
challenge. A guilty sender draws four instead. An unsuccessful challenger draws
six; acceptance means drawing four and losing the turn. [R3]

### UNO declaration and completion

A player reaching one card must declare UNO. Being caught before the next
player begins their turn incurs two cards. When a final Draw Two or Wild Draw
Four ends a round, its draw effect still resolves. The draw pile can be rebuilt
from discarded cards when exhausted. [R4]

### UnoGo implementation decisions

These specify online behavior; they are not claims that a printed rulebook
defines networking, timers or touch interaction.

| Situation | Required UnoGo behavior |
|---|---|
| Dealer | Choose a random dealer on each new single-round match; keep seat order visible. This replaces physical high-card dealer selection. |
| Card identity | Every physical card has a unique ID, including duplicate printed cards. Commands use IDs, not only color/value. |
| One-card play | One card per accepted play action. Matching duplicates cannot be discarded together. |
| Choosing a Wild color | Stage the selection locally, then commit card ID and color together. Cancelling the chooser does not discard the card. |
| Last card | Any card type can finish the round; no number-card-only restriction. Wait for mandatory effects before finalizing the result. |
| Draw decision | Highlight only the newly drawn card during the play/keep decision. Existing hand cards cannot be substituted. |
| Wild Draw Four declaration | Support the challenge mechanism rather than making every challenge mechanically futile. A possibly illegal declaration is accepted into a pending challenge state; the sender sees an own-hand warning before committing. |
| Challenge evidence | Record legality and the relevant hand **before** the declaration. If challenged, show the required hand evidence only to the challenger, never to the whole table. |
| Successful challenge | Sender draws four; challenger receives a normal turn. Keep the declared color. These continuation details are the frozen digital interpretation for v1. |
| Failed/declined challenge | Challenger/recipient draws six/four respectively, then loses the normal turn. No stacking opportunity is offered. |
| Finishing Wild Draw Four | Keep a provisional winner until the response/penalty is resolved. A single-card final Wild Draw Four has no other matching-color card to make it illegal. |
| Declaring UNO | With two cards, the player can arm the UNO button; the next play carries that declaration atomically. It is never automatically declared for a human. |
| Missed UNO | After an undeclared reduction to one card, open a server-timed two-second response window. The previous player may recover by calling UNO; another player may catch them. First accepted valid response wins. |
| Catch timing | The next actionable phase starts after that window closes/resolves. No catch is accepted before the reduction or afterward. This short grace window is a disclosed digital adaptation. |
| Combined effects | Resolve the UNO response window before opening a Wild Draw Four response or advancing through a forced penalty. A later UNO penalty does not change the saved Wild Draw Four evidence. |
| Recycling | Keep the current discard and active color; shuffle the buried discards into a new draw pile. Never create replacement cards or expose the new order. |
| Insufficient cards | Draw all available cards, recycling when possible. If no further card can exist, finish that penalty with the available cards and show a pile-empty notice. This rare-case policy is a digital exception, not a general reduction of penalties. |
| No-progress exhaustion | A full cycle with no play and no possible draw ends as No contest. Record no win/loss; preserve the history entry. Do not invent a winner from card values. |

### Timers and automation

- Normal turns: retain DominoGo's 30-second turn-clock pattern.
- A color choice and a play/keep choice consume the existing turn deadline;
  opening a chooser does not restart the clock.
- Wild Draw Four response: eight seconds; timeout accepts the four-card penalty.
- UNO response window: two seconds, then the next phase begins.
- Normal-turn timeout: draw one and keep it; if already deciding about a drawn
  card, keep it. Forced penalties resolve automatically. No automatic bluff.
- Pause freezes deadlines; resume establishes fresh remaining deadlines. Bot
  delays and response windows must not run behind an open pause.

The timers are initial design values and must be tested on the phone host with
real mobile latency. They belong to the saved rules version, not client-local
timers that can disagree.

## 5. Gameplay interface

The gameplay screen keeps DominoGo's familiar match bar, seat identity,
connection indicator, clock, table menu, reaction area and results pattern.
The central board and hand are purpose-built for cards.

### Table and hand

- Center: draw pile, top discard, active color and play-direction indicator.
  Use both text/symbols and color; Wild cards must not obscure the active color.
- Opponents: names, avatars, remaining-card counts, turn highlight, UNO state
  and reaction bubbles. Show no live opponent card faces.
- Local hand: readable cards with visible value/action and redundant color
  symbols. Highlight ordinary legal plays; clearly distinguish a challengeable
  Wild Draw Four declaration from a normal match.
- Keep useful card sizes on phones. A large hand scrolls horizontally; it must
  not shrink 40 cards into an unreadable strip. Support the worst valid hand
  size, not only the initial seven.
- Use tap-to-select/tap-to-play and keyboard controls. Drag-to-discard may reuse
  DominoGo's pointer capture and acknowledgement behavior, with one clear
  discard target rather than left/right chain endpoints.
- A submitted card stays pending until acknowledgement or a confirming snapshot.
  Repeated taps cannot duplicate a play. Errors restore the actionable hand.
- With 7–10 players, opponent seats become compact rows of name/count badges;
  the draw/discard area and local hand remain readable. Do not copy four-seat
  geometry and squeeze ten players into it.

### Action area

Use **Draw**, **Play drawn card / Keep card**, **UNO!**, and context-specific
**Accept +4 / Challenge** controls. Do not carry over DominoGo's always-visible
Pass button. Reactions stay in the familiar horizontally scrollable strip,
ordered by the shared account's usage.

The rules disclosure uses short examples: matching a red 7, keeping a drawn
card, declaring UNO and challenging +4. No mandatory onboarding interruption.

### Results

Show winner, other players' remaining-card counts, and a readable finishing-card
highlight. Completed hands may be reviewed face up, following DominoGo's
round-review convention, but only after all pending decisions are closed.
Show **Play again** for the host and the existing wait/return patterns for
others. A rematch gets a new match ID and cannot overwrite the prior result.

## 6. Theme and visual consistency

Keep the current menu hierarchy, principal history plane, local
Jakarta/Syncopate font family, glass capsules/circles, familiar account sheet,
focus treatment, spacing and phone navigation. Replace domino-specific marks
and nouns with original UnoGo card-themed equivalents.

**Selected theme: Navy**, chosen from the visual comparison on 16 November
2026. Preserve the preview's palette and DominoGo's familiar menu composition:

| Role | Color |
|---|---|
| Main field, gradient top | `#183f70` |
| Main field, gradient bottom | `#07172f` |
| Primary accent and primary action | `#98d0ff` |

Burgundy and Copper are unselected alternatives, outside the first-release
theme. The card colors remain
recognizable red, yellow, green and blue regardless of menu theme. Use original
card artwork and an original UnoGo wordmark; do not rely on copied commercial
art assets or imply an official publisher affiliation.

Minimum acceptance: no phone navigation covering actions; meaningful controls
at least 44 CSS pixels where practical; keyboard focus; readable localized
strings; color-independent card recognition; reduced-motion support; sound that
unlocks after a user gesture and respects saved mute/volume settings.

## 7. Statistics, leaderboard and history

- UnoGo Single round: one winner receives one win; other participating humans
  receive one loss when the round completes. Bots do not receive public accounts
  or leaderboard positions. Abandoned/paused/no-contest games award nothing.
- Count completed games once, transactionally, under a unique match ID. Reload,
  retries, reconnect and rematch must not duplicate totals.
- Preserve DominoGo's casual leaderboard layout and four reporting periods.
  For UnoGo sort by wins, then win rate, then completed games, then a stable
  identifier for deterministic ties. Explain its casual nature; no skill-rating
  or club claims.
- Preserve the current UTC reporting boundaries for parity. Do not silently
  reinterpret existing DominoGo records in another timezone.
- UnoGo history, leaderboards and account game statistics use `game_id=unogo`.
  Existing DominoGo statistics stay attributed to DominoGo. No conversion of
  101/Telefon points into UNO wins or a head start on a new game's leaderboard.
- Emote usage and friends are intentionally account-wide. Game results are not.

## 8. One APK, two games

### Host model

Deliver one shared Android hosting app, provisionally **Go Host**. Prefer an
in-place upgrade of `games.dominogo.host`, signed by the existing key, with a
monotonically higher Android version code. A display-name change must not
require uninstalling the existing app or removing its private data.

The host provides one row per game with status, Start, Stop, Open game and Share
link. Include Start both / Stop all as explicit combined actions, common tunnel
status, database-sync status and the GitHub check/update section. Its foreground
notification identifies which games are active and can stop them safely.

| Service | Address / responsibility |
|---|---|
| DominoGo listener | Preserve local port 3000 and the existing public hostname |
| UnoGo listener | Local port 3001 and `uno.driveatbaku.games` |
| Shared Rust host | One runtime owns identity, friends, storage and lifecycle; game engines have separate cancellation scopes |
| Shared database | One authoritative SQLite database in the app's private storage |
| Public tunnel | Prefer one dedicated named tunnel with two ingress routes; credentials remain private |

Stopping UnoGo must not kill the process, tunnel or timers needed by DominoGo.
Stopping DominoGo must not interrupt an active UnoGo match. The current
single-game process-exit-on-Stop implementation therefore needs a deliberate
lifecycle refactor, not a second Start button wired to the same global kill.

Keep the shared host alive while either game is enabled. A disabled game's
listener can show a simple offline response while the other game remains
available. Stop all closes both engines, releases database locks and allows
offline sync. A tunnel reconnect never resets saved matches.

### Domain and resource requirements

- Route the UNO hostname to port 3001; keep DominoGo's existing route intact.
  Extend the dedicated game tunnel, not an unrelated DriveAtBaku service.
- Do not embed tunnel tokens, account databases or signing secrets in the APK.
- Both games must operate within the existing phone-hosting approach; a paid
  application server is not a v1 dependency.
- Verify a normal four-player table in each game running together for at least
  30 minutes on the paired phone. Also test a ten-seat UNO table, backgrounding,
  reconnect, device sleep and tunnel loss. Capacity claims must follow tests.
- The phone remains the availability limit: if it is stopped/offline, the
  corresponding public game is unavailable. Show honest status in the host app.

## 9. GitHub request and update flow — required

Every host **Start** action checks the configured repository's main-branch
revision and latest applicable stable Android release. Starting a second game
must not stop the first while this check runs.

| Outcome | Host behavior |
|---|---|
| No newer source/release | Start the installed game normally |
| New commits, no installable release | Show the summary and comparison link; explain that source commits are not an installed APK update; offer Start installed |
| New compatible shared-host APK | Show installed/new version and release notes; link to the verified release asset |
| Offline, timeout or rate limit | Offer Retry, Start installed and Cancel; preserve any game already running |
| Changed/diverged source | Show the comparison honestly; do not claim the phone is current or silently downgrade it |
| Installation requested | Save/stop **both** games before installing a shared APK; the user performs the Android installation step |

Requirements:

- Use the configured public repository without embedding a GitHub access token.
  The default repository recommendation remains subject to the decision in §1.
- Build metadata records source revision, app version and supported schema/rules
  versions. A development build cannot pretend to be a published clean revision.
- Validate release tag, version, asset name and repository-owned HTTPS links.
  Reject an older APK and unrelated releases.
- Keep the existing `android-v<version>` tag convention unless a migration is
  explicitly designed. Publish a shared `GoHost.apk` release asset. For the
  first transition, also publish the same binary under the legacy
  `DominoGo-Host.apk` name so current installed hosts can discover it.
- Preserve install-over-update signing identity, database and tunnel settings.
  Never instruct users to uninstall as the normal update path.
- Release work includes committing/pushing the shared source, running CI,
  publishing the versioned APK and verifying the phone's served browser build.
  These are future implementation/release tasks; this GDD does not publish them.

## 10. Shared implementation and migration plan

Use Rust for authoritative game behavior and the browser application, following
the existing stack. Organize by responsibilities:

- `domino-core`: retain the current 101 and Telefon engines.
- `uno-core`: deck, legal actions, turn phases, challenges, UNO windows, winner
  resolution, deterministic test shuffles and fair-information bot policy.
- Shared product modules: identity, sessions, friends, invitations, preferences,
  room membership, acknowledgements, history and leaderboard presentation.
- Shared host/server: game-scoped routers and broadcasts, lifecycle management,
  persistence, Android bridge and public-tunnel supervision.

Avoid representing cards as domino tiles or adding UNO conditionals throughout
the domino engine. Reuse the account and room contracts while giving each game
a separate typed state and renderer.

### Storage and protocol

- Add a game identifier to new tables/matches/results and default legacy entries
  to DominoGo. Preserve existing user IDs, password hashes, sessions, friend
  relationships, match IDs, hidden-history flags and saved 101/Telefon games.
- Namespace rooms and timers by game and table. Globally unique seven-character
  table codes preserve the current user experience while avoiding ambiguity.
- A code belonging to the other game receives a clear destination-game message;
  it must not join silently through the wrong site's router.
- The listener/router determines game scope; client payloads cannot override it.
- Persist rules version, unique card IDs, private hands, draw/discard piles,
  active color, direction, dealer, turn owner, pending phase, response deadlines,
  UNO declaration state, challenge evidence, provisional winner and completed
  result. Reconnect must restore decisions, not invent a fresh turn.
- Serialize accepted mutations. Validate identity, membership, phase, turn,
  card ownership and command identity at the server. Replayed commands cannot
  draw/play twice or award two wins.
- Broadcast per-user views. Normal snapshots expose only one's own hand, public
  discard/color/direction, opponent counts and public decisions. Challenge
  evidence has a specific authorized recipient and bounded lifetime.

### Phone/PC sync

Retain the authorized USB/wireless-ADB workflow and recovery copies. The shared
database syncs only while **both games are stopped on both hosts**. One game's
Stop cannot unlock a database still used by the other. Merge existing account,
history, match and unique emote-use records without duplicate statistics.
Advance the common baseline only after a successful validated update of both
copies. Never replace valid account data with an APK-bundled database.

## 11. Bots and behavior boundaries

Bots use the same state transitions as humans. Their decision input contains
their own hand and public information; never the draw order, another hand or
the server's saved Wild Draw Four legality bit.

They should prefer useful legal plays, choose a Wild color based on their own
remaining hand, declare UNO and resolve draw/keep decisions. Challenge decisions
use public history rather than hidden evidence. Start with one understandable
bot policy; difficulty levels and deliberately imperfect personalities can
follow later. Bots must finish simulations for every supported player count.

Use concise feedback for draws, skips, reversed direction, UNO calls, catches,
challenges and completed rounds. Animations and sound reinforce authoritative
events and cannot delay controls after the server considers an action available.

## 12. Delivery sequence and acceptance gates

| Stage | Deliverable | Gate before proceeding |
|---|---|---|
| Design freeze | Theme choice, rule version and repository/update configuration | Owner reviews these explicit decisions |
| Shared foundation | One identity/friend store, game-scoped stats and host lifecycle | Existing DominoGo accounts, games, links and updates still work |
| Rules engine | Complete Single round state machine and private views | Rule examples, edge cases and full simulated games pass |
| Browser integration | Existing menus plus UNO gameplay | Guest invite to finished game works on phone and desktop |
| Combined host | Installable shared APK and both public routes | One game can start/stop independently without losing the other |
| Release candidate | Signed APK, GitHub release flow, data migration and recovery | Real phone/PC checks and dual-game soak test pass |

### Required checks

1. Deck construction and conservation: all 108 IDs stay accounted for through
   dealing, penalties, recycling, pause/restart and rematch.
2. Every starting action card, two-player behavior, ten-seat turn order and
   Reverse/Skip behavior at the ends of the seat list.
3. Voluntary drawing, drawn-card-only play, keep/end-turn and duplicate commands.
4. Legal/illegal Wild Draw Four declarations, accept/successful/failed challenge,
   private evidence and correct turn continuation.
5. UNO pre-arm, timely recovery, valid/invalid catch, two simultaneous catch
   requests and combined UNO/+4 response windows.
6. Final action cards, provisional victory, no-progress exhaustion and one-time
   win/loss accounting.
7. Bots for 2–10 seats, hidden-information isolation, and bounded legal progress.
8. Existing-account login on both sites, guest upgrade, shared friends,
   account-wide session revocation and persistent emote order.
9. Table links, full/started/wrong-game rooms, own-seat rejoin, host permissions,
   pause/resume, network loss and server restart in every pending phase.
10. UNO-only leaderboard/history, UTC period boundaries, rematch archives and
    per-user history hiding without modifying DominoGo records.
11. Phone and desktop layouts, all four languages, color recognition without
    color alone, keyboard play, very large hands and reduced motion.
12. APK install-over-update, data recovery, offline sync, independent game
    controls, both-hosts-stopped locks, both public addresses and GitHub failure,
    source-only change, newer release and rejected downgrade paths.

Acceptance means a friend can open an invite, play a full round, understand why
an action is available, reconnect without losing the game, see the result once,
and come back with the same account and friends. A separate DominoGo table must
remain playable throughout normal UnoGo use.

## 13. Later releases

Potential additions, deliberately outside v1: cumulative 500-point matches,
clearly labeled house-rule presets, teams, additional official deck variants,
cross-domain automatic sign-in, richer bot difficulty and broader matchmaking.
Do not expose unfinished mode selectors or market the casual leaderboard as a
ranked competitive system. New rules must keep old saved games interpretable.

## 14. Sources and local references

Official rule facts above are summarized from named editions; product behavior,
timers, hosting and architecture are UnoGo design decisions.

- **R1:** [Mattel Classic 108-card instructions, W2085](https://service.mattel.com/instruction_sheets/W2085-UNO.pdf).
- **R2:** [Mattel instructions with explicit two-player rules, 42001](https://service.mattel.com/instruction_sheets/42001.pdf).
- **R3:** [Mattel GDJ85 instructions](https://service.mattel.com/instruction_sheets/GDJ85-Eng.pdf): opening actions and Wild Draw Four challenge reference; edition also contains optional additional cards.
- **R4:** [Mattel FFK04 instructions](https://service.mattel.com/instruction_sheets/FFK04-Eng.pdf): classic-deck distinction, UNO declaration and end-of-round handling.
- Current product references: `DESIGN.md`, `docs/architecture.md`,
  `docs/future-clubs-and-competitive-rankings.md`, `android-host/README.md`,
  the current Rust menu/account/lobby/friends/leaderboard screens and shared
  emote-usage implementation.

The GDD preserves the owner's decisions. Its technical proposals and explicitly
marked digital edge policies should be reviewed during the first rules prototype;
they must not be silently changed by whichever implementation is easiest.


## November 17, 2026 — Friends playtest amendment

This user-approved amendment supersedes the original single-card/no-house-rule
requirements for newly started rounds. Existing saved Classic rounds retain
`classic-108-single-v1`; new rounds use `friends-108-number-batch-zero-five-v2`.
The deck remains 108 cards and each table plays one round.

- **Same-number groups:** a player may select several physical numbered cards in
  an explicit order. The first must be legal on the current discard; every card
  must have the same number. The last sets the active color. Groups are atomic;
  action cards cannot be grouped. After drawing, only that newly drawn card may
  be played. A group leaving one card still requires an explicit UNO declaration.
- **0:** after the full play/group, offer one optional exchange of the player's
  remaining hand with one opponent, or Keep. Never auto-swap. After 15 seconds
  without a choice, Keep applies. A final 0 wins without an empty-hand exchange.
- **5:** after the full play/group, one shared 1.2-second ready countdown opens
  the center-card tap challenge to everyone. The server records arrival order;
  the last responder draws one random card from the shuffled deck. Ignore
  repeated/stale taps. The 6-second window cannot freeze on a disconnected
  player: if anyone misses it, randomly select one nonresponder for the single
  penalty. A final 5 wins only after that penalty is resolved.
- **Grouped specials:** one optional swap or one tapping challenge after the
  entire group, confirmed by the user. UNO catch/recovery resolves first.
- **Table interface:** polished high-contrast cards; translated center text fits
  phone/landscape layouts; smaller offset draw deck; tap-to-lift/tap-again-to-play
  and hold/slide remain available; no floating Draw/Play menu. Emotes remain
  visible, swipe horizontally as needed, and retain usage-based ordering.
- **Account settings:** the language selector reflects the active language every
  time the account sheet is reopened, including after reload.

All special phases, tap order and deadlines persist across host recovery and
pause/resume. Bots use private player views and join tapping independently of
whose turn it was. Network arrival determines order; the interface does not
claim synchronized physical reaction measurements across different connections.


### November 17 follow-up: direct number selection

Remove the separate Group mode/button. Tap a card to lift it; a second tap on
that single card still plays it. Tapping another card adds it only when the
selection forms a valid same-number row. Otherwise the new card replaces the
selection. A selected multi-card row may be played with Play N or dragged onto
the table; tapping a selected member removes it. If removal leaves an unmatched
row, keep only its last remaining selection. Escape clears the selection.

A number row is legal when at least one owned, selected card matches the current
discard's color or number. The matching card need not be selected first. Preserve
chosen order, including reordering, and use the final card's color. All cards
must share the number; no matching card means no group. Drawn-card-only play and
Classic-save restrictions continue to apply. This supersedes the earlier first-
card matching requirement. Grouped 0/5 effects still resolve once.
