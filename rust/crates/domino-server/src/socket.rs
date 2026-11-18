//! Authenticated realtime lobby, game, invitation, and presence handlers.

use crate::domain::{app, from_core, sanitize_state, status_of, AppState};
use crate::ops;
use domino_core::{check_game_over, draw_from_market, pass_turn, play_tile, End, GameStatus, Tile};
use serde_json::{json, Value};
use socketioxide::extract::{AckSender, Data, SocketRef};

/// Handshake auth payload — `socket.handshake.auth.token` in TS.
#[derive(serde::Deserialize)]
pub(crate) struct ConnectAuth {
    token: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LobbyIdPayload {
    lobby_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayTilePayload {
    lobby_id: String,
    tile: Tile,
    end: End,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReactionPayload {
    lobby_id: String,
    emoji: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct InvitePayload {
    lobby_id: String,
    friend_user_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct InviteIdPayload {
    invite_id: i64,
}

fn user_of(socket: &SocketRef) -> Option<String> {
    let app = app();
    if !app.games.domino.enabled() {
        return None;
    }
    let (uid, token) = {
        let registry = app.registry.read().unwrap_or_else(|e| e.into_inner());
        let session = registry.by_socket.get(&socket.id)?;
        (session.user_id.clone(), session.token.clone())
    };
    if app.db.query_one("SELECT 1 FROM sessions WHERE id = ? AND user_id = ? AND (expires_at IS NULL OR expires_at > datetime('now'))", rusqlite::params![token, uid], |_| Ok(())).ok().flatten().is_none() {
        let _ = socket.clone().disconnect(); return None;
    }
    app.db
        .execute(
            "UPDATE users SET last_active_at = CURRENT_TIMESTAMP WHERE id = ?",
            [&uid],
        )
        .ok();
    Some(uid)
}

fn is_member(app: &AppState, lobby: &str, user: &str) -> bool {
    app.db
        .query_one(
            "SELECT 1 FROM lobby_players WHERE lobby_id = ? AND user_id = ?",
            rusqlite::params![lobby, user],
            |_| Ok(()),
        )
        .ok()
        .flatten()
        .is_some()
}

fn parse_payload<T: serde::de::DeserializeOwned>(
    raw: Value,
    ack: AckSender,
) -> Option<(T, AckSender)> {
    match serde_json::from_value(raw) {
        Ok(v) => Some((v, ack)),
        Err(_) => {
            let _ = ack.send(&json!({"error":"Invalid payload"}));
            None
        }
    }
}

const ALLOWED_REACTIONS: [&str; 16] = domino_core::REACTIONS;

/// TS `io.use` rejection error — its Display IS the `connect_error` message.
#[derive(Debug)]
pub(crate) struct AuthError;
impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Authentication error")
    }
}
impl std::error::Error for AuthError {}

/// Reject unknown or expired sessions, then register the socket fully.
///
/// Registration happens here rather than in [`on_connect`] on purpose:
/// socketioxide sends the CONNECT packet after middleware succeeds and then
/// *spawns* the connect handler. Events a client sends immediately (the
/// browser's `join_lobby` on reconnect) could otherwise reach a socket with
/// no listeners and be dropped silently, leaving the table without state.
pub async fn auth_middleware(
    socket: SocketRef,
    Data(auth): Data<ConnectAuth>,
) -> Result<(), AuthError> {
    let app = app();
    let user_id: Option<String> = app
        .db
        .query_one("SELECT user_id FROM sessions WHERE id = ? AND (expires_at IS NULL OR expires_at > datetime('now'))", [&auth.token], |r| r.get(0))
        .unwrap_or_default();
    let Some(user_id) = user_id else {
        return Err(AuthError);
    };

    // Register the session; detect the user's FIRST socket (presence).
    let first_socket = {
        let mut reg = app.registry.write().unwrap_or_else(|e| e.into_inner());
        let first = reg.by_user.get(&user_id).is_none_or(|s| s.is_empty());
        reg.by_socket.insert(
            socket.id,
            crate::domain::Sess {
                user_id: user_id.clone(),
                token: auth.token.clone(),
                handle: socket.clone(),
                lobbies: Default::default(),
            },
        );
        reg.by_user
            .entry(user_id.clone())
            .or_default()
            .insert(socket.id);
        first
    };

    register_events(&socket);

    if first_socket {
        let friends: Vec<String> = app
            .db
            .query_all(
                "SELECT CASE WHEN sender_id = ? THEN receiver_id ELSE sender_id END as friend_id
                 FROM friendships WHERE status = 'accepted' AND (sender_id = ? OR receiver_id = ?)",
                [&user_id, &user_id, &user_id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        for fid in friends {
            app.emit_to_user(&fid, "friend_online", &json!({ "userId": user_id }));
        }
    }
    Ok(())
}

/// Runs after the client has been told it is connected.
pub async fn on_connect(socket: SocketRef) {
    let app = app();
    let user_id = {
        let registry = app.registry.read().unwrap_or_else(|e| e.into_inner());
        registry.by_socket.get(&socket.id).map(|s| s.user_id.clone())
    };
    let Some(user_id) = user_id else { return };
    // Preferences belong to the authenticated account, never the table.
    if let Ok(counts) = app.db.reaction_usage(&user_id) {
        let _ = socket.emit("emote_usage", &json!({"userId":user_id,"counts":counts}));
    }
}

fn register_events(socket: &SocketRef) {
    socket.on("join_lobby", on_join_lobby);
    socket.on("leave_lobby", on_leave_lobby);
    socket.on("play_tile", on_play_tile);
    socket.on("draw_market", on_draw_market);
    socket.on("pass_turn", on_pass_turn);
    socket.on("next_round", on_next_round);
    socket.on("send_reaction", on_send_reaction);
    socket.on("stop_game_request", on_stop_game_request);
    socket.on("cancel_stop_vote", on_cancel_stop_vote);
    socket.on("continue_game", on_continue_game);
    socket.on("rematch", on_rematch);
    socket.on("invite_to_lobby", on_invite_to_lobby);
    socket.on("accept_game_invite", on_accept_game_invite);
    socket.on("reject_game_invite", on_reject_game_invite);

    socket.on_disconnect(on_disconnect);
}

async fn on_disconnect(socket: SocketRef) {
    let app = app();
    let (removed_uid, user_still_online) = {
        let mut reg = app.registry.write().unwrap_or_else(|e| e.into_inner());
        let uid = reg.by_socket.remove(&socket.id).map(|s| s.user_id);
        let mut still_online = false;
        if let Some(ref u) = uid {
            if let Some(set) = reg.by_user.get_mut(u) {
                set.remove(&socket.id);
                still_online = !set.is_empty();
                if set.is_empty() {
                    reg.by_user.remove(u);
                }
            }
        }
        (uid, still_online)
    };

    // TS `disconnect`: when the user has no sockets left, broadcast offline.
    if let Some(uid) = removed_uid {
        if !user_still_online {
            let friends: Vec<String> = app
                .db
                .query_all(
                    "SELECT CASE WHEN sender_id = ? THEN receiver_id ELSE sender_id END as friend_id
                     FROM friendships WHERE status = 'accepted' AND (sender_id = ? OR receiver_id = ?)",
                    [&uid, &uid, &uid],
                    |r| r.get(0),
                )
                .unwrap_or_default();
            for fid in friends {
                app.emit_to_user(&fid, "friend_offline", &json!({ "userId": uid }));
            }
        }
    }
}

/* ---------------------------- lobby presence ---------------------------- */

/// NOTE: the TS event takes the lobbyId as a **bare string** argument.
async fn on_join_lobby(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let Some(lobby_id) = d.as_str().map(|id| id.to_uppercase()) else {
        let _ = ack.send(&json!({"error":"Invalid table code"}));
        return;
    };
    let Some(uid) = user_of(&socket) else { return };
    if !is_member(&app(), &lobby_id, &uid) {
        let _ = ack.send(&json!({"error":"Join the table before subscribing"}));
        return;
    }
    {
        let shared = app();
        let mut reg = shared.registry.write().unwrap_or_else(|e| e.into_inner());
        if let Some(sess) = reg.by_socket.get_mut(&socket.id) {
            sess.lobbies.clear();
            sess.lobbies.insert(lobby_id.clone());
        }
    }
    let app = app();
    if let Some(user_id) = user_of(&socket) {
        if let Some(state) = ops::load_state(&app, &lobby_id) {
            let view = sanitize_state(&state, &user_id, Some(&lobby_id), &app.deadlines);
            let _ = socket.emit("game_state_update", &view);
        }
    }
    let _ = ack.send(&json!({"success":true}));
}

async fn on_leave_lobby(socket: SocketRef, Data(d): Data<Value>) {
    if let Some(lobby_id) = d.as_str() {
        let shared = app();
        let mut reg = shared.registry.write().unwrap_or_else(|e| e.into_inner());
        if let Some(sess) = reg.by_socket.get_mut(&socket.id) {
            sess.lobbies.remove(lobby_id);
        }
    }
}

/* ------------------------------ game moves ------------------------------ */

/// Shared move flow: load → engine op → optional `checkGameOver` → persist →
/// broadcast → timers → AI (exactly the TS pattern).
async fn apply_player_move(
    socket: &SocketRef,
    lobby_id: &str,
    op: impl FnOnce(
        &domino_core::GameState,
        &str,
    ) -> Result<domino_core::GameState, domino_core::DominoError>,
    check_round_end: bool,
    ack: AckSender,
) {
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let Some(user_id) = user_of(socket) else {
        let _ = ack.send(&json!({ "error": "Not your turn" }));
        return;
    };
    let Some(state) = ops::load_state(&app, lobby_id) else {
        let _ = ack.send(&json!({ "error": "Game not found" }));
        return;
    };

    // The TS engine throws `Game is not in playing state` — server-side JSON
    // with `status: 'stopped'` fails to parse into `GameState`, so we surface
    // the identical message first.
    if status_of(&state) != "playing" {
        let _ = ack.send(&json!({ "error": "Game is not in playing state" }));
        return;
    }
    let Ok(core) = crate::domain::to_core(&state) else {
        let _ = ack.send(&json!({ "error": "Corrupt game state" }));
        return;
    };

    match op(&core, &user_id) {
        Err(e) => {
            let _ = ack.send(&json!({ "error": e.to_string() }));
        }
        Ok(mut next) => {
            if check_round_end && next.status == GameStatus::RoundEnd {
                next = check_game_over(&next);
            }
            let value = from_core(&state, &next);
            if let Err(error) = ops::update_game_state(&app, lobby_id, &value) {
                eprintln!("Could not persist game: {error}");
                let _ = ack.send(&json!({"error":"Could not save game state"}));
                return;
            }
            ops::start_turn_timer(&app, lobby_id, &value);
            app.broadcast_state(lobby_id, &value);
            ops::check_and_run_ai_turn(&app, lobby_id, &value);
            let _ = ack.send(&json!({ "success": true }));
        }
    }
}

/// NOTE: `draw_market` deliberately skips the round-end check, exactly like
/// the TS handler (draws can never round-end).
async fn on_play_tile(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let Some((payload, ack)) = parse_payload::<PlayTilePayload>(d, ack) else {
        return;
    };
    apply_player_move(
        &socket,
        &payload.lobby_id,
        |state, user| play_tile(state, user, payload.tile, payload.end),
        true,
        ack,
    )
    .await;
}

async fn on_draw_market(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let Some((payload, ack)) = parse_payload::<LobbyIdPayload>(d, ack) else {
        return;
    };
    apply_player_move(&socket, &payload.lobby_id, draw_from_market, false, ack).await;
}

async fn on_pass_turn(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let Some((payload, ack)) = parse_payload::<LobbyIdPayload>(d, ack) else {
        return;
    };
    apply_player_move(&socket, &payload.lobby_id, pass_turn, true, ack).await;
}

async fn on_next_round(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let Some((payload, ack)) = parse_payload::<LobbyIdPayload>(d, ack) else {
        return;
    };
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let Some(user_id) = user_of(&socket) else {
        let _ = ack.send(&json!({ "error": "Not your turn" }));
        return;
    };
    if !is_member(&app, &payload.lobby_id, &user_id) {
        let _ = ack.send(&json!({"error":"Not a lobby member"}));
        return;
    }
    let Some(state) = ops::load_state(&app, &payload.lobby_id) else {
        let _ = ack.send(&json!({ "error": "Game not found" }));
        return;
    };
    if status_of(&state) != "round_end" {
        let _ = ack.send(&json!({ "error": "Round not ended" }));
        return;
    }
    let _ = match ops::do_next_round(&app, &payload.lobby_id, &state) {
        Ok(_) => ack.send(&json!({ "success": true })),
        Err(e) => ack.send(&json!({ "error": e })),
    };
}

/* ------------------------------- reactions ------------------------------- */

async fn on_send_reaction(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let Some((payload, ack)) = parse_payload::<ReactionPayload>(d, ack) else {
        return;
    };
    if !ALLOWED_REACTIONS.contains(&payload.emoji.as_str()) {
        let _ = ack.send(&json!({"error":"Choose a supported reaction"}));
        return;
    }
    if let Some(user_id) = user_of(&socket) {
        if !is_member(&app(), &payload.lobby_id, &user_id) {
            let _ = ack.send(&json!({"error":"Not a table member"}));
            return;
        }
        if app().db.record_reaction(&user_id, &payload.emoji).is_err() {
            let _ = ack.send(&json!({"error":"Could not save your reaction. Try again."}));
            return;
        }
        app().emit_to_room(
            &payload.lobby_id,
            "reaction",
            &json!({"userId":user_id,"emoji":payload.emoji}),
        );
        if let Ok(counts) = app().db.reaction_usage(&user_id) {
            app().emit_to_user(
                &user_id,
                "emote_usage",
                &json!({"userId":user_id,"counts":counts}),
            );
        }
        let _ = ack.send(&json!({"success":true}));
    }
}

/* ------------------------------ stop voting ------------------------------ */

async fn on_stop_game_request(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let Some((payload, ack)) = parse_payload::<LobbyIdPayload>(d, ack) else {
        return;
    };
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let Some(user_id) = user_of(&socket) else {
        let _ = ack.send(&json!({ "error": "Game is not in progress" }));
        return;
    };
    let Some(state) = ops::load_state(&app, &payload.lobby_id) else {
        let _ = ack.send(&json!({ "error": "Game not found" }));
        return;
    };
    let status = status_of(&state).to_string();
    if status != "playing" && status != "round_end" {
        let _ = ack.send(&json!({ "error": "Game is not in progress" }));
        return;
    }

    if !is_member(&app, &payload.lobby_id, &user_id) {
        let _ = ack.send(&json!({"error":"Not a lobby member"}));
        return;
    }

    // Only human players can vote
    if user_id.starts_with("ai_") {
        let _ = ack.send(&json!({ "error": "AI cannot vote" }));
        return;
    }

    // Count required votes: all human players
    let required = ops::count_humans(&state);

    // Register the vote; snapshot voters BEFORE any possible stop bookkeeping
    // (TS builds `voters` from the still-alive set after `delete`, so the
    // emitted array always includes the current voter).
    let (snapshot, current_votes) = {
        let mut votes_map = app.stop_votes.write().unwrap_or_else(|e| e.into_inner());
        let set = votes_map.entry(payload.lobby_id.clone()).or_default();
        set.insert(user_id.clone());
        (set.iter().cloned().collect::<Vec<_>>(), set.len())
    };

    if current_votes >= required {
        // All humans confirmed — stop the game
        let mut stopped = state.clone();
        stopped["previous_status"] = stopped.get("status").cloned().unwrap_or(Value::Null);
        stopped["status"] = json!("stopped");
        if let Err(error) = ops::update_game_state(&app, &payload.lobby_id, &stopped) {
            eprintln!("Could not persist game: {error}");
            let _ = ack.send(&json!({"error":"Could not save game state"}));
            return;
        }
        app.broadcast_state(&payload.lobby_id, &stopped);
        ops::clear_turn_timer(&app, &payload.lobby_id);
        app.stop_votes
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&payload.lobby_id);

        app.emit_to_room(
            &payload.lobby_id,
            "stop_vote_update",
            &json!({ "votes": current_votes, "required": required, "stopped": true, "voters": snapshot }),
        );
    } else {
        // Notify all players of progress
        app.emit_to_room(
            &payload.lobby_id,
            "stop_vote_update",
            &json!({ "votes": current_votes, "required": required, "stopped": false, "voters": snapshot }),
        );
    }

    let _ = ack.send(&json!({ "success": true }));
}

async fn on_cancel_stop_vote(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let Ok(payload) = serde_json::from_value::<LobbyIdPayload>(d) else {
        let _ = ack.send(&json!({ "success": true }));
        return;
    };
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let user_id = user_of(&socket);

    if let Some(user_id) = user_id {
        if !is_member(&app, &payload.lobby_id, &user_id) {
            let _ = ack.send(&json!({"error":"Not a lobby member"}));
            return;
        }
        let snapshot: Option<Vec<String>> = {
            let mut votes_map = app.stop_votes.write().unwrap_or_else(|e| e.into_inner());
            if let Some(set) = votes_map.get_mut(&payload.lobby_id) {
                set.remove(&user_id);
                let snap = set.iter().cloned().collect::<Vec<_>>();
                if set.is_empty() {
                    votes_map.remove(&payload.lobby_id);
                }
                Some(snap)
            } else {
                None
            }
        };

        if let Some(votes) = snapshot {
            let state = ops::load_state(&app, &payload.lobby_id);
            if let Some(state) = state {
                let human_players = ops::count_humans(&state);
                app.emit_to_room(
                    &payload.lobby_id,
                    "stop_vote_update",
                    &json!({ "votes": votes.len(), "required": human_players, "stopped": false, "voters": votes }),
                );
            }
        }
    }

    let _ = ack.send(&json!({ "success": true }));
}

/* --------------------------- continue / rematch --------------------------- */

async fn on_continue_game(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let Some((payload, ack)) = parse_payload::<LobbyIdPayload>(d, ack) else {
        return;
    };
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let Some(user_id) = user_of(&socket) else {
        let _ = ack.send(&json!({ "error": "Lobby not found" }));
        return;
    };

    let creator: Option<String> = app
        .db
        .query_one(
            "SELECT creator_id FROM lobbies WHERE id = ?",
            [&payload.lobby_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let Some(creator) = creator else {
        let _ = ack.send(&json!({ "error": "Lobby not found" }));
        return;
    };
    if creator != user_id {
        let _ = ack.send(&json!({ "error": "Only the host can continue the game" }));
        return;
    }

    if let Some(state) = ops::load_state(&app, &payload.lobby_id) {
        if status_of(&state) != "stopped" {
            let _ = ack.send(&json!({ "error": "Game is not stopped" }));
            return;
        }

        let mut next = state.clone();
        next["status"] = match next["previous_status"].as_str() {
            Some(s @ ("playing" | "round_end" | "finished")) => json!(s),
            _ => json!("playing"),
        };

        if let Err(error) = ops::update_game_state(&app, &payload.lobby_id, &next) {
            eprintln!("Could not persist game: {error}");
            let _ = ack.send(&json!({"error":"Could not save game state"}));
            return;
        }
        app.db
            .execute(
                "UPDATE lobbies SET status = ? WHERE id = ?",
                rusqlite::params!["playing", payload.lobby_id],
            )
            .ok();

        if status_of(&next) == "playing" {
            ops::start_turn_timer(&app, &payload.lobby_id, &next);
            ops::check_and_run_ai_turn(&app, &payload.lobby_id, &next);
        }
        app.broadcast_state(&payload.lobby_id, &next);
    }

    let _ = ack.send(&json!({ "success": true }));
}

async fn on_rematch(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let Some((payload, ack)) = parse_payload::<LobbyIdPayload>(d, ack) else {
        return;
    };
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let Some(user_id) = user_of(&socket) else {
        let _ = ack.send(&json!({ "error": "Lobby not found" }));
        return;
    };

    let creator: Option<String> = app
        .db
        .query_one(
            "SELECT creator_id FROM lobbies WHERE id = ?",
            [&payload.lobby_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let Some(creator) = creator else {
        let _ = ack.send(&json!({ "error": "Lobby not found" }));
        return;
    };
    if creator != user_id {
        let _ = ack.send(&json!({ "error": "Only the host can start a rematch" }));
        return;
    }

    // Active matches must be stopped or completed before rematching.
    if let Some(old_state) = ops::load_state(&app, &payload.lobby_id) {
        let status = status_of(&old_state);
        if status != "finished" && status != "stopped" {
            let _ = ack.send(&json!({ "error": "Game is not finished yet" }));
            return;
        }
    }

    let player_ids: Vec<String> = app
        .db
        .query_all(
            "SELECT user_id FROM lobby_players WHERE lobby_id = ? ORDER BY joined_at ASC",
            [&payload.lobby_id],
            |r| r.get(0),
        )
        .unwrap_or_default();

    if !(2..=4).contains(&player_ids.len()) {
        let _ = ack.send(&json!({"error":"A game needs 2 to 4 players"}));
        return;
    }
    if let Some(previous) = ops::load_state(&app, &payload.lobby_id) {
        if let Err(error) = ops::archive_match(&app, &payload.lobby_id, &previous) {
            eprintln!("Failed to archive match: {error}");
            let _ = ack.send(&json!({"error":"Could not preserve match history"}));
            return;
        }
    }
    app.stop_votes
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&payload.lobby_id);
    let state = match ops::create_table_game(
        &app,
        &payload.lobby_id,
        &player_ids.iter().map(String::as_str).collect::<Vec<_>>(),
    ) {
        Ok(state) => state,
        Err(_) => {
            let _ = ack.send(&json!({"error":"Could not load table rules"}));
            return;
        }
    };
    let mut value: Value = serde_json::to_value(&state).unwrap_or_default();
    value["matchId"] = json!(uuid::Uuid::new_v4().to_string());

    let saved = (|| -> rusqlite::Result<()> {
        let mut conn = app.db.conn();
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO game_states(state_json,lobby_id) VALUES (?,?) ON CONFLICT(lobby_id) DO UPDATE SET state_json=excluded.state_json,updated_at=CURRENT_TIMESTAMP",rusqlite::params![value.to_string(),payload.lobby_id])?;
        tx.execute(
            "UPDATE lobbies SET status='playing' WHERE id=?",
            [&payload.lobby_id],
        )?;
        tx.commit()
    })();
    if let Err(error) = saved {
        eprintln!("Could not save rematch: {error}");
        let _ = ack.send(&json!({"error":"Could not save rematch"}));
        return;
    }

    app.emit_to_room(&payload.lobby_id, "game_started", &json!({}));
    ops::start_turn_timer(&app, &payload.lobby_id, &value);
    app.broadcast_state(&payload.lobby_id, &value);
    ops::check_and_run_ai_turn(&app, &payload.lobby_id, &value);

    let _ = ack.send(&json!({ "success": true }));
}

/* ------------------------------- invitations ------------------------------ */

async fn on_invite_to_lobby(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let Some((payload, ack)) = parse_payload::<InvitePayload>(d, ack) else {
        return;
    };
    let app = app();
    let Some(user_id) = user_of(&socket) else {
        let _ = ack.send(&json!({ "error": "Not friends" }));
        return;
    };

    if !is_member(&app, &payload.lobby_id, &user_id) {
        let _ = ack.send(&json!({"error":"Not a lobby member"}));
        return;
    }

    // Verify they're friends
    let friendship = app
        .db
        .query_one(
            "SELECT * FROM friendships WHERE status = 'accepted' AND ((sender_id = ? AND receiver_id = ?) OR (sender_id = ? AND receiver_id = ?))",
            rusqlite::params![user_id, payload.friend_user_id, payload.friend_user_id, user_id],
            |_r| Ok(()),
        )
        .unwrap_or_default();
    if friendship.is_none() {
        let _ = ack.send(&json!({ "error": "Not friends" }));
        return;
    }

    // Verify lobby exists
    let lobby_exists: Option<String> = app
        .db
        .query_one(
            "SELECT id FROM lobbies WHERE id = ?",
            [&payload.lobby_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if lobby_exists.is_none() {
        let _ = ack.send(&json!({ "error": "Lobby not found" }));
        return;
    }

    // Check for existing pending invite
    let existing = app
        .db
        .query_one(
            "SELECT * FROM game_invitations WHERE lobby_id = ? AND receiver_id = ? AND status = 'pending'",
            rusqlite::params![payload.lobby_id, payload.friend_user_id],
            |_r| Ok(()),
        )
        .unwrap_or_default();
    if existing.is_some() {
        let _ = ack.send(&json!({ "error": "Already invited" }));
        return;
    }

    let sender_username: String = app
        .db
        .query_one("SELECT username FROM users WHERE id = ?", [&user_id], |r| {
            r.get(0)
        })
        .unwrap_or_default()
        .unwrap_or_default();

    let invite_id = app
        .db
        .execute_with_rowid(
            "INSERT INTO game_invitations (lobby_id, sender_id, receiver_id) VALUES (?, ?, ?)",
            rusqlite::params![payload.lobby_id, user_id, payload.friend_user_id],
        )
        .unwrap_or(-1);

    app.emit_to_user(
        &payload.friend_user_id,
        "game_invitation",
        &json!({
            "id": invite_id,
            "lobbyId": payload.lobby_id,
            "senderUsername": sender_username
        }),
    );

    let _ = ack.send(&json!({ "success": true }));
}

async fn on_accept_game_invite(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let Some((payload, ack)) = parse_payload::<InviteIdPayload>(d, ack) else {
        return;
    };
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let app = app();
    let Some(user_id) = user_of(&socket) else {
        let _ = ack.send(&json!({ "error": "Invitation not found" }));
        return;
    };

    let invite: Option<(String, String)> = app
        .db
        .query_one(
            "SELECT lobby_id, sender_id FROM game_invitations WHERE id = ? AND receiver_id = ? AND status = 'pending'",
            rusqlite::params![payload.invite_id, user_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or_default();
    let Some((lobby_id_raw, sender_id)) = invite else {
        let _ = ack.send(&json!({ "error": "Invitation not found" }));
        return;
    };

    // Auto-join the lobby
    let lobby: Option<(i64, String)> = app
        .db
        .query_one(
            "SELECT max_players, creator_id FROM lobbies WHERE id = ?",
            [&lobby_id_raw],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or_default();
    let Some((max_players, creator_id)) = lobby else {
        let _ = ack.send(&json!({ "error": "Lobby no longer exists" }));
        return;
    };

    let lobby_status: Option<String> = app
        .db
        .query_one(
            "SELECT status FROM lobbies WHERE id = ?",
            [&lobby_id_raw],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if lobby_status.as_deref() != Some("waiting") {
        let _ = ack.send(&json!({"error":"Game already started"}));
        return;
    }
    let players_count: i64 = app
        .db
        .query_one(
            "SELECT COUNT(*) FROM lobby_players WHERE lobby_id = ?",
            [&lobby_id_raw],
            |r| r.get(0),
        )
        .unwrap_or_default()
        .unwrap_or(0);
    let is_member: Option<i64> = app
        .db
        .query_one(
            "SELECT 1 FROM lobby_players WHERE lobby_id = ? AND user_id = ?",
            rusqlite::params![lobby_id_raw, user_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if players_count >= max_players && is_member.is_none() {
        let _ = ack.send(&json!({ "error": "Lobby is full" }));
        return;
    }

    app.db
        .execute(
            "UPDATE game_invitations SET status = 'accepted' WHERE id = ?",
            [payload.invite_id],
        )
        .ok();

    app.db
        .execute(
            "INSERT OR IGNORE INTO lobby_players (lobby_id, user_id) VALUES (?, ?)",
            rusqlite::params![lobby_id_raw, user_id],
        )
        .ok();

    let updated_players: Vec<Value> = app
        .db
        .query_all(
            "SELECT u.username, u.id FROM lobby_players lp JOIN users u ON lp.user_id = u.id WHERE lp.lobby_id = ? ORDER BY lp.joined_at ASC",
            [&lobby_id_raw],
            |r| Ok(json!({ "username": r.get::<_, String>(0)?, "id": r.get::<_, String>(1)? })),
        )
        .unwrap_or_default();

    app.emit_to_room(
        &lobby_id_raw,
        "lobby_updated",
        &json!({ "players": updated_players.clone() }),
    );

    // Notify the sender
    app.emit_to_user(
        &sender_id,
        "game_invite_accepted",
        &json!({ "userId": user_id, "lobbyId": lobby_id_raw }),
    );

    let _ = ack.send(&json!({
        "success": true,
        "lobbyId": lobby_id_raw,
        "players": updated_players,
        "maxPlayers": max_players,
        "creatorId": creator_id
    }));
}

async fn on_reject_game_invite(socket: SocketRef, Data(d): Data<Value>, ack: AckSender) {
    let Some((payload, ack)) = parse_payload::<InviteIdPayload>(d, ack) else {
        return;
    };
    let app = app();
    let Some(user_id) = user_of(&socket) else {
        let _ = ack.send(&json!({ "error": "Invitation not found" }));
        return;
    };

    let exists: Option<i64> = app
        .db
        .query_one(
            "SELECT id FROM game_invitations WHERE id = ? AND receiver_id = ? AND status = 'pending'",
            rusqlite::params![payload.invite_id, user_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if exists.is_none() {
        let _ = ack.send(&json!({ "error": "Invitation not found" }));
        return;
    }

    app.db
        .execute(
            "UPDATE game_invitations SET status = 'rejected' WHERE id = ?",
            [payload.invite_id],
        )
        .ok();
    let _ = ack.send(&json!({ "success": true }));
}
