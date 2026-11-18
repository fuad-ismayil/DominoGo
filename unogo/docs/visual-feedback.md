# Visual feedback follow-up — November 17, 2026

## Changes

- Removed visible color names from card faces. Hand buttons retain spoken
  color/value labels; the active color remains explicit above the table.
- The table edge no longer extends into the status text. Discard geometry is
  centered independently of the smaller upper-left draw deck, including compact
  phone and landscape layouts. The deck retains a minimum 44px touch size.
- Every human player, including the local sender, gets a reaction. An unclipped
  table layer anchors the local reaction above the emote row and opponents'
  reactions at their seats. Motion matches DominoGo's 2.4-second pop/settle/rise.
  Keyed events and their server age prevent polling from replaying animations.
  Repeated emoji sends animate as distinct events. Reduced motion stays static.
- UNO is a stable blue/gold toggle with a checked armed state and localized help.
  Removing its secondary-button hover style fixes the transparent/dark label.

## UNO behavior

With two cards, or a selected number group that will leave one, turn UNO on before
playing. Gold/checkmark means the next play declares UNO; blue means it does not.
Tapping again cancels the declaration. The toggle never plays the card itself.
An accepted game action resets it. If the player goes down to one without calling,
a two-second window lets them Call UNO or an opponent Catch UNO. A successful
catch draws two penalty cards; there is no automatic penalty just for timeout.

## Evidence and delivery

`visual_feedback.py` passed label removal, status spacing, centered discard and
separated smaller deck at 393x852, 320x568, 852x393 and 1440x960; every UNO toggle/
hover/checkmark state; two-player own/remote reactions, expiry, repeated events,
polling stability and reduced motion. Client tests and wasm Clippy passed.
The 10-seat/40-card special-phase viewport suite and gesture regression passed.

Rendered screenshots were inspected on browser phones and physical Android.
The design detector retains only the prior intentional card-color order-chip
underline warning. Browser captures: `screenshots/visual-feedback`; physical
captures: `screenshots/device-table/own-and-opponent-emotes.png`.

Go Host 2.2.1 (code 12) was installed in place after real database sync. Physical
Galaxy S9 OS touches passed card drag, 5 tapping and own/opponent emotes using an
isolated test database. Nineteen native updater checks and the real GitHub check
passed. Public DominoGo/UnoGo health identifies the phone; served UNO HTML/WASM
matches the final APK. Desktop production hosting remains stopped.

Gameplay rules and old-save compatibility are unchanged. Real iPhone/Safari
gesture testing and extended background/sleep/tunnel-loss acceptance remain.
