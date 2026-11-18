# UnoGo identity

The homepage and saved home-screen icon share an original three-card fan with
an open U, icy blue edges and restrained red, yellow, green and blue accents.
The wordmark preserves the existing local Syncopate type and Navy identity.

Editable vector masters live in `crates/uno-client/assets/brand`. Use
`python unogo/tools/render_brand.py` to regenerate the PNG and ICO outputs with
Playwright/Edge and Pillow. The 1024px icon is available for future exports;
192px and 512px icons are declared in the web manifest. A separate 512px maskable
asset keeps the mark inside Android's central safe area. Apple receives an
opaque 180px touch icon. Tiny favicons use a simplified two-card mark.

`unogo/build.ps1` copies the identity, manifest and favicon into the served build.
The Android asset packager includes them in the combined hosting APK.
The manifest opens a saved shortcut in standalone mode; offline play is not
provided. Platform prompts and shortcut behavior vary by browser.

Verified November 16, 2026: rendered desktop, 390px and 320px layouts; public
HTTPS logo loading; browser-parsed manifest; declared PNG dimensions; Apple touch
icon and favicon responses; no page errors or horizontal overflow. The design
scan reported no findings. Actual phone home-screen installation is untested.
Existing saved shortcuts may retain their old icon until removed and re-added.
