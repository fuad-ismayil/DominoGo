//! DominoGo HTTP and realtime server. Configure DOMINO_DB_PATH, DOMINO_PORT, and DOMINO_STATIC_DIR.

#[tokio::main]
async fn main() {
    domino_server::run().await;
}
