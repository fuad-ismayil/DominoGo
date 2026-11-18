#[cfg(target_arch = "wasm32")]
pub mod app;
#[cfg(target_arch = "wasm32")]
pub mod browser;
#[cfg(target_arch = "wasm32")]
pub mod i18n;
pub mod model;
#[cfg(target_arch = "wasm32")]
pub mod motion;
#[cfg(target_arch = "wasm32")]
pub mod net;
#[cfg(target_arch = "wasm32")]
pub mod screens;
#[path = "../../../../rust/crates/domino-client/src/i18n.rs"]
pub mod shared_i18n;
#[cfg(target_arch = "wasm32")]
pub mod store;
#[cfg(target_arch = "wasm32")]
pub mod ui;

#[cfg(test)]
mod text_regressions {
    #[test]
    fn interface_sources_have_no_reencoded_utf8() {
        let sources = [
            include_str!("ui.rs"),
            include_str!("store.rs"),
            include_str!("app.rs"),
            include_str!("screens/game.rs"),
            include_str!("screens/home.rs"),
        ];
        for source in sources {
            for marker in ["\u{00c3}", "\u{00c2}", "\u{00e2}\u{20ac}"] {
                assert!(
                    !source.contains(marker),
                    "Re-encoded UTF-8 in interface source"
                );
            }
        }
        assert!(include_str!("ui.rs").contains("How to play · UnoGo"));
        assert!(include_str!("screens/game.rs").contains("Reconnecting…"));
    }
}
