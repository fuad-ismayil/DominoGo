# Installed web-app layout correction

November 16, 2026. User screenshots showed the homepage header and account title
under the iPhone status bar, and an emerald bottom navigation bar in UnoGo.

The UnoGo shell now reserves the browser's top/left/right safe-area insets.
Fixed navigation reserves the bottom inset plus a 12px gap. Its navy background
explicitly overrides the shared DominoGo stylesheet's important emerald rule.
The account surface similarly overrides the legacy important background color.

The account panel stays inside all safe-area edges. Its heading and close button
are outside a dedicated scrolling settings body, so long password forms and
short landscape screens cannot scroll the close control away. Settings retain
their height rather than being compressed to fit the sheet.

Run `python unogo/tests/mobile_shell.py` after the release browser build.
It uses an isolated database and tests 393x852 with simulated 59px/34px insets,
320x568, 852x393 and desktop. Checks cover visible header, navigation placement,
computed navy colors, account bounds, long-form scrolling, fixed close button,
closing/navigation and page errors. Insets are simulated CSS variables; these
checks do not claim real iPhone or Safari verification. Signed-out entry is
checked too. Screenshots are in `docs/screenshots/mobile-shell`.

The live HTTPS browser build was checked for the new layout and page errors.
Actual iPhone home-screen behavior still needs checking on the user's device.
