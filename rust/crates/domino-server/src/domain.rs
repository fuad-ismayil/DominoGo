//! Shared application state, socket delivery, and private game-state projections.

use crate::db::Db;
use domino_core::GameState;
use serde_json::{json, Map, Value};
use socketioxide::extract::SocketRef;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Serialize read/validate/write transitions across Tokio workers.
pub static MUTATIONS: Mutex<()> = Mutex::new(());

pub static APP: OnceLock<std::sync::Arc<AppState>> = OnceLock::new();

pub fn app() -> std::sync::Arc<AppState> {
    APP.get().expect("AppState initialized at startup").clone()
}

pub type SocketId = socketioxide::socket::Sid;

/// A live Socket.IO connection.
pub struct Sess {
    pub user_id: String,
    pub token: String,
    pub handle: SocketRef,
    pub lobbies: HashSet<String>,
}

/// In-process registry replacing Socket.IO rooms (`user_{id}` / `lobby_{id}`)
/// plus the TS `onlineUsers` map. Per-socket emits are possible because every
/// session keeps a clonable handle.
#[derive(Default)]
pub struct Registry {
    pub by_socket: HashMap<SocketId, Sess>,
    pub by_user: HashMap<String, HashSet<SocketId>>,
}

/// Shared server state mirroring the module-level maps of `server.ts`.
pub struct AppState {
    pub db: Db,
    /// lobbyId → spawned turn timer task (TS `turnTimers`). std Mutex because
    /// `updateGameState` clears timers from synchronous contexts.
    pub turn_timers: Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
    /// lobbyId → epoch-ms deadline (TS `turnDeadlines`).
    pub deadlines: RwLock<HashMap<String, i64>>,
    /// lobbyId → set of human player userIds that voted to stop (TS `stopVotes`).
    pub stop_votes: RwLock<HashMap<String, HashSet<String>>>,
    pub registry: RwLock<Registry>,
    /// Pending AI turn keys `(lobbyId, player)` dedupe; dropped automatically
    /// when the scheduled AI task finishes acting or observing no change.
    pub ai_spawns: Mutex<HashSet<(String, String)>>,
}

impl AppState {
    pub fn disconnect_revoked_sessions(&self, user: &str) {
        let sockets: Vec<_> = {
            let registry = self.registry.read().unwrap_or_else(|e| e.into_inner());
            registry
                .by_socket
                .values()
                .filter(|s| s.user_id == user && !valid_session(&self.db, &s.token))
                .map(|s| s.handle.clone())
                .collect()
        };
        for socket in sockets {
            let _ = socket.disconnect();
        }
    }

    /// Emit a JSON payload to every socket of one user (TS `io.to('user_x').emit`).
    pub fn emit_to_user(&self, user_id: &str, event: &str, payload: &Value) {
        emit_on(
            &self.db,
            &self.registry.read().unwrap_or_else(|e| e.into_inner()),
            &[user_id],
            event,
            payload,
        );
    }

    /// Emit to every socket currently inside a lobby room
    /// (TS `io.to('lobby_x').emit`).
    pub fn emit_to_room(&self, lobby_id: &str, event: &str, payload: &Value) {
        // Tag every room event so a tab switching tables can discard frames
        // already in transit from its previous subscription.
        let mut payload = payload.clone();
        if let Some(fields) = payload.as_object_mut() {
            fields.insert("lobbyId".into(), json!(lobby_id));
        }
        let reg = self.registry.read().unwrap_or_else(|e| e.into_inner());
        for sess in reg.by_socket.values() {
            if sess.lobbies.contains(lobby_id) && valid_session(&self.db, &sess.token) {
                let _ = sess.handle.emit(event, &payload);
            }
        }
    }

    /// Per-user sanitized broadcast (TS `broadcastGameState`): every listening
    /// socket receives a payload built for ITS user.
    pub fn broadcast_state(&self, lobby_id: &str, state: &Value) {
        let reg = self.registry.read().unwrap_or_else(|e| e.into_inner());
        for sess in reg.by_socket.values() {
            if sess.lobbies.contains(lobby_id) && valid_session(&self.db, &sess.token) {
                let view = sanitize_state(state, &sess.user_id, Some(lobby_id), &self.deadlines);
                let _ = sess.handle.emit("game_state_update", &view);
            }
        }
    }
}

fn valid_session(db: &Db, token: &str) -> bool {
    db.query_one("SELECT 1 FROM sessions WHERE id = ? AND (expires_at IS NULL OR expires_at > datetime('now'))", [token], |_| Ok(())).ok().flatten().is_some()
}

fn emit_on(db: &Db, reg: &Registry, user_ids: &[&str], event: &str, payload: &Value) {
    for uid in user_ids {
        if let Some(sids) = reg.by_user.get(*uid) {
            for sid in sids {
                if let Some(sess) = reg.by_socket.get(sid) {
                    if valid_session(db, &sess.token) {
                        let _ = sess.handle.emit(event, payload);
                    }
                }
            }
        }
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Hide market contents always and other hands until the round ends; add derived counts
/// (`opponentHandCounts`, `marketCount`, `turnDeadline`), keep every other key.
pub fn sanitize_state(
    state: &Value,
    user_id: &str,
    lobby_id: Option<&str>,
    deadlines: &std::sync::RwLock<HashMap<String, i64>>,
) -> Value {
    let mut out = state.clone();
    if let Some(id) = lobby_id {
        out["lobbyId"] = json!(id);
    }
    let players: Vec<String> = out
        .get("players")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let mut opponent_counts = Map::new();
    for p in &players {
        if p != user_id {
            let count = out
                .get("hands")
                .and_then(|h| h.get(p))
                .and_then(Value::as_array)
                .map_or(0, |a| a.len());
            opponent_counts.insert(
                p.clone(),
                Value::Number(serde_json::Number::from(count as u64)),
            );
        }
    }

    let own_hand = out
        .get("hands")
        .and_then(|h| h.get(user_id))
        .cloned()
        .unwrap_or(json!([]));

    if !matches!(out["status"].as_str(), Some("round_end" | "finished")) {
        out["hands"] = json!({ user_id: own_hand });
    }
    out["opponentHandCounts"] = Value::Object(opponent_counts);
    out["marketCount"] = json!(out
        .get("market")
        .and_then(Value::as_array)
        .map_or(0, |a| a.len()));
    out["market"] = json!([]);

    let deadline = match lobby_id {
        Some(id) => deadlines
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .copied(),
        None => None,
    };
    out["turnDeadline"] = match deadline {
        Some(v) => json!(v),
        None => Value::Null,
    };
    out
}

/* ---------- stored-state helpers (state_json as serde_json::Value) ---------- */

/// Parse the engine's strict `GameState` from a stored blob. Fails on server
/// statuses like `stopped` — callers must branch on status first (as TS does).
pub fn to_core(state: &Value) -> Result<GameState, String> {
    serde_json::from_value(state.clone()).map_err(|e| e.to_string())
}

/// Serialize an engine result back to the stored shape, re-attaching any
/// server-only keys that the TS engine preserves by spread. NOTE: the TS stop
/// system stores the key as `previous_status` (snake_case) — see `stop_game_request`.
pub fn from_core(base: &Value, core_result: &GameState) -> Value {
    let mut out = json!(core_result);
    out["matchId"] = base
        .get("matchId")
        .filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))
        .cloned()
        .unwrap_or_else(|| json!(uuid::Uuid::new_v4().to_string()));
    let old_board = base["board"].as_array();
    let added = old_board
        .filter(|old| core_result.board.len() == old.len() + 1)
        .and_then(|old| {
            core_result
                .board
                .iter()
                .find(|tile| {
                    !old.iter().any(|v| {
                        serde_json::from_value::<[u8; 2]>(v.clone())
                            .ok()
                            .is_some_and(|previous| {
                                previous == **tile || previous == [tile[1], tile[0]]
                            })
                    })
                })
                .copied()
        });
    if core_result.board.is_empty() {
        out["lastPlayedTile"] = Value::Null;
        out["roundFinishingTile"] = Value::Null;
    } else {
        out["lastPlayedTile"] =
            added.map_or_else(|| base["lastPlayedTile"].clone(), |tile| json!(tile));
        out["roundFinishingTile"] = if matches!(
            core_result.status,
            domino_core::GameStatus::RoundEnd | domino_core::GameStatus::Finished
        ) {
            // A blocked round ends on a pass: keep its last actual placement
            // identifiable during review, even though that action added no tile.
            out["lastPlayedTile"].clone()
        } else {
            base["roundFinishingTile"].clone()
        };
    }
    if let Some(ps) = base.get("previous_status") {
        out["previous_status"] = ps.clone();
    }
    out
}

/// Status string accessor for stored blobs.
pub fn status_of(state: &Value) -> &str {
    state.get("status").and_then(Value::as_str).unwrap_or("")
}

pub fn turn_player(state: &Value) -> Option<String> {
    let players = state.get("players")?.as_array()?;
    let idx = state.get("turnIndex")?.as_u64()? as usize;
    players.get(idx).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_completed_rounds_reveal_hands_and_market_stays_private() {
        let mut state = json!({"players":["A","B"],"hands":{"A":[],"B":[[2,3]]},"market":[[6,6]]});
        let deadlines = RwLock::new(HashMap::new());
        for status in ["playing", "stopped", "round_end", "finished"] {
            state["status"] = json!(status);
            for user in ["A", "B"] {
                let view = sanitize_state(&state, user, None, &deadlines);
                assert_eq!(
                    view["hands"].as_object().unwrap().len(),
                    if matches!(status, "round_end" | "finished") {
                        2
                    } else {
                        1
                    }
                );
                assert_eq!(view["market"], json!([]));
            }
        }
    }
    #[test]
    fn last_play_tracks_both_ends_and_clears_on_deal() {
        let mut core = domino_core::create_game(&["A", "B"], domino_core::GameMode::OneOhOne);
        core.board = vec![[1, 2]];
        let base = json!(core);
        core.board.insert(0, [3, 1]);
        let left = from_core(&base, &core);
        assert_eq!(left["lastPlayedTile"], json!([3, 1]));
        let mut blocked = core.clone();
        blocked.status = domino_core::GameStatus::RoundEnd;
        assert_eq!(
            from_core(&left, &blocked)["roundFinishingTile"],
            json!([3, 1])
        );
        core.board.push([2, 4]);
        core.status = domino_core::GameStatus::RoundEnd;
        let finish = from_core(&left, &core);
        assert_eq!(finish["roundFinishingTile"], json!([2, 4]));
        assert_eq!(from_core(&finish, &core)["lastPlayedTile"], json!([2, 4]));
        core.board.clear();
        assert!(from_core(&finish, &core)["lastPlayedTile"].is_null());
    }
    #[test]
    fn sanitize_hides_market_and_hands() {
        let state: Value = serde_json::from_str(
            r#"{"mode":"101","players":["Human","ai_1"],
                "hands":{"Human":[[3,3]],"ai_1":[[0,4],[5,6]]},
                "market":[[6,6],[2,2]],
                "board":[],"openEnds":[1,1],
                "scores":{"Human":{"temporary":0,"permanent":0},"ai_1":{"temporary":0,"permanent":0}},
                "turnIndex":0,"status":"playing","roundNumber":1,
                "lastWinner":null,"consecutivePasses":0,
                "previous_status":"playing"}"#,
        )
        .unwrap();
        let deadlines: std::sync::RwLock<HashMap<String, i64>> =
            std::sync::RwLock::new(HashMap::new());
        let view = sanitize_state(&state, "Human", Some("L1"), &deadlines);
        let own: Vec<Value> = view["hands"]["Human"].as_array().unwrap().clone();
        assert_eq!(own.len(), 1);
        assert!(
            view["hands"].as_object().unwrap().len() == 1,
            "only own hand exposed"
        );
        assert_eq!(
            view["opponentHandCounts"]["ai_1"].as_u64().unwrap(),
            2,
            "opponentHandCounts carries counts not tiles"
        );
        assert_eq!(view["market"].as_array().unwrap().len(), 0);
        assert_eq!(view["marketCount"].as_u64().unwrap(), 2);
        assert!(view["turnDeadline"].is_null());
        assert_eq!(
            view["lobbyId"], "L1",
            "private updates identify their table"
        );
        assert!(sanitize_state(&state, "Human", None, &deadlines)
            .get("lobbyId")
            .is_none());
        assert_eq!(
            view["previous_status"], "playing",
            "server-extras preserved"
        );
    }
}
