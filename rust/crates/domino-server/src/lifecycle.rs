//! Per-game lifecycle. Network listeners stay bound while disabled; mutations and
//! watchdogs stop, private state is saved, and the other game remains available.
use crate::{domain::AppState, ops, unogo};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    middleware::{from_fn, Next},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use tokio::sync::RwLock;

#[derive(Clone, Copy)]
pub enum GameId {
    Domino,
    Uno,
}
impl GameId {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "dominogo" => Some(Self::Domino),
            "unogo" => Some(Self::Uno),
            _ => None,
        }
    }
}
pub struct GameSwitch {
    enabled: AtomicBool,
    pub generation: AtomicU64,
    pub gate: RwLock<()>,
}
impl Default for GameSwitch {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            generation: AtomicU64::new(0),
            gate: RwLock::new(()),
        }
    }
}
impl GameSwitch {
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
}
#[derive(Default)]
pub struct Games {
    pub domino: GameSwitch,
    pub uno: GameSwitch,
}
impl Games {
    pub fn initial() -> Self {
        let games = Self::default();
        if std::env::var("GO_HOST_INITIAL_GAMES").as_deref() == Ok("none") {
            games.domino.enabled.store(false, Ordering::SeqCst);
            games.uno.enabled.store(false, Ordering::SeqCst);
        }
        games
    }
    pub fn game(&self, id: GameId) -> &GameSwitch {
        match id {
            GameId::Domino => &self.domino,
            GameId::Uno => &self.uno,
        }
    }
    pub fn status(&self) -> Value {
        json!({"dominogo":self.domino.enabled(),"unogo":self.uno.enabled()})
    }
}
pub fn gated(router: Router, app: Arc<AppState>, game: GameId) -> Router {
    router.layer(from_fn(move|request:axum::extract::Request,next:Next|{let app=app.clone();async move{
        let _gate=app.games.game(game).gate.read().await;
        if !app.games.game(game).enabled(){return (StatusCode::SERVICE_UNAVAILABLE,[("Cache-Control","no-store")],Json(json!({"code":"GAME_OFFLINE","error":"This game is stopped on the host."}))).into_response();}
        next.run(request).await
    }}))
}
pub async fn set_enabled(app: &Arc<AppState>, game: GameId, enabled: bool) -> Result<(), String> {
    let switch = app.games.game(game);
    let _gate = switch.gate.write().await;
    if switch.enabled() == enabled {
        return Ok(());
    }
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    switch.enabled.store(false, Ordering::SeqCst);
    switch.generation.fetch_add(1, Ordering::SeqCst);
    match game {
        GameId::Uno => unogo::host_enabled(&app.db, enabled).map_err(|e| e.2)?,
        GameId::Domino => {
            if !enabled {
                ops::suspend_host_matches(app)?;
                let sockets: Vec<_> = app
                    .registry
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .by_socket
                    .values()
                    .map(|s| s.handle.clone())
                    .collect();
                for socket in sockets {
                    let _ = socket.disconnect();
                }
            }
        }
    }
    switch.enabled.store(enabled, Ordering::SeqCst);
    if enabled && matches!(game, GameId::Domino) {
        ops::recover_matches(app);
    }
    Ok(())
}

// Optional desktop-only control, bound to loopback and authenticated. Never
// mounted on either game listener or exposed through their public tunnels.
pub fn control_router(app: Arc<AppState>, token: String) -> Router {
    Router::new()
        .route(
            "/status",
            get(|State(app): State<Arc<AppState>>| async move { Json(app.games.status()) }),
        )
        .route(
            "/games/{game}",
            post(
                |State(app): State<Arc<AppState>>,
                 Path(game): Path<String>,
                 Json(body): Json<Value>| async move {
                    let (Some(game), Some(enabled)) =
                        (GameId::parse(&game), body["enabled"].as_bool())
                    else {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"error":"Choose a game and enabled state"})),
                        )
                            .into_response();
                    };
                    match set_enabled(&app, game, enabled).await {
                        Ok(()) => Json(app.games.status()).into_response(),
                        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":e})))
                            .into_response(),
                    }
                },
            ),
        )
        .with_state(app)
        .layer(from_fn(
            move |headers: HeaderMap, request: axum::extract::Request, next: Next| {
                let token = token.clone();
                async move {
                    if headers.get("authorization").and_then(|h| h.to_str().ok())
                        != Some(format!("Bearer {token}").as_str())
                    {
                        return StatusCode::UNAUTHORIZED.into_response();
                    }
                    next.run(request).await
                }
            },
        ))
}
