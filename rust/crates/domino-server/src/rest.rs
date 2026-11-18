//! HTTP API for accounts, lobbies, match history, friends, and rankings.

// Axum responses carry the HTTP body directly; boxing every error adds allocation.
#![allow(clippy::result_large_err)]

use crate::auth::{hash_password, internal_error, is_unique_violation, new_user_id, AuthUser};
use crate::domain::{app, AppState};
use crate::ops;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use domino_core::DrawRule;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    game_router(state.clone()).merge(shared_router(state))
}

/// Account authority and the friend graph are shared by both game origins.
pub(crate) fn shared_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/guest", post(guest))
        .route("/api/auth/me", get(me))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/logout-others", post(logout_others))
        .route("/api/auth/upgrade", post(upgrade))
        .route("/api/auth/reset-password", post(reset_password))
        .route("/api/friends", get(friends_list))
        .route("/api/friends/request", post(friend_request))
        .route("/api/friends/accept", post(friend_accept))
        .route("/api/friends/reject", post(friend_reject))
        .route("/api/friends/cancel", post(friend_cancel))
        .route("/api/friends/{target_user_id}", delete(friend_remove))
        .with_state(state)
}

fn game_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/api/health",
            get(|State(app): State<Arc<AppState>>| async move {
                (
                    [(axum::http::header::CACHE_CONTROL, "no-store")],
                    Json(json!({"gameId":"dominogo","hostId":app.host_id})),
                )
            }),
        )
        .route("/api/matches/history", get(match_history))
        .route("/api/matches/{id}", delete(delete_match))
        .route("/api/lobbies", post(create_lobby))
        .route("/api/lobbies/{id}/join", post(join_lobby))
        .route("/api/lobbies/{id}/leave", post(leave_lobby))
        .route("/api/lobbies/{id}/add-bot", post(add_bot))
        .route("/api/lobbies/{id}/kick", post(kick))
        .route("/api/lobbies/{id}/start", post(start_game))
        .route("/api/stats/{user_id}", get(get_stats))
        .route("/api/leaderboard", get(leaderboard))
        .with_state(state)
}

/* --------------------------------- helpers -------------------------------- */

fn err(code: StatusCode, message: &str) -> Response {
    (code, Json(json!({ "error": message }))).into_response()
}

type JsonBody = axum::extract::Json<Value>;

fn body_str(body: &Value, key: &str) -> Option<String> {
    body.get(key).and_then(Value::as_str).map(str::to_string)
}

/// `Math.random().toString(36).substring(2, 9)` equivalent: `n` uniform
/// lowercase base-36 characters derived from a random UUID.
pub(crate) fn random_base36(n: usize) -> String {
    const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let id = uuid::Uuid::new_v4();
    let bytes = id.as_bytes();
    let mut out = String::with_capacity(n);
    let mut i = 0usize;
    while i < n {
        for b in bytes {
            if i == n {
                break;
            }
            out.push(ALPHABET[(*b % 36) as usize] as char);
            i += 1;
        }
    }
    out
}

fn lobby_players(app: &AppState, lobby_id: &str) -> Option<Vec<Value>> {
    app.db
        .query_all(
            "SELECT u.username, u.id FROM lobby_players lp JOIN users u ON lp.user_id = u.id WHERE lp.lobby_id = ? ORDER BY lp.joined_at ASC",
            [lobby_id],
            |r| Ok(json!({ "username": r.get::<_, String>(0)?, "id": r.get::<_, String>(1)? })),
        )
        .ok()
}

fn emit_lobby_updated(app: &AppState, lobby_id: &str, players: &[Value]) {
    let creator: Option<String> = app
        .db
        .query_one(
            "SELECT creator_id FROM lobbies WHERE id = ?",
            [lobby_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    app.emit_to_room(
        lobby_id,
        "lobby_updated",
        &json!({ "players": players, "creatorId": creator }),
    );
}

fn game_state_row(app: &AppState, lobby_id: &str) -> Option<Value> {
    app.db
        .query_one(
            "SELECT state_json FROM game_states WHERE lobby_id = ?",
            [lobby_id],
            |r| {
                let raw: String = r.get(0)?;
                serde_json::from_str(&raw).map_err(|_| {
                    rusqlite::Error::InvalidColumnType(
                        0,
                        "state_json".into(),
                        rusqlite::types::Type::Text,
                    )
                })
            },
        )
        .ok()
        .flatten()
}

/* ---------------------------------- auth ---------------------------------- */

async fn register(
    State(_): State<Arc<AppState>>,
    Json(body): JsonBody,
) -> Result<Json<Value>, Response> {
    let Some(username) = body_str(&body, "username") else {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Username and password are required",
        ));
    };
    let username = username.trim().to_string();
    if !(3..=32).contains(&username.chars().count()) || username.chars().any(char::is_control) {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Username must contain 3 to 32 visible characters",
        ));
    }
    let Some(password) = body_str(&body, "password") else {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Username and password are required",
        ));
    };
    if password.len() > 1024 {
        return Err(err(StatusCode::BAD_REQUEST, "Password is too long"));
    }
    if password.chars().count() < 8 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Password must be at least 8 characters long",
        ));
    }

    let app = app();
    let user_id = new_user_id();
    let password_hash = tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|_| internal_error("Password hashing failed"))?
        .map_err(|_| internal_error("Password hashing failed"))?;

    match app.db.execute(
        "INSERT INTO users (id, username, password_hash, is_guest) VALUES (?, ?, ?, 0)",
        rusqlite::params![user_id, username, password_hash],
    ) {
        Ok(_) => {}
        Err(e) if is_unique_violation(&e) => {
            return Err(err(StatusCode::CONFLICT, "Username already exists"));
        }
        Err(_) => return Err(internal_error("Internal server error")),
    }

    let token = crate::auth::generate_session_token();
    match app.db.execute(
        "INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, datetime('now', '+30 days'))",
        rusqlite::params![token, user_id],
    ) {
        Ok(_) => Ok(Json(
            json!({ "token": token, "user": { "id": user_id, "username": username, "isGuest": false } }),
        )),
        Err(_) => Err(internal_error("Internal server error")),
    }
}

async fn login(
    State(_): State<Arc<AppState>>,
    Json(body): JsonBody,
) -> Result<Json<Value>, Response> {
    let (Some(username), Some(password)) =
        (body_str(&body, "username"), body_str(&body, "password"))
    else {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Username and password are required",
        ));
    };

    if password.len() > 1024 {
        return Err(err(StatusCode::BAD_REQUEST, "Password is too long"));
    }

    let app = app();
    let user: Option<(String, String, i64, Option<String>)> = app
        .db
        .query_one(
            "SELECT id, username, is_guest, password_hash FROM users WHERE username = ?",
            [&username],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap_or_default();
    // TS: single lookup, `user.password_hash !== hashPassword(password)` → 401.
    let Some((id, username, is_guest, password_hash)) = user else {
        return Err(err(
            StatusCode::UNAUTHORIZED,
            "Invalid username or password",
        ));
    };
    let stored = password_hash.clone().unwrap_or_default();
    let legacy = !stored.starts_with("$argon2");
    let password_for_verify = password.clone();
    let verified = tokio::task::spawn_blocking(move || {
        crate::auth::verify_password(&password_for_verify, &stored)
    })
    .await
    .unwrap_or(false);
    if !verified {
        return Err(err(
            StatusCode::UNAUTHORIZED,
            "Invalid username or password",
        ));
    }

    if legacy {
        let upgraded = tokio::task::spawn_blocking(move || hash_password(&password))
            .await
            .map_err(|_| internal_error("Password hashing failed"))?
            .map_err(|_| internal_error("Password hashing failed"))?;
        app.db
            .execute(
                "UPDATE users SET password_hash = ? WHERE id = ?",
                rusqlite::params![upgraded, id],
            )
            .map_err(|_| internal_error("Account upgrade failed"))?;
    }
    let token = crate::auth::generate_session_token();
    app.db.execute(
        "INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, datetime('now', '+30 days'))",
        rusqlite::params![token, id],
    ).ok();

    Ok(Json(
        json!({ "token": token, "user": { "id": id, "username": username, "isGuest": is_guest != 0 } }),
    ))
}

async fn guest(
    State(_): State<Arc<AppState>>,
    Json(body): JsonBody,
) -> Result<Json<Value>, Response> {
    let Some(prefix) = body_str(&body, "prefix") else {
        return Err(err(StatusCode::BAD_REQUEST, "Valid prefix is required"));
    };
    let clean_prefix: String = prefix
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(16)
        .collect();
    if clean_prefix.is_empty() {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Prefix must contain alphanumeric characters",
        ));
    }

    let app = app();
    let mut username = String::new();
    let mut success = false;
    let user_id = new_user_id();

    for _ in 0..10 {
        let suffix = random_base36(5);
        username = format!("{}-{}", clean_prefix, suffix.to_uppercase());
        match app.db.execute(
            "INSERT INTO users (id, username, is_guest) VALUES (?, ?, 1)",
            rusqlite::params![user_id, username],
        ) {
            Ok(_) => {
                success = true;
                break;
            }
            Err(e) if is_unique_violation(&e) => continue,
            Err(_) => return Err(internal_error("Internal server error")),
        }
    }
    if !success {
        return Err(internal_error("Failed to generate unique guest username"));
    }

    let token = crate::auth::generate_session_token();
    app.db.execute(
        "INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, datetime('now', '+30 days'))",
        rusqlite::params![token, user_id],
    ).ok();

    Ok(Json(
        json!({ "token": token, "user": { "id": user_id, "username": username, "isGuest": true } }),
    ))
}

async fn me(State(_): State<Arc<AppState>>, auth: AuthUser) -> Result<Json<Value>, Response> {
    let app = app();
    app.db
        .execute(
            "UPDATE users SET last_active_at = CURRENT_TIMESTAMP WHERE id = ?",
            [&auth.id],
        )
        .ok();

    let guest_expires_at = if auth.is_guest != 0 && !auth.id.starts_with("ai_") {
        let expires = chrono::Utc::now() + chrono::Duration::hours(24);
        Some(expires.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
    } else {
        None
    };

    Ok(Json(json!({
        "user": {
            "id": auth.id,
            "username": auth.username,
            "is_guest": auth.is_guest,
            "isGuest": auth.is_guest != 0,
            "guestExpiresAt": guest_expires_at
        }
    })))
}

async fn logout(State(_): State<Arc<AppState>>, auth: AuthUser) -> Result<Json<Value>, Response> {
    app()
        .db
        .execute("DELETE FROM sessions WHERE id = ?", [&auth.token])
        .ok();
    app().disconnect_revoked_sessions(&auth.id);
    Ok(Json(json!({ "success": true })))
}

async fn logout_others(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<Value>, Response> {
    app()
        .db
        .execute(
            "DELETE FROM sessions WHERE user_id = ? AND id != ?",
            rusqlite::params![auth.id, auth.token],
        )
        .ok();
    app().disconnect_revoked_sessions(&auth.id);
    Ok(Json(json!({ "success": true })))
}

/// An authenticated member may replace their password without the old password.
async fn reset_password(auth: AuthUser, Json(body): JsonBody) -> Result<Json<Value>, Response> {
    if auth.is_guest != 0 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Save your guest account before setting a password",
        ));
    }
    let password = body_str(&body, "password")
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "New password is required"))?;
    if password.len() > 1024 {
        return Err(err(StatusCode::BAD_REQUEST, "Password is too long"));
    }
    if password.chars().count() < 8 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Password must be at least 8 characters long",
        ));
    }
    let hash = tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|_| internal_error("Password hashing failed"))?
        .map_err(|_| internal_error("Password hashing failed"))?;
    let app = app();
    {
        let mut conn = app.db.conn();
        let tx = conn
            .transaction()
            .map_err(|_| internal_error("Could not update password"))?;
        // Recheck after hashing, in case this session was revoked while waiting.
        let changed = tx.execute(
            "UPDATE users SET password_hash = ? WHERE id = ? AND is_guest = 0 AND EXISTS (SELECT 1 FROM sessions WHERE id = ? AND user_id = users.id AND expires_at > datetime('now'))",
            rusqlite::params![hash, auth.id, auth.token],
        ).map_err(|_| internal_error("Could not update password"))?;
        if changed != 1 {
            return Err(err(StatusCode::UNAUTHORIZED, "Please sign in again"));
        }
        tx.execute(
            "DELETE FROM sessions WHERE user_id = ? AND id != ?",
            rusqlite::params![auth.id, auth.token],
        )
        .map_err(|_| internal_error("Could not update password"))?;
        tx.commit()
            .map_err(|_| internal_error("Could not update password"))?;
    }
    app.disconnect_revoked_sessions(&auth.id);
    Ok(Json(json!({"success": true})))
}

async fn upgrade(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): JsonBody,
) -> Result<Json<Value>, Response> {
    if auth.is_guest == 0 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Account is already registered",
        ));
    }
    let Some(password) = body_str(&body, "password") else {
        return Err(err(StatusCode::BAD_REQUEST, "Password is required"));
    };
    if password.len() > 1024 {
        return Err(err(StatusCode::BAD_REQUEST, "Password is too long"));
    }
    if password.chars().count() < 8 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Password must be at least 8 characters long",
        ));
    }
    let final_username = body_str(&body, "newUsername")
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| auth.username.clone());

    let app = app();
    let password_hash = tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|_| internal_error("Password hashing failed"))?
        .map_err(|_| internal_error("Password hashing failed"))?;
    match app.db.execute(
        "UPDATE users SET username = ?, password_hash = ?, is_guest = 0 WHERE id = ?",
        rusqlite::params![final_username, password_hash, auth.id],
    ) {
        Ok(_) => Ok(Json(
            json!({ "success": true, "user": { "id": auth.id, "username": final_username, "isGuest": false } }),
        )),
        Err(e) if is_unique_violation(&e) => {
            Err(err(StatusCode::CONFLICT, "Username already exists"))
        }
        Err(_) => Err(internal_error("Internal server error")),
    }
}

/* --------------------------------- lobbies -------------------------------- */

async fn create_lobby(auth: AuthUser, Json(body): JsonBody) -> Result<Json<Value>, Response> {
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let lobby_id = random_base36(7).to_uppercase();
    let max_players = body.get("maxPlayers").and_then(Value::as_i64);
    let mode: domino_core::GameMode = body
        .get("mode")
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()
        .map_err(|_| err(StatusCode::BAD_REQUEST, "Choose 101 or 5-lik (Telefon)"))?
        .unwrap_or(domino_core::GameMode::OneOhOne);
    let mode = serde_json::to_value(mode).unwrap();
    let draw_rule: DrawRule = body
        .get("drawRule")
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()
        .map_err(|_| err(StatusCode::BAD_REQUEST, "Choose a valid draw rule"))?
        .unwrap_or_default();
    let draw_rule = serde_json::to_value(draw_rule).unwrap();

    if !matches!(max_players, Some(2..=4)) {
        return Err(err(StatusCode::BAD_REQUEST, "Choose 2, 3, or 4 players"));
    }
    match app.db.execute(
        "INSERT INTO lobbies (id, creator_id, max_players, draw_rule, game_mode) VALUES (?, ?, ?, ?, ?)",
        rusqlite::params![lobby_id, auth.id, max_players, draw_rule.as_str().unwrap(), mode.as_str().unwrap()],
    ) {
        Ok(_) => {}
        Err(_) => return Err(internal_error("Failed to create lobby")),
    }
    match app.db.execute(
        "INSERT INTO lobby_players (lobby_id, user_id) VALUES (?, ?)",
        rusqlite::params![lobby_id, auth.id],
    ) {
        Ok(_) => Ok(Json(json!({ "id": lobby_id }))),
        Err(_) => Err(internal_error("Failed to create lobby")),
    }
}

async fn join_lobby(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
    Path(lobby_id_raw): Path<String>,
) -> Result<Json<Value>, Response> {
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let lobby_id = lobby_id_raw.to_uppercase();
    let Some(lobby) = app.db.query_one(
        "SELECT * FROM lobbies WHERE id = ?",
        [&lobby_id],
        |r: &rusqlite::Row<'_>| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "creator_id": r.get::<_, Option<String>>(1).unwrap_or_default(),
                "max_players": r.get::<_, i64>(2)?,
                "draw_rule": r.get::<_, String>("draw_rule")?,
                "mode": r.get::<_, String>("game_mode")?,
                "status": r.get::<_, Option<String>>(3).unwrap_or_default().unwrap_or_else(|| "waiting".into())
            }))
        },
    ).unwrap_or_default() else {
        if app.db.query_one("SELECT 1 FROM uno_tables WHERE id = ?", [&lobby_id], |_| Ok(())).ok().flatten().is_some() {
            return Err((StatusCode::CONFLICT, Json(json!({"error":"This table belongs to UnoGo", "code":"WRONG_GAME", "gameId":"unogo", "url":format!("https://uno.driveatbaku.games/?join={lobby_id}")}))).into_response());
        }
        return Err(err(StatusCode::NOT_FOUND, "Lobby not found"));
    };
    let creator = lobby["creator_id"].as_str().unwrap_or_default().to_string();
    let max_players = lobby["max_players"].as_i64().unwrap_or(0);
    let lobby_status = lobby["status"].as_str().unwrap_or("waiting").to_string();

    let players = lobby_players(&app, &lobby_id).unwrap_or_default();
    let is_player_in_lobby = players.iter().any(|p| p["id"] == json!(auth.id));

    if (players.len() as i64) >= max_players && !is_player_in_lobby {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Lobby is full","code":"TABLE_FULL"})),
        )
            .into_response());
    }
    if lobby_status != "waiting" && !is_player_in_lobby {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"Game already started","code":"TABLE_STARTED"})),
        )
            .into_response());
    }

    let inserted = app
        .db
        .execute(
            "INSERT OR IGNORE INTO lobby_players (lobby_id, user_id) VALUES (?, ?)",
            rusqlite::params![lobby_id, auth.id],
        )
        .is_ok();

    if !inserted {
        return Err(internal_error("Failed to join lobby"));
    }

    let players = lobby_players(&app, &lobby_id).unwrap_or_default();
    emit_lobby_updated(&app, &lobby_id, &players);

    let mut game_status = lobby_status;
    if let Some(state) = game_state_row(&app, &lobby_id) {
        if let Some(status) = state.get("status").and_then(Value::as_str) {
            game_status = status.to_string();
        }
    }
    Ok(Json(json!({
        "success": true,
        "players": players,
        "maxPlayers": max_players,
        "drawRule": lobby["draw_rule"],
        "mode": lobby["mode"],
        "creatorId": creator,
        "status": game_status
    })))
}

async fn leave_lobby(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
    Path(lobby_id_raw): Path<String>,
) -> Result<Json<Value>, Response> {
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let lobby_id = lobby_id_raw.to_uppercase();
    // Leaving the screen must not destroy a live match or strand its turn.
    if let Some(state) = ops::load_state(&app, &lobby_id) {
        if matches!(
            state["status"].as_str(),
            Some("playing" | "round_end" | "stopped")
        ) {
            return Ok(Json(json!({"success":true})));
        }
    }
    app.db
        .execute(
            "DELETE FROM lobby_players WHERE lobby_id = ? AND user_id = ?",
            rusqlite::params![lobby_id, auth.id],
        )
        .ok();

    let players = lobby_players(&app, &lobby_id).unwrap_or_default();
    let humans: Vec<_> = players
        .iter()
        .filter(|p| !p["id"].as_str().unwrap_or("").starts_with("ai_"))
        .collect();
    if humans.is_empty() {
        ops::clear_turn_timer(&app, &lobby_id);
        app.db
            .execute("DELETE FROM lobbies WHERE id = ?", [&lobby_id])
            .ok();
    } else {
        app.db
            .execute(
                "UPDATE lobbies SET creator_id = ? WHERE id = ? AND creator_id = ?",
                rusqlite::params![humans[0]["id"].as_str().unwrap_or(""), lobby_id, auth.id],
            )
            .ok();
        emit_lobby_updated(&app, &lobby_id, &players);
    }
    Ok(Json(json!({ "success": true })))
}

async fn add_bot(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
    Path(lobby_id_raw): Path<String>,
) -> Result<Json<Value>, Response> {
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let lobby_id = lobby_id_raw.to_uppercase();
    let Some(lobby) = app.db.query_one(
        "SELECT * FROM lobbies WHERE id = ?",
        [&lobby_id],
        |r| Ok(json!({ "creator_id": r.get::<_, Option<String>>(1).unwrap_or_default(), "status": r.get::<_, Option<String>>(3).unwrap_or_default().unwrap_or_else(|| "waiting".into()) })),
    ).unwrap_or_default() else {
        return Err(err(StatusCode::NOT_FOUND, "Lobby not found"));
    };
    if lobby["creator_id"] != json!(auth.id) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Only creator can add bots" })),
        )
            .into_response());
    }
    if lobby["status"] != json!("waiting") {
        return Err(err(StatusCode::BAD_REQUEST, "Game already started"));
    }
    let players = lobby_players(&app, &lobby_id).unwrap_or_default();
    let max_players: i64 = app
        .db
        .query_one(
            "SELECT max_players FROM lobbies WHERE id = ?",
            [&lobby_id],
            |r| r.get(0),
        )
        .unwrap_or_default()
        .unwrap_or(0);
    if players.len() as i64 >= max_players {
        return Err(err(StatusCode::BAD_REQUEST, "Lobby is full"));
    }

    let mut bot_num = 1usize;
    while players
        .iter()
        .any(|p| p["id"] == json!(format!("ai_{bot_num}")))
    {
        bot_num += 1;
    }
    let bot_id = format!("ai_{bot_num}");

    if app
        .db
        .execute(
            "INSERT INTO lobby_players (lobby_id, user_id) VALUES (?, ?)",
            rusqlite::params![lobby_id, bot_id],
        )
        .is_ok()
    {
        let players = lobby_players(&app, &lobby_id).unwrap_or_default();
        emit_lobby_updated(&app, &lobby_id, &players);
        return Ok(Json(json!({ "success": true, "players": players })));
    }
    Err(internal_error("Failed to add bot"))
}

async fn kick(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
    Path(lobby_id_raw): Path<String>,
    Json(body): JsonBody,
) -> Result<Json<Value>, Response> {
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let lobby_id = lobby_id_raw.to_uppercase();
    let Some(lobby) = app.db.query_one(
        "SELECT * FROM lobbies WHERE id = ?",
        [&lobby_id],
        |r| Ok(json!({ "creator_id": r.get::<_, Option<String>>(1).unwrap_or_default(), "status": r.get::<_, Option<String>>(3).unwrap_or_default().unwrap_or_else(|| "waiting".into()) })),
    ).unwrap_or_default() else {
        return Err(err(StatusCode::NOT_FOUND, "Lobby not found"));
    };
    if lobby["creator_id"] != json!(auth.id) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Only creator can kick players" })),
        )
            .into_response());
    }
    if lobby["status"] != json!("waiting") {
        return Err(err(StatusCode::BAD_REQUEST, "Game already started"));
    }
    let Some(target_user_id) = body_str(&body, "targetUserId") else {
        return Err(internal_error("Internal server error"));
    };
    if target_user_id == auth.id {
        return Err(err(StatusCode::BAD_REQUEST, "Cannot kick yourself"));
    }

    app.db
        .execute(
            "DELETE FROM lobby_players WHERE lobby_id = ? AND user_id = ?",
            rusqlite::params![lobby_id, target_user_id],
        )
        .ok();
    let players = lobby_players(&app, &lobby_id).unwrap_or_default();
    emit_lobby_updated(&app, &lobby_id, &players);
    Ok(Json(json!({ "success": true, "players": players })))
}

async fn start_game(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
    Path(lobby_id_raw): Path<String>,
) -> Result<Json<Value>, Response> {
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let lobby_id = lobby_id_raw.to_uppercase();
    let creator: Option<String> = app
        .db
        .query_one(
            "SELECT creator_id FROM lobbies WHERE id = ?",
            [&lobby_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let Some(creator) = creator else {
        return Err(err(StatusCode::NOT_FOUND, "Lobby not found"));
    };
    if creator != auth.id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Only creator can start" })),
        )
            .into_response());
    }

    let player_ids: Vec<String> = app
        .db
        .query_all(
            "SELECT user_id FROM lobby_players WHERE lobby_id = ? ORDER BY joined_at ASC",
            [&lobby_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if !(2..=4).contains(&player_ids.len()) {
        return Err(err(StatusCode::BAD_REQUEST, "A game needs 2 to 4 players"));
    }
    let status: Option<String> = app
        .db
        .query_one(
            "SELECT status FROM lobbies WHERE id = ?",
            [&lobby_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if status.as_deref() != Some("waiting") {
        return Err(err(StatusCode::BAD_REQUEST, "Game already started"));
    }
    let player_ids: Vec<&str> = player_ids.iter().map(String::as_str).collect();
    let game = ops::create_table_game(&app, &lobby_id, &player_ids)
        .map_err(|_| internal_error("Could not load table rules"))?;
    let mut state_value = serde_json::to_value(game).unwrap_or_default();
    state_value["matchId"] = json!(uuid::Uuid::new_v4().to_string());

    {
        let mut conn = app.db.conn();
        let tx = conn
            .transaction()
            .map_err(|_| internal_error("Could not start game"))?;
        tx.execute(
            "INSERT INTO game_states(lobby_id,state_json) VALUES (?,?)",
            rusqlite::params![lobby_id, state_value.to_string()],
        )
        .map_err(|_| internal_error("Could not start game"))?;
        tx.execute(
            "UPDATE lobbies SET status='playing' WHERE id=?",
            [&lobby_id],
        )
        .map_err(|_| internal_error("Could not start game"))?;
        tx.commit()
            .map_err(|_| internal_error("Could not start game"))?;
    }

    app.emit_to_room(&lobby_id, "game_started", &json!({}));
    ops::start_turn_timer(&app, &lobby_id, &state_value);
    app.broadcast_state(&lobby_id, &state_value);
    ops::check_and_run_ai_turn(&app, &lobby_id, &state_value);

    Ok(Json(json!({ "success": true })))
}

/* -------------------- match history + stats + leaderboard ------------------ */

async fn match_history(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<Value>, Response> {
    let app = app();
    let rows: Vec<Value> = app
        .db
        .query_all(
            "SELECT l.id, l.status, strftime('%Y-%m-%dT%H:%M:%SZ', l.created_at) as created_at,
                    strftime('%Y-%m-%dT%H:%M:%SZ', gs.updated_at) as updated_at, gs.state_json,
                    (SELECT COUNT(*) FROM lobby_players WHERE lobby_id = l.id) as player_count
             FROM lobbies l
             JOIN lobby_players lp ON l.id = lp.lobby_id
             JOIN game_states gs ON l.id = gs.lobby_id
             LEFT JOIN hidden_match_history hmh ON hmh.lobby_id = l.id AND hmh.user_id = ?
             WHERE lp.user_id = ?
             AND hmh.lobby_id IS NULL
             ORDER BY COALESCE(gs.updated_at, l.created_at) DESC
             LIMIT 20",
            rusqlite::params![auth.id, auth.id],
            |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0).ok(),
                    "status": r.get::<_, Option<String>>(1).ok().flatten(),
                    "created_at": r.get::<_, Option<String>>(2).ok().flatten(),
                    "updated_at": r.get::<_, Option<String>>(3).ok().flatten(),
                    "state_json": r.get::<_, Option<String>>(4).ok().flatten(),
                    "player_count": r.get::<_, i64>(5).unwrap_or(0),
                }))
            },
        )
        .unwrap_or_default();

    let mut matches: Vec<Value> = Vec::new();
    for m in rows {
        let mut scores: Option<Value> = None;
        let mut mode = json!("101");
        let mut winner: Option<String> = None;
        let mut game_status = m["status"].clone();
        if let Some(raw) = m["state_json"].as_str() {
            if let Ok(state) = serde_json::from_str::<Value>(raw) {
                scores = state.get("scores").cloned();
                mode = state.get("mode").cloned().unwrap_or(json!("101"));
                if let Some(status) = state.get("status").and_then(Value::as_str) {
                    game_status = json!(status);
                }
                if state["status"] == json!("finished") {
                    if let Some(players) = state["players"].as_array() {
                        for p in players {
                            let id = p.as_str().unwrap_or_default().to_string();
                            let perm = state["scores"][&id]["permanent"].as_i64().unwrap_or(0);
                            if perm >= if state["mode"] == "telefon" { 365 } else { 101 } {
                                winner = Some(id);
                                break;
                            }
                        }
                    }
                }
            }
        }
        let id = m["id"].as_str().unwrap_or_default().to_string();
        let players = app
            .db
            .query_all(
                "SELECT u.id, u.username FROM lobby_players lp JOIN users u ON lp.user_id = u.id WHERE lp.lobby_id = ? ORDER BY lp.joined_at ASC",
                [&id],
                |r| Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "username": r.get::<_, String>(1)?
                })),
            )
            .unwrap_or_default();

        matches.push(json!({
            "id": m["id"],
            "status": game_status,
            "created_at": m["created_at"],
            "updated_at": m["updated_at"],
            "player_count": m["player_count"],
            "players": players,
            "scores": scores,
            "mode": mode,
            "winner": winner
        }));
    }

    let archived = app.db.query_all("SELECT cm.summary_json FROM completed_matches cm JOIN completed_match_players cp ON cp.match_id = cm.id WHERE cp.user_id = ? AND cp.hidden = 0 ORDER BY cm.updated_at DESC LIMIT 20", [&auth.id], |r| r.get::<_,String>(0)).unwrap_or_default();
    matches.extend(
        archived
            .into_iter()
            .filter_map(|raw| serde_json::from_str::<Value>(&raw).ok()),
    );
    matches.sort_by(|a, b| b["updated_at"].as_str().cmp(&a["updated_at"].as_str()));
    matches.truncate(20);
    Ok(Json(json!({ "matches": matches })))
}

async fn delete_match(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>, Response> {
    let app = app();
    let lobby_id = id.to_uppercase();
    let archived = app
        .db
        .execute(
            "UPDATE completed_match_players SET hidden = 1 WHERE match_id = ? AND user_id = ?",
            rusqlite::params![lobby_id, auth.id],
        )
        .unwrap_or(0);
    if archived > 0 {
        return Ok(Json(json!({"success":true})));
    }
    let member = app
        .db
        .query_one(
            "SELECT 1 FROM lobby_players WHERE lobby_id = ? AND user_id = ?",
            rusqlite::params![lobby_id, auth.id],
            |_| Ok(true),
        )
        .unwrap_or_default();
    if member.is_none() {
        return Err(err(StatusCode::NOT_FOUND, "Match not found"));
    }
    app.db
        .execute(
            "INSERT OR IGNORE INTO hidden_match_history (user_id, lobby_id) VALUES (?, ?)",
            rusqlite::params![auth.id, lobby_id],
        )
        .ok();
    Ok(Json(json!({ "success": true })))
}

async fn get_stats(Path(user_id): Path<String>) -> Result<Json<Value>, Response> {
    let app = app();
    let stats: Option<(i64, i64, i64, i64, String)> = app
        .db
        .query_one(
            "SELECT ps.wins, ps.losses, ps.total_score, ps.games_played, u.username
         FROM player_stats ps JOIN users u ON ps.user_id = u.id WHERE ps.user_id = ?",
            [&user_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap_or_default();

    let Some((wins, losses, total_score, games_played, username)) = stats else {
        let username: String = app
            .db
            .query_one("SELECT username FROM users WHERE id = ?", [&user_id], |r| {
                r.get(0)
            })
            .unwrap_or_default()
            .unwrap_or_else(|| "Unknown".into());
        return Ok(Json(json!({
            "username": username, "wins": 0, "losses": 0, "totalScore": 0,
            "gamesPlayed": 0, "winRate": 0, "avgScore": 0
        })));
    };

    Ok(Json(json!({
        "username": username,
        "wins": wins,
        "losses": losses,
        "totalScore": total_score,
        "gamesPlayed": games_played,
        "winRate": if games_played > 0 { ((wins as f64 * 100.0) / games_played as f64).round() as i64 } else { 0 },
        "avgScore": if games_played > 0 { (total_score as f64 / games_played as f64).round() as i64 } else { 0 }
    })))
}

#[derive(Default, serde::Deserialize)]
struct LeaderboardQuery {
    period: Option<String>,
}

/// Half-open UTC calendar windows. Weeks begin on Monday.
pub(crate) fn ranking_window(
    period: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<(i64, i64)> {
    use chrono::{Datelike, Duration, TimeZone, Utc};
    let day = now.date_naive();
    let (start, end) = match period {
        "day" => (day, day.checked_add_signed(Duration::days(1))?),
        "week" => {
            let start = day
                .checked_sub_signed(Duration::days(day.weekday().num_days_from_monday().into()))?;
            (start, start.checked_add_signed(Duration::days(7))?)
        }
        "month" => {
            let start = day.with_day(1)?;
            let (year, month) = if day.month() == 12 {
                (day.year() + 1, 1)
            } else {
                (day.year(), day.month() + 1)
            };
            (start, chrono::NaiveDate::from_ymd_opt(year, month, 1)?)
        }
        _ => return None,
    };
    Some((
        Utc.from_utc_datetime(&start.and_hms_opt(0, 0, 0)?)
            .timestamp_millis(),
        Utc.from_utc_datetime(&end.and_hms_opt(0, 0, 0)?)
            .timestamp_millis(),
    ))
}

type RankingRow = (String, String, i64, i64, i64, i64);
fn leaderboard_rows(
    db: &crate::db::Db,
    bounds: Option<(i64, i64)>,
) -> rusqlite::Result<Vec<RankingRow>> {
    let source = if bounds.is_some() {
        "(SELECT ds.user_id,SUM(ds.wins) AS wins,SUM(ds.losses) AS losses,SUM(ds.total_score) AS total_score,COUNT(*) AS games_played FROM dated_player_stats ds JOIN leaderboard_match_completions mc ON mc.match_id=ds.match_id WHERE mc.completed_at_ms >= ?1 AND mc.completed_at_ms < ?2 GROUP BY ds.user_id)"
    } else {
        "player_stats"
    };
    let sql = format!("SELECT ps.user_id,u.username,ps.wins,ps.losses,ps.total_score,ps.games_played FROM {source} ps JOIN users u ON ps.user_id=u.id WHERE u.is_guest=0 AND ps.user_id NOT LIKE 'ai_%' AND ps.games_played >= 1 ORDER BY ps.wins DESC, CAST(ps.wins AS FLOAT)/ps.games_played DESC, ps.total_score DESC, ps.user_id ASC LIMIT 50");
    let map = |r: &rusqlite::Row<'_>| {
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            r.get(4)?,
            r.get(5)?,
        ))
    };
    match bounds {
        Some((start, end)) => db.query_all(&sql, rusqlite::params![start, end], map),
        None => db.query_all(&sql, [], map),
    }
}

async fn leaderboard(Query(query): Query<LeaderboardQuery>) -> Result<Json<Value>, Response> {
    let app = app();
    let period = query.period.as_deref().unwrap_or("all");
    if !matches!(period, "day" | "week" | "month" | "all") {
        return Err(err(StatusCode::BAD_REQUEST, "Invalid leaderboard period"));
    }
    let bounds = ranking_window(period, chrono::Utc::now());
    let rows = leaderboard_rows(&app.db, bounds)
        .map_err(|_| internal_error("Could not load leaderboard"))?;
    let tracking_started_at: String = app
        .db
        .query_one(
            "SELECT value FROM ranking_metadata WHERE key='tracking_started_at'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| internal_error("Could not load leaderboard metadata"))?
        .ok_or_else(|| internal_error("Missing leaderboard metadata"))?;
    let timestamp = |ms| {
        chrono::DateTime::from_timestamp_millis(ms)
            .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
    };
    let leaderboard: Vec<Value> = rows
        .into_iter()
        .enumerate()
        .map(|(idx, (user_id, username, wins, losses, total_score, games_played))| {
            json!({
                "rank": idx + 1,
                "userId": user_id,
                "username": username,
                "wins": wins,
                "losses": losses,
                "gamesPlayed": games_played,
                "winRate": if games_played > 0 { ((wins as f64 * 100.0) / games_played as f64).round() as i64 } else { 0 },
                "totalScore": total_score,
                "avgScore": if games_played > 0 { (total_score as f64 / games_played as f64).round() as i64 } else { 0 }
            })
        })
        .collect();

    Ok(Json(
        json!({ "leaderboard": leaderboard, "period":period,"timezone":"UTC","trackingStartedAt":tracking_started_at,"periodStart":bounds.and_then(|b|timestamp(b.0)),"periodEnd":bounds.and_then(|b|timestamp(b.1)) }),
    ))
}

/* --------------------------------- friends -------------------------------- */

async fn friend_request(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): JsonBody,
) -> Result<Json<Value>, Response> {
    let app = app();
    if auth.is_guest != 0 {
        return Err(err(
            StatusCode::FORBIDDEN,
            "Create an account to use the friend system",
        ));
    }
    let Some(username) = body_str(&body, "username") else {
        return Err(err(StatusCode::BAD_REQUEST, "Username is required"));
    };

    let target: Option<(String, i64)> = app
        .db
        .query_one(
            "SELECT id, is_guest FROM users WHERE username = ?",
            [&username],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or_default();
    let Some((target_id, target_guest)) = target else {
        return Err(err(StatusCode::NOT_FOUND, "User not found"));
    };
    if target_guest != 0 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Cannot add guest users as friends",
        ));
    }
    if target_id == auth.id {
        return Err(err(StatusCode::BAD_REQUEST, "Cannot add yourself"));
    }

    let existing: Option<String> = app.db.query_one(
        "SELECT status FROM friendships WHERE (sender_id = ? AND receiver_id = ?) OR (sender_id = ? AND receiver_id = ?)",
        rusqlite::params![auth.id, target_id, target_id, auth.id],
        |r| r.get(0),
    ).unwrap_or_default();
    if let Some(status) = existing {
        if status == "accepted" {
            return Err(err(StatusCode::BAD_REQUEST, "Already friends"));
        }
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Friend request already pending",
        ));
    }

    app.db
        .execute(
            "INSERT INTO friendships (sender_id, receiver_id) VALUES (?, ?)",
            rusqlite::params![auth.id, target_id],
        )
        .ok();
    app.emit_to_user(
        &target_id,
        "friend_request_received",
        &json!({ "id": auth.id, "username": auth.username }),
    );
    Ok(Json(json!({ "success": true })))
}

async fn friend_accept(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): JsonBody,
) -> Result<Json<Value>, Response> {
    friendship_action(
        body.get("friendshipId").and_then(Value::as_i64),
        auth,
        "accept",
    )
    .await
}

async fn friend_reject(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): JsonBody,
) -> Result<Json<Value>, Response> {
    friendship_action(
        body.get("friendshipId").and_then(Value::as_i64),
        auth,
        "reject",
    )
    .await
}

async fn friend_cancel(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): JsonBody,
) -> Result<Json<Value>, Response> {
    friendship_action(
        body.get("friendshipId").and_then(Value::as_i64),
        auth,
        "cancel",
    )
    .await
}

async fn friendship_action(
    friendship_id: Option<i64>,
    auth: AuthUser,
    action: &str,
) -> Result<Json<Value>, Response> {
    let app = app();
    let Some(friendship_id) = friendship_id else {
        return Err(err(StatusCode::NOT_FOUND, "Request not found"));
    };

    if action == "accept" {
        let row: Option<(String,)> = app.db.query_one(
            "SELECT sender_id FROM friendships WHERE id = ? AND receiver_id = ? AND status = 'pending'",
            rusqlite::params![friendship_id, auth.id],
            |r| Ok((r.get(0)?,)),
        ).unwrap_or_default();
        let Some((sender_id,)) = row else {
            return Err(err(StatusCode::NOT_FOUND, "Request not found"));
        };
        app.db
            .execute(
                "UPDATE friendships SET status = ? WHERE id = ?",
                rusqlite::params!["accepted", friendship_id],
            )
            .ok();
        app.emit_to_user(
            &sender_id,
            "friend_request_accepted",
            &json!({ "id": auth.id, "username": auth.username }),
        );
        return Ok(Json(json!({ "success": true })));
    }

    if action == "reject" {
        let exists: Option<i64> = app.db.query_one(
            "SELECT id FROM friendships WHERE id = ? AND receiver_id = ? AND status = 'pending'",
            rusqlite::params![friendship_id, auth.id],
            |r| r.get(0),
        ).unwrap_or_default();
        if exists.is_none() {
            return Err(err(StatusCode::NOT_FOUND, "Request not found"));
        }
        app.db
            .execute("DELETE FROM friendships WHERE id = ?", [friendship_id])
            .ok();
        return Ok(Json(json!({ "success": true })));
    }

    // cancel — sender side
    let exists: Option<i64> = app
        .db
        .query_one(
            "SELECT id FROM friendships WHERE id = ? AND sender_id = ? AND status = 'pending'",
            rusqlite::params![friendship_id, auth.id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if exists.is_none() {
        return Err(err(StatusCode::NOT_FOUND, "Request not found"));
    }
    app.db
        .execute("DELETE FROM friendships WHERE id = ?", [friendship_id])
        .ok();
    Ok(Json(json!({ "success": true })))
}

async fn friend_remove(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
    Path(target_user_id): Path<String>,
) -> Result<Json<Value>, Response> {
    let app = app();
    let changes = app
        .db
        .execute(
            "DELETE FROM friendships WHERE status = 'accepted' AND ((sender_id = ? AND receiver_id = ?) OR (sender_id = ? AND receiver_id = ?))",
            rusqlite::params![auth.id, target_user_id, target_user_id, auth.id],
        )
        .unwrap_or(0);

    if changes == 0 {
        return Err(err(StatusCode::NOT_FOUND, "Friendship not found"));
    }

    app.emit_to_user(
        &target_user_id,
        "friend_removed",
        &json!({ "userId": auth.id }),
    );
    Ok(Json(json!({ "success": true })))
}

async fn friends_list(
    State(_): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<Value>, Response> {
    let app = app();
    if auth.is_guest != 0 {
        return Ok(Json(
            json!({ "friends": [], "incoming": [], "outgoing": [], "pendingGameInvites": [] }),
        ));
    }

    let friend_rows: Vec<(i64, String, String)> = app
        .db
        .query_all(
            "SELECT f.id, CASE WHEN f.sender_id = ? THEN f.receiver_id ELSE f.sender_id END as friend_id,
                    CASE WHEN f.sender_id = ? THEN u2.username ELSE u1.username END as username
             FROM friendships f
             JOIN users u1 ON f.sender_id = u1.id
             JOIN users u2 ON f.receiver_id = u2.id
             WHERE f.status = 'accepted' AND (f.sender_id = ? OR f.receiver_id = ?)",
            rusqlite::params![auth.id, auth.id, auth.id, auth.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or_default();

    let friends: Vec<Value> = friend_rows
        .into_iter()
        // TS shape: { id: friend_id, username, isOnline } — NOT the friendship row id.
        .map(|(_friendship_id, friend_id, username)| {
            let is_online = app
                .registry
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .by_user
                .get(&friend_id)
                .map(|e| !e.is_empty())
                .unwrap_or(false) || app.db.query_one("SELECT 1 FROM users WHERE id=? AND last_active_at>datetime('now','-20 seconds')",[&friend_id],|_|Ok(())).ok().flatten().is_some();
            json!({ "id": friend_id, "username": username, "isOnline": is_online })
        })
        .collect();

    let incoming: Vec<Value> = app
        .db
        .query_all(
            "SELECT f.id, f.sender_id as userId, u.username FROM friendships f JOIN users u ON f.sender_id = u.id WHERE f.receiver_id = ? AND f.status = 'pending'",
            [&auth.id],
            |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "userId": r.get::<_, String>(1)?, "username": r.get::<_, String>(2)? })),
        )
        .unwrap_or_default();

    let outgoing: Vec<Value> = app
        .db
        .query_all(
            "SELECT f.id, f.receiver_id as userId, u.username FROM friendships f JOIN users u ON f.receiver_id = u.id WHERE f.sender_id = ? AND f.status = 'pending'",
            [&auth.id],
            |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "userId": r.get::<_, String>(1)?, "username": r.get::<_, String>(2)? })),
        )
        .unwrap_or_default();

    let pending_game_invites: Vec<Value> = app
        .db
        .query_all(
            "SELECT gi.id, gi.lobby_id as lobbyId, u.username as senderUsername FROM game_invitations gi JOIN users u ON gi.sender_id = u.id
             WHERE gi.receiver_id = ? AND gi.status = 'pending'
             AND gi.lobby_id IN (SELECT id FROM lobbies WHERE status IN ('waiting', 'playing'))",
            [&auth.id],
            |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "lobbyId": r.get::<_, String>(1)?, "senderUsername": r.get::<_, String>(2)? })),
        )
        .unwrap_or_default();

    Ok(Json(
        json!({ "friends": friends, "incoming": incoming, "outgoing": outgoing, "pendingGameInvites": pending_game_invites }),
    ))
}

#[cfg(test)]
mod ranking_tests {
    use super::*;
    fn utc(s: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }
    #[test]
    fn utc_periods_cover_leap_day_monday_and_year_rollover() {
        for (period, now, start, end) in [
            (
                "day",
                "2024-02-29T23:59:59Z",
                "2024-02-29T00:00:00Z",
                "2024-03-01T00:00:00Z",
            ),
            (
                "week",
                "2026-11-01T23:59:59Z",
                "2026-10-26T00:00:00Z",
                "2026-11-02T00:00:00Z",
            ),
            (
                "week",
                "2026-11-02T00:00:00Z",
                "2026-11-02T00:00:00Z",
                "2026-11-09T00:00:00Z",
            ),
            (
                "month",
                "2026-12-31T23:59:59Z",
                "2026-12-01T00:00:00Z",
                "2027-01-01T00:00:00Z",
            ),
        ] {
            assert_eq!(
                ranking_window(period, utc(now)),
                Some((utc(start).timestamp_millis(), utc(end).timestamp_millis()))
            );
        }
        assert_eq!(ranking_window("all", utc("2026-11-02T00:00:00Z")), None);
    }
    #[test]
    fn dated_queries_are_half_open_and_lifetime_remains_legacy_inclusive() {
        let db = crate::db::Db::open_in_memory().unwrap();
        db.execute(
            "INSERT INTO users(id,username) VALUES ('human','Human')",
            [],
        )
        .unwrap();
        db.execute("INSERT INTO player_stats(user_id,wins,losses,total_score,games_played) VALUES ('human',9,1,1000,10)",[]).unwrap();
        for (id, at) in [
            ("before", 99),
            ("start", 100),
            ("inside", 199),
            ("end", 200),
        ] {
            db.execute(
                "INSERT INTO leaderboard_match_completions VALUES (?,?)",
                rusqlite::params![id, at],
            )
            .unwrap();
            db.execute(
                "INSERT INTO dated_player_stats VALUES (?,'human',1,0,20)",
                [id],
            )
            .unwrap();
        }
        let daily = leaderboard_rows(&db, Some((100, 200))).unwrap();
        assert_eq!((daily[0].2, daily[0].4, daily[0].5), (2, 40, 2));
        let all = leaderboard_rows(&db, None).unwrap();
        assert_eq!((all[0].2, all[0].4, all[0].5), (9, 1000, 10));
        let tracked: String = db
            .query_one(
                "SELECT value FROM ranking_metadata WHERE key='tracking_started_at'",
                [],
                |r| r.get(0),
            )
            .unwrap()
            .unwrap();
        assert!(chrono::DateTime::parse_from_rfc3339(&tracked).is_ok());
    }
}
