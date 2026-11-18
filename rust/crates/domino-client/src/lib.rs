//! DominoGo browser application: Dioxus UI and a Rust realtime transport.

#[cfg(target_arch = "wasm32")]
pub mod app;
#[cfg(target_arch = "wasm32")]
pub mod game;

#[cfg(target_arch = "wasm32")]
pub mod browser;
#[cfg(target_arch = "wasm32")]
pub mod net;

#[path = "game/board.rs"]
pub mod board;
#[path = "game/feedback.rs"]
pub mod feedback;
pub mod i18n;
pub mod invite;
#[path = "game/placement.rs"]
pub mod placement;
pub mod reactions;
#[cfg(target_arch = "wasm32")]
pub mod store;
pub mod swipe;
pub mod types;

#[cfg(target_arch = "wasm32")]
pub use app::App;

/// Browser entry point (called from `main.rs` on wasm32).
#[cfg(target_arch = "wasm32")]
pub fn web_main() {
    // Prime the sound cache up-front so the first `play()` has no latency.
    // Creating <audio> elements does not require a user gesture.
    game::audio::init_audio();
    dioxus::launch(App);
}

/// Native stub â€” the Dioxus web client has no desktop target yet.
#[cfg(not(target_arch = "wasm32"))]
pub fn stub_native() {
    eprintln!(
        "domino-client is a Dioxus *web* client; compile it for wasm32:\n\
         cargo check -p domino-client --target wasm32-unknown-unknown"
    );
}

#[path = "game/layout.rs"]
pub mod layout;
#[cfg(target_arch = "wasm32")]
pub mod screens;
#[path = "game/telefon_layout.rs"]
pub mod telefon_layout;
