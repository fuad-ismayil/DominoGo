# Direct card selection — November 17, 2026

Go Host 2.2.2 removes UnoGo's Group button. Click/tap a card to lift it. Another
card joins the selection only if the resulting same-number row is legal;
otherwise it replaces the selection. Tap an already selected member of a row
to remove it. Escape clears the selection. A single selected card still plays
on its second tap; use Play N or drag to play a selected row.

The shared engine and player-view validator both require at least one selected
card to match the current discard by color or number. That matching anchor may
be selected anywhere in the order. Unmatched pairs, mixed numbers, duplicate or
unowned cards reject atomically. The final selected card sets the active color.
Removing the only anchor collapses an otherwise invalid row to one card.
Drawn-card-only restrictions, Classic saves and one grouped 0/5 effect remain.

The hand keeps its original fan stacking order while cards are selected.
Pointer-down feedback does not move the card's hit area. This fixes lifted cards
covering an unselected middle neighbor and unstable physical tap targets.

## Verification

- 25 core tests, 4 client tests, wasm Clippy and 29 server tests passed.
- House-rule browser integration now uses real mouse clicks for direct selection:
  unmatched pair rejection, different-number replacement, late matching anchor,
  additional colors, anchor removal, clear, final color and multiplayer snapshots.
- Existing keyboard order/reorder/group drag, explicit UNO arming, four-language
  remount/layout, optional 0 and concurrent/restarted/bot 5 challenges passed.
- Gesture, visual feedback and 10-seat/40-card viewport regressions passed.
  The mocked viewport countdown is frozen so slow machines test the intended state.
- Physical Galaxy S9 OS taps selected Red 8 and Blue 8 directly, with no Group
  control. Play 2 removed both and set Blue. Physical drag, 5 and own/remote emotes
  also passed using isolated data. Captures: `screenshots/device-table`.
- The design detector retains its prior intentional color-underline warning.

2.2.2 was installed in place after sync verification. Nineteen phone updater
checks and the real GitHub check passed. Both public game domains identify the
phone; served UNO HTML/WASM match the APK. Desktop production hosting remains
stopped. Checksum-bound device evidence is recorded beside the APK.

Real iPhone/Safari and prolonged sleep/background/tunnel-loss testing remain.
