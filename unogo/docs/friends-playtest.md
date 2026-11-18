# Friends playtest update — November 17, 2026

Implemented all nine supplied playtest requests: refined cards, translated table
layout, active account language on reopening, smaller offset deck, ordered
same-number groups, optional 0 hand exchange, everyone-tap 5, visible emotes and
removal of the floating Draw/Play menu.

New rounds use `friends-108-number-batch-zero-five-v2`. Saved Classic rounds keep
their original rules. The last card in a chosen number group sets the color;
grouped 0s/5s trigger their special once after the entire group. Explicitly arm
UNO when a selection leaves one card. Invalid groups reject atomically.

0 offers one opponent or Keep, defaulting to Keep after 15 seconds. 5 has a
shared 1.2-second ready countdown and 6-second response window. The server orders
arrivals, rejects duplicate/stale taps and assigns one shuffled-deck card to the
last responder. At timeout, one random nonresponder receives the single penalty.
Network latency affects arrival order. Bots participate independently of turn;
pause and restart preserve the challenge. A final 5 wins after penalty resolution.

## Validation

- 24 core, 4 client/localization/UTF-8 and 29 server tests passed; client/server
  Clippy passed with warnings denied.
- House-rule integration passed: chosen order, reordering, group drag, UNO arming,
  both final colors, invalid groups, optional swap/private hands, simultaneous
  taps, duplicate handling, rank feedback, one penalty, timeout, restart and bots.
- Full two-browser round passed: invites/friends, keyboard/touch, Wild cancel,
  UNO, reactions, pause/offline/restart, results and repeated result reloads.
- Four-language account remount/reload and responsive table checks passed.
  10 seats/40 cards and special phases fit small phone, simulated iPhone insets,
  browser phone, landscape and desktop.
- Gesture checks passed: touch drag, missed/illegal/cancelled drops, horizontal
  swipe, Wild cancel/commit, second-tap play, reduced motion and failed requests.
- Independent host lifecycle, 13 sync tests, 5 Android updater JVM tests and
  19 on-device updater checks passed. APK signing/assets/JNI/16 KB alignment passed.
- Physical Galaxy S9 WebView OS touches committed a drag once and recorded the
  first 5 tap; the second player received exactly one penalty. No screen overflow.
  This used an isolated temporary database, without changing player statistics.
- Visual inspection covered groups, swap, tapping, translated small layouts and
  actual device captures. The design detector's single rounded-border warning
  is intentional: order-chip underlines communicate card color, alongside text.

## Delivery

Go Host 2.2.0 (code 11) was installed in place after real PC/phone database sync.
Both public game domains identify the phone runtime; served UNO HTML/WASM match
the final APK. The real GitHub check succeeded without a revision warning.
Both games are running, and desktop production hosting remains stopped.

Real iPhone/Safari gestures and extended sleep/background/tunnel-loss acceptance
remain untested. Screenshots are under `screenshots/house-rules` and
`screenshots/device-table`. APK details are in `android-host/dist/GoHost.build.json`.
