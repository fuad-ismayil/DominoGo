# Go Host 2.2.2 build and device status

Installed November 17, 2026 after PC/phone sync verification. Version code 13,
same package/signing identity. APK size: 18,470,742 bytes.
SHA-256: `fc889eb6b2da15fb1cfa46497291cd18c48fb2da2d0b91c85699dad147b47108`.

Direct number selection, matching-anchor validation and stable fanned-card touch
targets passed browser mouse/keyboard and physical Android OS touch checks.
25 core, 4 client and 29 server tests passed, along with house-rule, gesture,
visual feedback and viewport integration checks. 19 native updater checks and
the real GitHub check passed. Both public domains match the phone runtime;
served UNO HTML/WASM match this final APK. Desktop production hosting stays off.
See [direct selection verification](../../unogo/docs/direct-selection.md).

## Historical 2.2.1 evidence

Installed November 17, 2026 after real PC/phone database sync. Package identity
and signing are retained; version code 12. APK size: 17,592,730 bytes.
SHA-256: `72976edca0c357fd9389bb816a3e966a66886938e4d3a1eede8166277378f0fd`.

Card caption removal, table/deck geometry, emote animation and UNO toggle polish
passed browser tests. Physical Android OS touches passed dragging, the 5 challenge
and simultaneous own/opponent emote visibility. All 19 updater checks passed.
The real GitHub check succeeded. Both public domains match the phone runtime;
served UNO HTML/WASM match this APK. Desktop production hosting remains stopped.
See [visual verification](../../unogo/docs/visual-feedback.md).

## Historical 2.2.0 evidence

Installed in place November 17, 2026. Development build; not published.

- Package `games.dominogo.host`, version code 11, ARM64; signing identity retained.
- APK `dist/GoHost.apk`, 20,983,496 bytes; identical legacy alias retained.
- SHA-256 `3bdac876f4cd6f9281652ed1a8011873a4e210040a3b41ccfe20353d4bfe6ff7`.
- Both games, release clients and Friends native rules included; static checks pass.
- Real PC/phone sync preceded the upgrade; accounts and saved games retained.
- Physical Android OS touch drag and 5 tap/order/one-penalty checks passed.
- Five JVM updater tests and 19 phone updater checks passed; real GitHub check succeeded.
- Both public domains identify the phone runtime; UNO HTML/WASM match the final APK.
- Both games run on the phone. Desktop production hosting remains stopped.

See [playtest verification](../../unogo/docs/friends-playtest.md) and checksum-bound
`dist/GoHost.device-check.json`. Real iPhone/Safari gestures and extended sleep,
background and tunnel-loss acceptance remain untested.

## Historical 2.1.2 evidence

Prepared and installed November 16, 2026. Development build; not published.

- APK: `dist/GoHost.apk`, 17,538,177 bytes; identical legacy alias retained.
- Package: `games.dominogo.host`, version code 10, ARM64.
- SHA-256: `76180cc54c638aad8a64bcf5879a3efea54d2693bc069d701a6fa1354c928bfc`.
- Signing certificate matches the previous DominoGo Host APK.
- Both games and release browser clients are included; no private database or
  tunnel credentials are embedded. Static asset/signature/16 KB ELF checks pass.

## Connected phone evidence

The Galaxy S9 was upgraded in place from 1.5 through the shared schema. Recovery
copies of PC, phone and baseline were saved privately. Real two-way database
sync completed before the new hosting session. Go Host 2.1.1 is installed and
both games run on the phone. Public DominoGo and UnoGo HTTPS health responses
match the phone runtime. Installed UNO HTML/WASM match the APK bytes.

A test-only Android instrumentation overlay used OS touch events on a physical
WebView and an isolated temporary server/database. A dragged card highlighted
the discard pile and was accepted once; seven cards became six, with no screen
overflow. Actual ADB captures are under `unogo/docs/screenshots/device-table`.
This did not alter player data or statistics. Checksum-bound evidence is in
`dist/GoHost.device-check.json`; future APKs do not inherit it automatically.

## Automated evidence

- 17 UNO client/core tests and Rust Clippy pass.
- Gesture checks cover native Chromium touch events, illegal/missed/cancelled
  drops, horizontal hand swiping, Wild color cancel/commit, second-tap play,
  keyboard fallback, reduced motion and server-failure recovery.
- Viewport tests cover 10 seats, 40 cards, turn/drawn/UNO/challenge/empty-pile
  states on small phone, iPhone inset simulation, browser phone, landscape and
  desktop. Account safe-area and scrolling tests also pass.
- Android updater JVM tests pass. Server and sync tests passed during shared
  host preparation; no game rules changed in this interaction update.

The complete two-browser regression round, friendship/invites, reactions, Wild cancellation, pause/restart, four-language account controls and password recovery passed. Native UnoGo Stop/Start also preserved the running DominoGo runtime.

## Remaining acceptance

Real iPhone/Safari gestures, longer simultaneous/background hosting, sleep and
tunnel-loss recovery remain. Physical Android acceptance is scoped above; it is
not a claim that every hardware release check is complete. Desktop hosting stays
stopped; both permanent game domains now route to the phone tunnel.

2.1.1 result reload/UTF-8 patch: both result paths passed three consecutive reloads, the public rules footer was checked, and the physical Android touch test passed again. Both games were restarted on the phone after successful real database sync.

2.1.2 fixes the false GitHub failure for locally built APKs. Five JVM tests and 19 on-device updater checks passed. The real phone GitHub check succeeded without the warning, and both public games are running. Browser/gameplay evidence above is unchanged from 2.1.1; this patch changes only the native updater decision and version metadata.
