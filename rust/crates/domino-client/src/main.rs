//! Binary entry point.

#[cfg(target_arch = "wasm32")]
fn main() {
    domino_client::web_main();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    domino_client::stub_native();
}
