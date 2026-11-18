# UnoGo product context

## Platform and hosting
Rust/Dioxus browser client and shared Rust host for phones and desktop. Go Host
is the single Android APK for DominoGo and UnoGo, with independent server controls.
The connected phone is the current public host. Keep the desktop production
server stopped unless the user asks to move hosting back.

## Confirmed product truth
The original brief is `unogo-gdd.md`; its November 17 Friends amendment records
the user's playtest changes. Private 2–10 seat rounds mix humans and bots.
Accounts and friendships are shared with DominoGo; history and statistics remain
separate. No points currency, certification or cumulative match mode.
New rounds use Friends rules: ordered same-number groups, optional hand exchange
on 0, and an everyone-tap challenge on 5. Resolve a grouped 0 or 5 once after the
whole group. Existing saved Classic rounds retain their original rules.

## Visual and interaction authority
Preserve the navy identity (#183f70 to #07172f), icy blue #98d0ff, local
Jakarta/Syncopate typography and original UnoGo brand. Crisp ivory card faces
sit on a navy physical table. Tap to lift, tap again to play; hold and slide onto
the discard pile. Direct card selection builds legal same-number rows, supports reordering, and
allows the whole row to be dragged. At least one selected card must match the
discard; incompatible cards replace the selection. No separate Group button. Draw/keep lives on the smaller deck.
Emotes stay visible in a compact horizontal row. Avoid duplicate action menus.

## Constraints
Rust application behavior, private server snapshots, transactional commands,
explicit human UNO declarations, staged Wild colors and challengeable +4 cards.
Support phones, keyboard, large hands, reduced motion and English/Azerbaijani/
Turkish/Russian. Preserve player data and signing identity during APK upgrades.
