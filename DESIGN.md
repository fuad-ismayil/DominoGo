# DominoGo design contract

Clubs and competitive rankings are deferred until suitable matchmaking and player population exist. Use Home, Account, Registered player and Leaderboard in product copy. Do not label registered users as members of a nonexistent club or show MEMBER/GUEST membership badges. Existing club-prefixed CSS classes are implementation names and do not imply product features. The casual player leaderboard retains its existing ordering and offers calendar-period filters; it is not a verified skill ranking. The preserved future proposal is in docs/future-clubs-and-competitive-rankings.md.

This documents the existing product and the approved refinement, not a new visual direction.

DominoGo is a Rust browser domino game for mouse and touch, with private tables, bots, invitations, 101 scoring and four interface languages. The primary menu task is creating, joining or returning to a table. Treat these screens as working application UI rather than a marketing landing page.

Preserve the emerald table, ivory dimensional dominoes, gold primary action, authored domino logo, local Jakarta/Syncopate fonts and the user's clear Liquid Glass circles/capsules. Glass should identify interactive controls and the main information plane; it should not turn every row into another floating panel. Gameplay and its unobstructed round review are authoritative visual references.

Menu hierarchy: greeting and primary actions on an open surface; history on one principal plane; match rows separated by dividers; secondary navigation lower in emphasis but always reachable. No slogan eyebrows above headings, decorative numbering or braille glyphs masquerading as dominoes. Actual membership and round state labels remain useful information.

Use the shared club-system.css layer for menu-level tokens and patterns, keeping it scoped away from gameplay. Panel radius is 16px; pills/circles are for controls. Focus has a visible warm outline. Controls target at least 44px. Text wraps in Azerbaijani, Turkish and Russian. The viewport, not an assumed phone model, defines available space. Fixed navigation reserves matching bottom space.

Apply Impeccable's scoped polish workflow, preserve functioning controls and verify the rendered result. A detector pass is evidence about mechanical patterns, not a certification of quality. No skill overrides the user's Rust-only application-source requirement or the requested game identity.
