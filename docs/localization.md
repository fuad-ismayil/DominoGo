# Interface languages

DominoGo offers English, Azerbaijani, Turkish and Russian. Select a language on the welcome/sign-in card, in Your account, or in the table settings. Changes apply immediately, retain the player's game/session, update the document language, and are saved under `domino.language`. A first visit uses the browser language when supported, otherwise English.

The Rust `i18n` module loads the four JSON catalogues once. English copy is the lookup key; named placeholders are substituted without interpreting inserted player names. Dates use the chosen locale. Player usernames, table codes, protocol identifiers and game-state values are never translated. Known error messages, accessibility labels, rules, scoring explanations, invitations and account controls use the same catalogues.

Each language received a separate wording review. Azerbaijani uses daş, bazar, raund and əsas/müvəqqəti xal. Turkish uses taş, el, maç and taş çekme; it avoids a literal translation of bazar. Russian uses кость, базар, раунд and distinguishes основной счёт from points в запасе. Count and winner templates avoid unnecessary gender/plural assumptions. The scoring prose explains the strict greater-than-12 threshold, consecutive wins, pending-score loss, and the lone blank exception.

When changing copy, update all four files in `rust/crates/domino-client/locales`. Preserve placeholders exactly. Native tests require identical keys and matching placeholder counts for every language. Formatting does not recursively substitute placeholders inside player-provided text. Unrecognized future copy falls back to English rather than rendering blank text.

Run `cargo test --manifest-path rust/Cargo.toml -p domino-client --offline`, then the WebAssembly Clippy/build checks. Test language changes, reload persistence, narrow layouts and long translations in the browser. Existing verification limits for physical devices still apply.
