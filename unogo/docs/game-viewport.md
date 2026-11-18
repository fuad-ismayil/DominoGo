# Phone-sized gameplay table

The live table uses a 100dvh flex shell, reserving the real safe-area insets and
header before assigning the remaining height to gameplay. The center card piles
use the remaining space instead of imposing a minimum page height. Player rows
and long hands scroll horizontally. Empty decision rows take no space.

Short phones use smaller cards and place turn information beside the piles.
Landscape uses two columns, with the hand beside the piles. Active decision
buttons remain visible; extra instructional paragraphs are available to screen
readers rather than taking another row on the shortest portrait layout. The
empty-pile notice is grouped with the turn status. Completed results can still
scroll when necessary; the live table does not hide overflow to conceal controls.

Run `python unogo/tests/game_viewport.py` after a release browser build. It uses
an isolated database and conserved 40-card hand with 10 seats, plus browser-only
snapshot fixtures for turn, drawn-card, UNO, challenge and empty-pile states.
It verifies document height/width, action visibility, horizontal hand scrolling
without page movement, console errors and rendered captures at:

- 393x852, with simulated iPhone 59px/34px safe insets.
- 390x664 browser viewport.
- 320x568 small phone, with a 20px top inset.
- 852x393 landscape, with a 21px bottom inset.
- 1440x960 desktop.

All checks passed November 16, 2026. The live HTTPS bundle and refreshed combined
APK contain this layout. Real phone/Safari testing has not been performed here.
