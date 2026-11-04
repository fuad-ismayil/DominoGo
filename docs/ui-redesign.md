# UI restoration and refinement

## Baseline

The preserved original application was run locally against an isolated Rust server on port 3011 for direct comparison. Its gameplay strengths are a full-window emerald table, compact opponent capsules, an anchored ivory hand, restrained gold highlights, and endpoint drop zones beside the chain. These are the implemented design foundation. The earlier Rust client's large cards, disconnected end buttons and horizontally scrolling strip were a regression.

Menus have been reorganized for readable phone-sized navigation rather than preserving the original compressed two-column mobile grid. Account controls, guest expiry/save, social notifications, invitations, lobby controls, detailed match history, scores, and session actions must remain functional.

## Material direction

Emerald felt remains the content surface. Clear glass belongs to controls and navigation above that surface, following Apple's material guidance: https://developer.apple.com/design/human-interface-guidelines/materials . Warm rim lighting, translucent green tint, curved reflections, inner highlights and depth create the optical character. Heavy opaque blur is avoided. CSS is a browser interpretation of the material, not Apple's native rendering implementation.

Typography restores the original Plus Jakarta Sans and Syncopate identity. Fonts are served locally with their licenses, avoiding third-party requests during play. Reduced motion and transparency preferences receive explicit fallbacks.

## Interaction requirements

Pointer-based dragging supports mouse and touch through the same path: activation threshold, pointer capture, a lifted tile ghost, legal endpoint feedback, release/cancel cleanup, and server-authoritative placement. Drag movement updates only the lifted DOM ghost and target highlights rather than rerendering the table on every pointer event. Tap/keyboard selection remains available alongside dragging.

The table uses the available viewport without unnecessary menu chrome. All 28 tiles remain representable in the board layout. Long hands, long usernames, four seats, small portrait phones and short landscape screens are explicit layout cases.

Audio uses documented recorded tabletop foley, cached in memory, with gesture unlocking and persistent mute/volume. Playback failures must not interrupt a game.
