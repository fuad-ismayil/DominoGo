# Result reload and UTF-8 correction — 2.1.1

Some client source strings had been decoded using the Windows legacy encoding
and written back as UTF-8, in a few cases twice. The live result heading, turn
separator, reconnect/sending messages and rules footer were affected. The
archived result used an uncorrupted heading, explaining why the visible symptom
varied between result paths. All affected strings were repaired using explicit
UTF-8 reads/writes. A source regression check catches the re-encoding markers.

Archived results now save only their match ID. After authenticated reload, the
client loads the user's permitted history and restores that result. It never
stores another player's hand or trusts a cached result from another account.
Leaving the review or signing out clears this destination. Explicit Home
navigation also clears the old table destination, so reloading Home stays Home.

Restoring an already joined table now uses GET rather than issuing another join.
Missing/forbidden tables clear the saved destination; temporary network errors
retain it for a retry. Invitations continue to use the join route.

Checks: three consecutive live-result reloads, three archived-result reloads,
exact UTF-8 heading/footer assertions, explicit Home reload, complete two-browser
round and account/social/recovery regression, client unit tests and Clippy.
The patched APK was installed in place; actual phone touch acceptance passed
again on an isolated WebView table. Both public routes match the phone runtime,
and the public rules footer was checked in a fresh browser. Private recovery
copies and real phone/PC sync were retained before restarting both games.
