//! DominoGo HTTP and realtime server. Configure DOMINO_DB_PATH, DOMINO_PORT, and DOMINO_STATIC_DIR.

mod auth;
mod db;
mod domain;
mod lifecycle;
mod ops;
mod rest;
mod socket;
#[path = "../../../../unogo/server/mod.rs"]
mod unogo;

use domain::{AppState, Registry};
use socketioxide::handler::ConnectHandler;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use tower_http::cors::CorsLayer;

#[cfg(target_os = "android")]
mod android;

pub async fn run() {
    let db_path = std::env::var("DOMINO_DB_PATH").unwrap_or_else(|_| "database.sqlite".to_string());
    let port: u16 = std::env::var("DOMINO_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);

    let db = db::Db::open(&db_path).expect("failed to open/create SQLite database");
    unogo::recover(&db).unwrap_or_else(|e| panic!("UnoGo recovery failed: {}", e.2));

    // TS: cleanupStaleGuests() at startup — log the same line.
    let cleaned = db.cleanup_stale_guests().unwrap_or(0);
    println!("Cleaned up {cleaned} rows of stale guest data");

    let (socket_layer, io) = socketioxide::SocketIo::new_layer();
    let app_state = Arc::new(AppState {
        host_id: uuid::Uuid::new_v4().to_string(),
        db,
        games: lifecycle::Games::initial(),
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
    let router = lifecycle::gated(
        router.layer(socket_layer).layer(CorsLayer::permissive()),
        app_state.clone(),
        lifecycle::GameId::Domino,
    );

    let uno_port: u16 = std::env::var("UNOGO_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3001);
    let mut uno_router = unogo::router(app_state.clone());
    if let Ok(dir) = std::env::var("UNOGO_STATIC_DIR") {
        uno_router = uno_router.fallback_service(
            tower_http::services::ServeDir::new(&dir).not_found_service(
                tower_http::services::ServeFile::new(format!("{dir}/index.html")),
            ),
        );
    }
    let uno_router = lifecycle::gated(
        uno_router.layer(CorsLayer::permissive()),
        app_state.clone(),
        lifecycle::GameId::Uno,
    );
    let uno_listener = tokio::net::TcpListener::bind(("0.0.0.0", uno_port))
        .await
        .expect("failed to bind UnoGo port");
    println!("UnoGo API running on http://0.0.0.0:{uno_port}");
    unogo::host_enabled(&app_state.db, app_state.games.uno.enabled())
        .unwrap_or_else(|e| panic!("UNO lifecycle recovery: {}", e.2));
    let uno_clock = unogo::spawn_clock(app_state.clone());

    if !app_state.games.domino.enabled() {
        ops::suspend_host_matches(&app_state).expect("Domino lifecycle recovery");
    }
    ops::recover_matches(&app_state);
    ops::spawn_guest_cleanup(app_state.clone());
    ops::spawn_waiting_room_cleanup(app_state.clone());

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("failed to bind port");
    println!("Server running on http://0.0.0.0:{port}");
    let server = axum::serve(listener, router);
    let uno_server = axum::serve(uno_listener, uno_router);
    #[cfg(target_os = "android")]
    android::install_control(app_state.clone());
    if let Ok(port) = std::env::var("GO_HOST_CONTROL_PORT") {
        let port: u16 = port.parse().expect("invalid loopback control port");
        let token =
            std::env::var("GO_HOST_CONTROL_TOKEN").expect("control requires a private token");
        assert!(
            token.len() >= 32,
            "control token must contain at least 32 characters"
        );
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .expect("control port is unavailable");
        let router = lifecycle::control_router(app_state.clone(), token);
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                eprintln!("Local host control: {e}");
            }
        });
    }
    #[cfg(target_os = "android")]
    {
        let (stop, mut domino_stop) = tokio::sync::watch::channel(false);
        let mut uno_stop = domino_stop.clone();
        tokio::spawn(async move {
            android::shutdown().await;
            let _ = stop.send(true);
        });
        tokio::try_join!(
            async {
                server
                    .with_graceful_shutdown(async move {
                        let _ = domino_stop.changed().await;
                    })
                    .await
            },
            async {
                uno_server
                    .with_graceful_shutdown(async move {
                        let _ = uno_stop.changed().await;
                    })
                    .await
            }
        )
        .expect("server error");
    }
    #[cfg(not(target_os = "android"))]
    tokio::try_join!(async { server.await }, async { uno_server.await }).expect("server error");
    uno_clock.abort();
}
