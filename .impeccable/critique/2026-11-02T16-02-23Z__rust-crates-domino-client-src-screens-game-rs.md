---
target: DominoGo overall game, gameplay feel and UI materials
total_score: 27
max_score: 40
na_heuristics: 
p0_count: 0
p1_count: 3
target_identity: "file:C:\\Users\\Fuad\\Desktop\\dominogo\\rust\\crates\\domino-client\\src\\screens\\game.rs"
target_fingerprint: "sha256:da9ec8dfcd745f414e20c18ca75e8d8867cb5ea274f4542e61666f00f5054e9f"
target_path: "C:\\Users\\Fuad\\Desktop\\dominogo\\rust\\crates\\domino-client\\src\\screens\\game.rs"
timestamp: 2026-11-02T16-02-23Z
slug: rust-crates-domino-client-src-screens-game-rs
---
# DominoGo: game experience investigation

Date: 2026-11-02. Scope: rules, bots, interaction, pacing, visual materials, interface flows, accessibility, multiplayer confidence, and the return-to-play loop. This is an investigation and proposed roadmap; application code and production data were not changed.

## Evidence and limits

Reviewed the Rust core/client/server, CSS, localization, CI, acceptance harness, design contract, and stored desktop/mobile screenshots. Current checks: **52 workspace tests passed**, WebAssembly client check passed, and the existing bot arena reproduced **691/1,024 wins (67.5%)** against its pip-greedy baseline. That benchmark establishes relative strength against one policy, not human difficulty or player enjoyment. The live network acceptance harness was inspected but was not rerun against the user's database.

Browser access was attempted and rejected because local browser use is disabled in Work settings. Visual judgments therefore use stored captures reconciled with source. Some captures predate later fixes: notably the stretched avatar in restored/phone-game.png has a subsequent explicit square-size fix. Do not treat that old shape as an outstanding defect. No fresh audio listening, latency measurement, physical-phone touch test, screen-reader test, or live playtest was completed. Timing and animation suggestions below are starting hypotheses to test.

## Product diagnosis

The rules and multiplayer foundations are substantially stronger than the moment-to-moment presentation. The game has a recognizable private-club identity, credible ivory pieces, clear gold primary actions, fair-information bots, shared legal-move logic, persistence, private hands, social invitations, reconnection, and unobstructed round review. Preserve these investments.

The highest-value direction is to make each action understandable and physically continuous: pick up a piece, see exactly where it fits, commit it, hear its contact, understand the next turn, and understand the score earned. More decorative panels or more modes would leave these gaps intact.

## Independent design assessment

Method: dual-agent (A: /root/design_review; B: /root/evidence_review), with parent gameplay/source investigation. A completed before B's detector findings entered the synthesis. Neither agent edited the project. The following heuristic scores are an evidence-limited design judgment, not measured player satisfaction or a functional certification.

| Heuristic | Score / 4 | Main assessment |
|---|---:|---|
| System status | 3 | Turn, timer, reconnect and busy indicators exist; presentation timing needs live evaluation. |
| Real-world match | 3 | Strong physical table metaphor; pending/banked points need explanation. |
| Control and freedom | 3 | Pause, leave, rejoin and cancellation exist; continuation depends on host. |
| Consistency | 3 | Shared score UI and strong identity; gameplay target sizes differ from menu. |
| Error prevention | 3 | Shared legal gating and busy-state prevention are strong. |
| Recognition | 2 | Collected, pending, banked, hand totals and cumulative scores require interpretation. |
| Efficiency | 3 | Drag, tap, keyboard endpoints, rejoin and invitations are supported. |
| Aesthetic restraint | 3 | Board prominence is excellent; phone greeting and review compete for space. |
| Error recovery | 2 | Alerts and reconnect mechanisms exist; placement recovery lacks physical continuity. |
| Help | 2 | Rules exist in all languages but are dense and insufficiently contextual. |
| **Total** | **27/40** | **Significant usability refinement remains despite a strong identity.** |

Detector: source scan exited 0 and returned `[]`, with no reported rules or locations. The target is Rust/Dioxus source, while the scanner offers no reliable Rust parsing coverage or parsed-file count; CSS is stored separately. Zero findings therefore does not establish clean UI. No detector false positives were reported to adjudicate. Manual source/capture evidence is the useful signal here.

Cognitive load is low at create/join, naturally higher when selecting among a hand of legal tiles, and unnecessarily high at score review. Several simultaneous numerical concepts require reconciliation. Emotional progression is strongest when the table opens and when the completed board remains visible; the likely low point is an unexplained score change or an action without continuous confirmation. These are hypotheses for observed player sessions.

Newcomers need rule-specific guidance and a score transaction. Returning phone players need recent-table access above a long greeting. Players with low vision need larger key information and inspection affordances; keyboard and screen-reader foundations exist but need live verification. These concerns should guide the first player tests.

Questions to carry into playtesting: Can a newcomer explain why the winner banked a different amount from the collected pips? Does a returning phone player find the current table faster with a compact resume action? Does a delayed placement remain understandable without implying that an unconfirmed move succeeded? Prioritization is supplied below under the user's explicit delegation, rather than requiring another approval round.

## 1. Tile placement needs a complete physical sequence — high priority

Evidence: game/drag.rs removes the ghost and source class in finish() before calling place(). screens/game.rs sends a server request and clears selection immediately; the authoritative board and rack update only when the new state arrives. app.rs plays placement sound after comparing received board lengths. game.css gives new board tiles a 160ms opacity arrival, with a separate last-played rim. There is no rack-to-end travel or confirmed landing sequence.

Consequence: under network delay, the dragged piece returns visually to its rack before appearing on the board. That is a source-derived latency risk, not a measured live reproduction. Invalid/off-target drops also clean up immediately without a return animation or explanatory response.

Proposal:

- Model interaction explicitly: idle, selected, dragging, target preview, submitting, confirmed, rejected/cancelled.
- On a valid hover, show a translucent, correctly oriented domino at its prospective position. Preview only information the player is allowed to know.
- Keep a lightweight pending representation at the intended endpoint until the server confirms. Never locally commit official turn, score, or hidden state.
- On confirmation, settle the piece into the authoritative layout and align the contact sound with that landing. On rejection, return it to the rack with a specific explanation.
- Distinguish cancellation from an error: releasing on empty felt should quietly restore the tile.
- Retain tap-tile/tap-end and keyboard placement. Do not automatically pick an end when both are legal; that is a strategic choice.

Starting motion targets: immediate selection response; roughly 120–180ms settling; roughly 160–220ms return. Tune by observation, not by treating these numbers as standards.

Implementation: game/drag.rs, screens/game.rs, app.rs, game/tile.rs, assets/game.css. For resilience, introduce a monotonic state revision/action identifier before expanding presentation events; reject or reconcile pending actions against the corresponding authoritative revision.

## 2. The interface knows the rules but does not explain the present decision — high priority

Evidence: shared legal moves correctly govern enabled tiles and Draw/Pass. The rack caption still says “Drag a glowing tile to either open end” throughout the player's turn. Rules are five dense paragraphs behind disclosure, including opening-double priority, two-tile draws, retained turns, lone-blank scoring, and pending-point banking above 12.

Proposal: derive one short guidance sentence from the same legal state:

| Situation | Guidance |
|---|---|
| Required opening double | “Open with your 3–3.” |
| Multiple legal placements | “Match 2 or 5. Choose a highlighted tile.” |
| No match, market available | “No match. Draw 2 tiles; your turn continues.” |
| Last market tile | “Draw the last tile.” |
| No legal move or market | “No moves left. Pass to continue.” |
| Optional free draw | Keep Play primary; explain Draw as an optional alternative. |
| Submitting/reconnecting | Describe confirmation or reconnection, and prevent misleading new submissions. |

Add a skippable practice round teaching selection, choice of end, drawing, and one concrete score example. Explain unusual rules at their first occurrence. Keep the complete rules available for reference. Do not change established 101 rules as a visual refinement.

The same principle applies to score presentation. Show a round ledger: previous banked/pending amount, collected pips, pending promotion or reset, and new total. Pending scores already exist and are displayed; the missing part is the reason for the change. Example: “8 pending + 7 collected = 15 banked.”

## 3. Pacing is mechanically uniform — high priority

Evidence: ops.rs sets AI_DELAY_MS to 1,500 and reschedules after every bot draw. Three ordinary bot moves therefore add 4.5 seconds of configured waiting before the human gets another turn, excluding networking and rendering. Repeated draws add another 1.5 seconds apiece. Every human turn has a 30-second watchdog; timeout uses a legal bot fallback. The match header says a player is thinking but gives no draw/pass action history.

Proposal:

- Offer a fast bot pace, with shorter delays for forced actions and repeated draws. Preserve enough time to perceive each move.
- Use a restrained action cue: “Leyla drew 2,” “Bot 1 passed,” or a brief hand-to-board movement. A permanently expanding log is unnecessary on the table; a compact recent-actions disclosure is sufficient.
- Announce timeout assistance explicitly, including which tile was played, so automatic action does not look like lost control.
- Offer creation-time casual timer settings or an untimed solo practice option. Multiplayer choices must remain visible and authoritative for all players.
- Display countdowns using a server-time offset and monotonic local elapsed time. Current client Date::now() is compared directly with the server's epoch deadline, so a mis-set device clock can misrepresent remaining time.

## 4. Refine the materials through lighting, hierarchy, and consistency

Keep the approved emerald felt, dimensional ivory, warm gold, local fonts, authored domino emblem, and clear glass controls.

The stored views show pleasant pieces but a large, relatively uniform green field; many secondary surfaces sit in similar dark-green tonal ranges. Some instructions, counts, and score details are extremely small. The short board is intentionally capped at a nominal tile scale; unused space is not automatically a packing bug. Test a slightly larger early-round chain on large displays while retaining room for the full 28-piece chain.

Material direction:

| Material | Proposed role and treatment |
|---|---|
| Felt | Matte foundation; restrained fine texture, a broad light pool, quiet perimeter darkening. Avoid a visibly repeating decorative grid. |
| Ivory | Shared face/edge/pip tokens for CSS rack pieces and SVG board pieces. Consistent light direction, restrained bevel, contact shadow that becomes softer while lifted. |
| Glass | Interactive controls and principal information planes. One coherent rim, tint, and elevation treatment; sufficient text backing. Keep the board visually open. |
| Gold | Primary action, current turn, selected tile, and important score transition. Use distinct shapes/labels as well as color so these roles remain distinguishable. |

Separate material from interaction state: hover should raise a piece, selection should identify it, target preview should show its destination, and confirmation should show completion. Making all these states glow more strongly would obscure their meaning.

Non-playable rack pieces are currently dimmed using brightness(.7), opacity .7, and disabled rules. This also reduces pip readability. Keep them readable for planning and use a small lift/edge treatment on legal pieces. Improve active-player identification with a clear “Your turn”/turn marker and optional progress ring, without relying solely on a gold outline.

Consolidate final CSS rules and expose shared tokens after the interaction work. game.css combines compressed base rules with repeated corrective overrides; club-system.css refines a separate menu system. Preserve their scope while removing contradictory sizing and material definitions. Do not rewrite the stack to achieve this.

## 5. Mobile comfort and feedback settings need a dedicated pass

Evidence after resolving corrective CSS overrides: portrait rack faces are 35px wide inside targets with a 40px minimum width; portrait reactions are 36px wide and 44px high, and Draw/Pass have a 44px minimum height. Landscape rack targets already have a 44px minimum width, while Draw/Pass are 40px high and reactions have a 34px minimum width. Mobile round-review buttons can be 36px high. SVG endpoint targets shrink with board scale. The table menu has a later 44px square fix but becomes 38px in short landscape. There is no universal 44px gameplay target policy; several earlier small-control rules have already been corrected and should not be reported as current defects.

Use at least 44px practical hit regions around primary gameplay controls, even when the drawn object is smaller. Keep endpoint hit regions in screen coordinates rather than allowing the rendered SVG chain scale to define their usability. Collapse reactions behind one comfortably sized control on narrow screens. Preserve rack space for actual play.

Test a large hand: the rack wraps into a capped scroll area while each tile uses touch-action:none. Dragging and scrolling need an intentional gesture policy or a separate scroll affordance; physical-device testing must establish whether this is frustrating.

Also verify scrolling of revealed opponent hands: .opponent-seat sets pointer-events:none, while nested .revealed-tiles uses overflow:auto without explicitly restoring pointer interaction. Keyboard focusability is provided, but touch/pointer scrolling of overflow is a code-backed concern that has not been reproduced live. Restore interaction on the review hand region if testing confirms the obstruction.

Audio already has persisted enable/volume controls and decoded sample playback. It does not need replacement. Haptics, however, are dispatched inside play(sound) after the sound-enabled guard: muting audio also suppresses vibration. Add independent haptic preference and dispatch. Avoid surprising automatic vibration merely because a sound happens.

CSS honors reduced motion in several places. Canvas confetti does not consult that preference, so CSS alone cannot disable it. Make celebrations respect the same preference. The existing match_win() effect has no caller; app.rs uses round_win() for both round and match completion. Give final victory a distinct, restrained ending and preserve the round board for review.

Keyboard endpoint activation, labels, score live announcements, and account focus management already exist. Next checks: changing turn/action announcements, disabled-action explanations, readable focus on scaled SVG controls, zoom/text scaling, and usable inspection of non-playable tiles.

## 6. Reduce setup friction and improve reasons to return

The solo path currently traverses guest entry, home, create, table configuration, bot addition, and Start. Add “Practice now” with sensible defaults while retaining custom/private tables. Offer “Play with these players again” and clear rematch readiness; guests currently wait for the host to advance rounds. A ready indicator should communicate real acknowledgement, rather than labeling any seated guest “Ready to play.” Preserve host authority.

Use personality where it supports recognition: more distinguishable avatars/bot identities, authored tile backs, and a small set of social reactions. Gameplay avatars currently use initials, and hidden racks cap their drawn backs at ten while a separate count carries the actual total. Do not mistake that cap for lost game tiles.

The leaderboard ranks cumulative wins then win rate. record_match_stats excludes bot accounts, but human victories against bots enter the same totals. These are valid club activity statistics, but do not establish competitive human skill. Separate practice results from human competitive results before adding stronger ranking claims. Difficulty tiers can start from the current fair bot, with softer decision quality for easier tiers; stronger play can use public pass history and bounded sampling consistent with visible information. Do not grant access to hidden opponent hands.

Deferred features: public matchmaking, alternate rule sets, cosmetics economy, seasons, spectator/replay interfaces, and offline/PWA mode. These require product decisions and additional systems. None should delay fixing placement, guidance, pacing, and score comprehension.

## Recommended delivery sequence

| Stage | Deliverable | Completion evidence |
|---|---|---|
| 1 | Placement states, oriented preview, confirmed landing, failed-drop recovery, contextual Draw/Pass guidance | Repeated mouse/touch/keyboard turns; delayed and rejected requests; no duplicated or lost tiles; server state remains authoritative. |
| 2 | Bot pace options, clear draw/pass/timeout cues, score-change ledger, practice entry | New players can explain their move and score; compare turn gaps and time to first move before/after. |
| 3 | Material tokens, readable rack, larger hit regions, independent haptics, motion preferences | Phone/tablet/desktop and 2/3/4-player captures, long hands, full chain, four languages, reduced-motion/transparency checks. |
| 4 | Rematch readiness, richer player identity, separated practice/competitive records, difficulty choices | Repeat-play flow and ranking semantics verified; paired seeded bot comparisons by player count and rule setting. |

Evaluate changes with newcomers, experienced domino players, and one-handed phone users. Track time to first legal move, rejected or abandoned drops, blocked-state hesitation, server confirmation latency, bot waiting time, score-explanation success, and rematch completion. Establish a baseline before inventing retention targets. Browser/layout tests cannot establish subjective satisfaction; physical-device play and observed sessions remain necessary.

## Reference principles

The project-specific findings above come from local source and captures. External principles supporting the proposed feedback, control, and learning improvements:

- [W3C: Provide Feedback](https://www.w3.org/WAI/WCAG2/supplemental/patterns/o4p10-status-feedback/): recognizable action/status feedback.
- [Xbox Accessibility Guideline 110](https://learn.microsoft.com/en-us/xbox/accessibility/xbox-accessibility-guidelines/110): configurable haptics with redundant informational cues.
- [Xbox Accessibility Guidelines](https://learn.microsoft.com/en-us/xbox/accessibility/guidelines): input, clarity, time limits, UI context, and motion considerations.
- [Google Playables design practices](https://developers.google.com/youtube/gaming/playables/certification/best_practices_design): interactive onboarding and meaningful feedback. Used as general design guidance, not as DominoGo platform requirements.
