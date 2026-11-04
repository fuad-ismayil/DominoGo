//! DominoGo HTTP and realtime server. Configure DOMINO_DB_PATH, DOMINO_PORT, and DOMINO_STATIC_DIR.

mod auth;
mod db;
mod domain;
mod ops;
mod rest;
mod socket;

use domain::{AppState, Registry};
use socketioxide::handler::ConnectHandler;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    let db_path = std::env::var("DOMINO_DB_PATH").unwrap_or_else(|_| "database.sqlite".to_string());
    let port: u16 = std::env::var("DOMINO_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);

    let db = db::Db::open(&db_path).expect("failed to open/create SQLite database");

    // TS: cleanupStaleGuests() at startup — log the same line.
    let cleaned = db.cleanup_stale_guests().unwrap_or(0);
    println!("Cleaned up {cleaned} rows of stale guest data");

    let (socket_layer, io) = socketioxide::SocketIo::new_layer();
    let app_state = Arc::new(AppState {
        db,
        turn_timers: Mutex::new(HashMap::new()),
        deadlines: RwLock::new(HashMap::new()),
        stop_votes: RwLock::new(HashMap::new()),
        registry: RwLock::new(Registry::default()),
        ai_spawns: Mutex::new(HashSet::new()),
    });
    domain::APP
        .set(app_state.clone())
        .unwrap_or_else(|_| panic!("AppState initialized once"));
    io.ns("/", socket::on_connect.with(socket::auth_middleware));

    let mut router = rest::router(app_state.clone());
    if let Ok(dir) = std::env::var("DOMINO_STATIC_DIR") {
        router =
            router.fallback_service(tower_http::services::ServeDir::new(&dir).not_found_service(
                tower_http::services::ServeFile::new(format!("{dir}/index.html")),
            ));
    }
    // socketioxide layer intercepts `/socket.io/*`; CORS sits outermost so
    // REST, polling and websocket handshakes are all covered (TS: cors '*').
    let router = router.layer(socket_layer).layer(CorsLayer::permissive());

    ops::recover_matches(&app_state);
    ops::spawn_guest_cleanup(app_state.clone());
    ops::spawn_waiting_room_cleanup(app_state.clone());

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("failed to bind port");
    println!("Server running on http://0.0.0.0:{port}");
    axum::serve(listener, router).await.expect("server error");
}
