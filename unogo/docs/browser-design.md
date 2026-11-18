# Browser direction and verification

Mode: Operate. This surface extends DominoGo's existing product and composition.
The GDD explicitly selects Navy; the root `DESIGN.md` continues to govern
DominoGo and is preserved. No concept alternatives or generated artwork are used.

## Direction contract

THESIS: friends recognize the same entry, home, setup and waiting-room flow;
UNO is a distinct, readable card board inside that family.

OWN-WORLD: #183f70 to #07172f field, #98d0ff primary action, local
Jakarta/Syncopate fonts, glass capsules/circles, one history plane and divided rows.
Original typographic cards use color names, numbers and drawn action symbols.

STORY: enter as guest or with the shared account, create or join a private table,
invite friends, start deliberately, play a single round, and return to results.

FIRST VIEWPORT: desktop Home retains open greeting/actions on the left and
history on the right; phones stack them above the familiar bottom navigation.
Gameplay puts compact opponent identities above draw/discard piles and a large,
horizontally scrolling hand with reachable actions below. No game navigation
overlays the hand.

FORM: inherited menu composition, purpose-built card table. The chosen GDD
structure is already fixed; seed: none (precisely specified extension).

FINISH: unreviewed and undocumented is unfinished; this build ends with the
finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance.
This extension uses the existing root design contract with the GDD's explicit
Navy substitution; no raster artwork ships. Fonts and their licenses are copied
from the existing product; icons and card faces are code-native geometry.

## Validation scope

`tests/browser_flow.py` runs the compiled browser app against an isolated actual
host in Microsoft Edge. Two browser storage contexts exercise guest invites,
waiting-room rejoin, keyboard/touch card selection, complete rounds, reactions,
pause voting, offline control lock, host restart and browser reload, and results.
Registered contexts exercise registration, login, shared friendship requests,
acceptance and in-product UNO table invitations.

A conserved 108-card fixture assigns 40 cards to the local player and seven to
each of nine opponents to verify the valid large-hand layout. The fixture is
inserted only into the test database; no privileged test endpoint ships. It also
checks that cancelling a Wild chooser preserves both the hand and the deadline.

Required captures in `docs/screenshots`: entry/home/setup/lobby/game/results on
desktop; lobby/game/results on 390×844 phones; ten seats with forty cards at
1440×960, 390×844 and 320×740; color selection on a phone; and the account sheet
in Azerbaijani, Turkish, Russian and English. Each capture disables entrance
motion and is taken from the document top. Horizontal document overflow is a
test failure; hand scrolling is intentional.

## Release boundaries

This is a locally built browser milestone. Android packaging, independent game
Start/Stop, offline database sync for UNO, signed updates and public tunnel
routing remain separate delivery stages. API snapshots currently poll rather
than subscribing over Socket.IO. Active-table hiding, paginated history and
full completed-hand review remain follow-up product work.
