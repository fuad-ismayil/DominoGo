#[cfg(target_arch = "wasm32")]
fn main() {
    dioxus::launch(uno_client::app::App);
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("Build UnoGo for the web with unogo/build.ps1");
}
