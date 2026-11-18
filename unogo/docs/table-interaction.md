# Direct card play — Go Host 2.1.0

Surface mode: Experience. Preserve the established navy identity and rules;
make direct card movement the focal interaction while keeping recovery and
keyboard access explicit.

The navy identity stays. Gameplay now centers on a table surface, an overlapping
fanned hand, small opponent hands and physical deck/discard piles. Routine play
uses cards rather than a row of Draw/Play buttons. Reactions and duplicate card
controls live in compact, keyboard-accessible native disclosures. Keep and UNO
choices remain visible when relevant; UNO is never declared automatically.

## Interaction and motion

- Tap a card to lift it; tap again to play. Keyboard activation follows the same
  path, with named Draw/Play controls available as a fallback.
- Hold for 180ms or move upward to pick up a card. It follows the pointer as a
  floating copy; a valid discard target highlights. Missed/illegal drops return
  to the hand. A horizontal movement before pickup pans a long hand.
- Wild drops open color selection. The card stays in the authoritative hand
  until the color is submitted; cancelling sends no play.
- Pointer capture retains the gesture outside its originating card. Cancellation,
  lost capture, Escape, changed turns/matches and leaving the screen clear the
  gesture. Each accepted drop uses the existing sequenced server command.
- Accepted plays, draws and initial deals carry spatial motion. Snapshot polling
  does not replay unchanged animations. Opponent card backs never reveal hands.
- Pointer frames update a floating copy directly instead of re-rendering all
  cards. Flights use transforms; motion is clipped in a dedicated inert layer.
  Reduced motion skips nonessential flights/dealing and keeps direct feedback.

The implementation is Rust/Dioxus and browser APIs, with no new JavaScript UI
framework or drag library dependency. References considered:
[W3C Pointer Events](https://www.w3.org/TR/pointerevents/),
[Dioxus pointer events](https://docs.rs/dioxus/0.7.10/dioxus/events/type.PointerEvent.html),
[dioxus-dnd](https://github.com/kindintelligence/dioxus-dnd), and
[WebKit pointer/Web Animations support](https://webkit.org/blog/10247/new-webkit-features-in-safari-13-1/).
The native pointer implementation fits the existing Rust client and a specialized
card/pile gesture without importing a sortable-list interaction model.

## Evidence, November 16, 2026

`card_gestures.py` exercises real Chromium touch events against an isolated
server: legal drop once, missed/illegal drop, cancellation, hand swipe, Wild
cancel/commit, second-tap play, keyboard controls, reduced motion and failed
request recovery. `game_viewport.py` covers 10 seats/40 cards and all decision
states at 320/390/393px, desktop and landscape. The shared account, round, invite,
reaction, password and recovery browser regression suite uses the new controls.

`device_table.py` and the test-only Android instrumentation ran on the connected
Galaxy S9. OS touch events picked up a card, highlighted the pile and committed
one card on release; the isolated server hand went from seven to six, with no
viewport overflow. Actual device screenshots were captured through ADB under
`docs/screenshots/device-table`. This is physical Android WebView evidence,
not a claim of Safari testing or a measured performance rating.

The APK was upgraded in place, with real PC/phone sync and private recovery
copies. Both permanent public links identify the phone runtime. The desktop
server and its UNO tunnel are stopped. Installed HTML/WASM match the APK exactly.
Checksum-bound device evidence is in `android-host/dist/GoHost.device-check.json`.
Real iPhone/Safari and extended background/sleep/tunnel-loss acceptance remain.
