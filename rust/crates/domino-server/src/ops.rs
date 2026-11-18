//! Authoritative match transitions, persistence, turn clocks, and bot scheduling.

use crate::domain::{from_core, now_ms, status_of, to_core, turn_player, AppState};
use domino_core::{
    check_game_over, draw_count_for, draw_from_market, get_ai_move, next_round, pass_turn,
    play_tile, DominoError, GameState, GameStatus,
};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// TS `TURN_TIMER_MS` — 30 s per human turn.
pub const TURN_TIMER_MS: u64 = 30_000;
/// TS AI `setTimeout(..., 1500)` delay.
pub const AI_DELAY_MS: u64 = 1_500;

/// Creation-only table settings are read from storage, never from move payloads.
pub fn create_table_game(
    app: &AppState,
    lobby_id: &str,
    players: &[&str],
) -> Result<GameState, String> {
    let rule: String = app
        .db
        .query_one(
            "SELECT draw_rule FROM lobbies WHERE id = ?",
            [lobby_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?
        .ok_or("Table not found")?;
    let mode: String = app
        .db
        .query_one(
            "SELECT game_mode FROM lobbies WHERE id = ?",
            [lobby_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?
        .ok_or("Table not found")?;
    let mode = serde_json::from_value(Value::String(mode)).map_err(|e| e.to_string())?;
    let mut state = domino_core::create_game(players, mode);
    state.draw_rule = serde_json::from_value(Value::String(rule)).map_err(|e| e.to_string())?;
    Ok(state)
}

/// Load a lobby's stored state (`SELECT state_json …`) as [`serde_json::Value`].
pub fn load_state(app: &AppState, lobby_id: &str) -> Option<Value> {
    let raw: Option<String> = app
        .db
        .query_one(
            "SELECT state_json FROM game_states WHERE lobby_id = ?",
            [lobby_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    raw.and_then(|raw| serde_json::from_str(&raw).ok())
}

/// Persist + duelist status sync + stats, mirroring `updateGameState`.
pub fn update_game_state(app: &AppState, lobby_id: &str, state: &Value) -> Result<(), String> {
    let mut conn = app.db.conn();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let raw: String = tx
        .query_row(
            "SELECT state_json FROM game_states WHERE lobby_id = ?",
            [lobby_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let previous: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let mut stored = state.clone();
    let match_id = previous["matchId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| state["matchId"].as_str().filter(|s| !s.is_empty()))
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    stored["matchId"] = serde_json::json!(match_id);
    let changed = tx.execute("UPDATE game_states SET state_json = ?, updated_at = CURRENT_TIMESTAMP WHERE lobby_id = ?", rusqlite::params![stored.to_string(), lobby_id]).map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err("Game state no longer exists".into());
    }
    tx.execute(
        "UPDATE lobbies SET status = ? WHERE id = ?",
        rusqlite::params![status_of(state), lobby_id],
    )
    .map_err(|e| e.to_string())?;
    if status_of(state) == "finished" && status_of(&previous) != "finished" {
        let inserted = tx.execute("INSERT OR IGNORE INTO leaderboard_match_completions(match_id,completed_at_ms) VALUES (?,?)", rusqlite::params![match_id, now_ms()]).map_err(|e| e.to_string())?;
        if inserted == 1 {
            record_match_stats(&tx, &match_id, state).map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    drop(conn);
    if matches!(status_of(state), "finished" | "stopped") {
        clear_turn_timer(app, lobby_id);
    }
    Ok(())
}

fn record_match_stats(
    tx: &rusqlite::Transaction<'_>,
    match_id: &str,
    state: &Value,
) -> rusqlite::Result<()> {
    let winner = state["lastWinner"].as_str().unwrap_or("");
    for player in state["players"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if player.starts_with("ai_") {
            continue;
        }
        let win = i64::from(player == winner);
        let loss = 1 - win;
        let score = state["scores"][player]["permanent"].as_i64().unwrap_or(0);
        tx.execute("INSERT INTO dated_player_stats(match_id,user_id,wins,losses,total_score) VALUES (?,?,?,?,?)", rusqlite::params![match_id,player,win,loss,score])?;
        tx.execute("INSERT INTO player_stats(user_id,wins,losses,total_score,games_played) VALUES (?,?,?,?,1)
            ON CONFLICT(user_id) DO UPDATE SET wins=wins+excluded.wins, losses=losses+excluded.losses,
            total_score=total_score+excluded.total_score, games_played=games_played+1", rusqlite::params![player,win,loss,score])?;
    }
    Ok(())
}

/* ------------------------------ turn timers ------------------------------ */

/// `clearTurnTimer` — abort the pending task and drop the deadline.
pub fn clear_turn_timer(app: &AppState, lobby_id: &str) {
    if let Some(handle) = app
        .turn_timers
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(lobby_id)
    {
        handle.abort();
    }
    app.deadlines
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .remove(lobby_id);
}

/// `startTurnTimer` — spawn a 30 s watchdog for the current human player only.
pub fn start_turn_timer(app: &Arc<AppState>, lobby_id: &str, state: &Value) {
    start_turn_timer_for(app, lobby_id, state, TURN_TIMER_MS);
}
fn start_turn_timer_for(app: &Arc<AppState>, lobby_id: &str, state: &Value, duration: u64) {
    clear_turn_timer(app, lobby_id);
    if !app.games.domino.enabled() {
        return;
    }

    // Don't start timers for non-playing states
    if status_of(state) != "playing" {
        return;
    }
    let Some(current_player) = turn_player(state) else {
        return;
    };
    // Don't start timer for AI — AI has its own 1.5 s delay
    if current_player.starts_with("ai_") {
        return;
    }
    // Don't start timer if only 1 human player (solo vs AI) — no need to rush
    let humans = count_humans(state);
    if humans <= 1 {
        return;
    }
    // The host can turn the watchdog off when creating the table.
    if !turn_timer_enabled(app, lobby_id) {
        return;
    }

    let deadline = now_ms() + duration as i64;
    app.deadlines
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .insert(lobby_id.to_string(), deadline);

    let handle = tokio::spawn(turn_timer_tick(
        app.clone(),
        lobby_id.to_string(),
        current_player,
        deadline,
        duration,
    ));
    app.turn_timers
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(lobby_id.to_string(), handle);
}

async fn turn_timer_tick(
    app: Arc<AppState>,
    lobby_id: String,
    expected_player: String,
    expected_deadline: i64,
    duration: u64,
) {
    tokio::time::sleep(Duration::from_millis(duration)).await;

    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    if !app.games.domino.enabled() {
        return;
    }

    if app
        .deadlines
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(&lobby_id)
        .copied()
        != Some(expected_deadline)
    {
        return;
    }

    // TS try-block body: reload, verify the turn still belongs to this player.
    let Some(state) = load_state(&app, &lobby_id) else {
        return;
    };
    let still_their_turn = status_of(&state) == "playing"
        && turn_player(&state).as_deref() == Some(expected_player.as_str());
    if !still_their_turn {
        return;
    }

    // A timeout must take a legal action even when the player could play.
    // Drawing or passing illegally would otherwise leave the table stalled.
    let Some(core) = to_core(&state).ok() else {
        return;
    };
    let Some(next) = ai_fallback(&core, &expected_player) else {
        return;
    };
    let next = if next.status == GameStatus::RoundEnd {
        check_game_over(&next)
    } else {
        next
    };
    // Remove this task before starting its successor; never abort ourselves.
    app.turn_timers
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&lobby_id);
    let value = from_core(&state, &next);

    if let Err(e) = update_game_state(&app, &lobby_id, &value) {
        eprintln!("Could not persist transition: {e}");
        return;
    }
    start_turn_timer(&app, &lobby_id, &value);
    app.broadcast_state(&lobby_id, &value);
    check_and_run_ai_turn(&app, &lobby_id, &value);
}

/// Creation-time table setting; tables without a stored row keep the timer.
pub fn turn_timer_enabled(app: &AppState, lobby_id: &str) -> bool {
    app.db
        .query_one(
            "SELECT turn_timer FROM lobbies WHERE id = ?",
            [lobby_id],
            |r| r.get::<_, bool>(0),
        )
        .ok()
        .flatten()
        .unwrap_or(true)
}

/// Number of human players in a stored state (TS `filter(!p.startsWith('ai_'))`).
pub fn count_humans(state: &Value) -> usize {
    state
        .get("players")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter(|p| !p.starts_with("ai_"))
                .count()
        })
        .unwrap_or(0)
}

/* --------------------------------- AI turns -------------------------------- */

/// `checkAndRunAITurn` — if the current player is an AI spawn the delayed
/// turn; the `ai_spawns` dedupe keeps concurrent triggers from double-acting.
pub fn check_and_run_ai_turn(app: &Arc<AppState>, lobby_id: &str, state: &Value) {
    if !app.games.domino.enabled() {
        return;
    }
    if status_of(state) != "playing" {
        return;
    }
    let Some(current_player) = turn_player(state) else {
        return;
    };
    if !current_player.starts_with("ai_") {
        return;
    }

    let generation = app.games.domino.generation();
    let key = (lobby_id.to_string(), current_player.clone(), generation);
    let inserted = app
        .ai_spawns
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key.clone());
    if !inserted {
        return; // an AI task for this exact turn is already pending
    }

    tokio::spawn(ai_turn(
        app.clone(),
        lobby_id.to_string(),
        current_player,
        generation,
    ));
}

/// Removes the dedupe key on every exit path (success, early return, panic).
struct AiSpawnGuard {
    app: Arc<AppState>,
    key: (String, String, u64),
}

impl Drop for AiSpawnGuard {
    fn drop(&mut self) {
        self.app
            .ai_spawns
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.key);
    }
}

async fn ai_turn(app: Arc<AppState>, lobby_id: String, current_player: String, generation: u64) {
    let guard = AiSpawnGuard {
        app: app.clone(),
        key: (lobby_id.clone(), current_player.clone(), generation),
    };

    tokio::time::sleep(Duration::from_millis(AI_DELAY_MS)).await;

    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    if !app.games.domino.enabled() || app.games.domino.generation() != generation {
        return;
    }

    // Ensure it's still the AI's turn
    let Some(state) = load_state(&app, &lobby_id) else {
        return;
    };
    let still_its_turn = status_of(&state) == "playing"
        && turn_player(&state).as_deref() == Some(current_player.as_str());
    if !still_its_turn {
        return;
    }

    let Some(core) = to_core(&state).ok() else {
        return;
    };

    let action = get_ai_move(&core, &current_player);
    let outcome: Result<GameState, DominoError> = match action {
        domino_core::AIAction::Play { tile, end } => play_tile(&core, &current_player, tile, end),
        domino_core::AIAction::Draw => draw_from_market(&core, &current_player),
        domino_core::AIAction::Pass => pass_turn(&core, &current_player),
    };

    // The TS server has a latent stall bug here: when `playTile` throws (e.g.
    // round-1 double-less hands) the error is swallowed and the AI never acts.
    // The Rust port keeps the game flowing with a deterministic fallback.
    let next = match outcome {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("AI fallback after engine error: {e}");
            ai_fallback(&core, &current_player)
        }
    };

    let Some(next) = next else { return };
    let next = if next.status == GameStatus::RoundEnd {
        check_game_over(&next)
    } else {
        next
    };
    let value = from_core(&state, &next);

    if let Err(e) = update_game_state(&app, &lobby_id, &value) {
        eprintln!("Could not persist transition: {e}");
        return;
    }
    start_turn_timer(&app, &lobby_id, &value);
    app.broadcast_state(&lobby_id, &value);
    drop(guard);

    // Recursively check if the next player is also an AI (or if the AI drew
    // and still has its turn — draws never advance `turnIndex`).
    check_and_run_ai_turn(&app, &lobby_id, &value);
}

/// Deterministic safety net:
/// 1. play the first tile that genuinely matches an open end (skipped during a
///    round-1 opening because the engine requires the priority double),
/// 2. otherwise draw when draws remain,
/// 3. otherwise pass — any of which keeps the game flowing.
fn ai_fallback(core: &GameState, current_player: &str) -> Option<GameState> {
    for (tile, end) in domino_core::legal_moves(core, current_player) {
        if let Ok(next) = play_tile(core, current_player, tile, end) {
            return Some(next);
        }
    }

    let draws_left = draw_count_for(core.players.len()) > 0 && !core.market.is_empty();
    if draws_left {
        draw_from_market(core, current_player).ok()
    } else {
        pass_turn(core, current_player).ok()
    }
}

/* ------------------------------ start & advance ------------------------------ */

/// Advance to the next round for the `next_round` socket event.
pub fn do_next_round(app: &Arc<AppState>, lobby_id: &str, state: &Value) -> Result<Value, String> {
    let core = to_core(state)?;
    let next = next_round(&core).map_err(|e| e.to_string())?;
    let next = if next.status == GameStatus::RoundEnd {
        check_game_over(&next)
    } else {
        next
    };
    let value = from_core(state, &next);
    update_game_state(app, lobby_id, &value)?;
    start_turn_timer(app, lobby_id, &value);
    app.broadcast_state(lobby_id, &value);
    check_and_run_ai_turn(app, lobby_id, &value);
    Ok(value)
}

/// Periodic guest cleanup — every 30 min, mirroring `setInterval(cleanupStaleGuests, …)`.
/// Waiting rooms are transient. Bots do not keep an abandoned room alive.
/// The grace interval protects refreshes, mobile suspension and reconnection.
pub fn cleanup_unstarted_tables(
    app: &AppState,
    idle: &mut std::collections::HashMap<String, i64>,
    now: i64,
) -> Result<usize, String> {
    if !app.games.domino.enabled() {
        idle.clear();
        return Ok(0);
    }
    let _transition = crate::domain::MUTATIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let candidates=app.db.query_all("SELECT id FROM lobbies WHERE status = 'waiting' AND NOT EXISTS (SELECT 1 FROM game_states WHERE lobby_id = lobbies.id)",[],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;
    let registry = app.registry.read().unwrap_or_else(|e| e.into_inner());
    let active: std::collections::HashSet<_> = registry
        .by_socket
        .values()
        .flat_map(|s| s.lobbies.iter().cloned())
        .collect();
    let mut removed = 0;
    idle.retain(|id, _| candidates.contains(id));
    for id in candidates {
        if active.contains(&id) {
            idle.remove(&id);
            continue;
        }
        let since = *idle.entry(id.clone()).or_insert(now);
        if now - since >= 90_000 {
            removed+=app.db.execute("DELETE FROM lobbies WHERE id = ? AND status = 'waiting' AND NOT EXISTS (SELECT 1 FROM game_states WHERE lobby_id = lobbies.id)",[&id]).map_err(|e|e.to_string())?;
            idle.remove(&id);
        }
    }
    Ok(removed)
}
pub fn spawn_waiting_room_cleanup(app: Arc<AppState>) {
    tokio::spawn(async move {
        let mut idle = std::collections::HashMap::new();
        loop {
            if let Err(e) = cleanup_unstarted_tables(&app, &mut idle, now_ms()) {
                eprintln!("Waiting room cleanup failed: {e}");
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}

pub fn spawn_guest_cleanup(app: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30 * 60)).await;
            let app = app.clone();
            tokio::task::spawn_blocking(move || {
                let _transition = crate::domain::MUTATIONS
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                app.db.cleanup_stale_guests().ok();
            })
            .await
            .ok();
        }
    });
}

/// Re-arm unfinished matches after a process restart. Stored hands remain authoritative.
pub fn recover_matches(app: &Arc<AppState>) {
    if !app.games.domino.enabled() {
        return;
    }
    let rows = app
        .db
        .query_all("SELECT lobby_id, state_json FROM game_states", [], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .unwrap_or_default();
    for (lobby, raw) in rows {
        if let Ok(mut state) = serde_json::from_str::<Value>(&raw) {
            let remaining = state["hostRemainingMs"].as_u64().unwrap_or(TURN_TIMER_MS);
            if state.get("hostRemainingMs").is_some() {
                state.as_object_mut().unwrap().remove("hostRemainingMs");
                if update_game_state(app, &lobby, &state).is_err() {
                    continue;
                }
            }
            start_turn_timer_for(app, &lobby, &state, remaining);
            check_and_run_ai_turn(app, &lobby, &state);
        }
    }
}

pub fn suspend_host_matches(app: &Arc<AppState>) -> Result<(), String> {
    let rows = app
        .db
        .query_all("SELECT lobby_id,state_json FROM game_states", [], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    for (id, raw) in rows {
        if let Ok(mut state) = serde_json::from_str::<Value>(&raw) {
            if status_of(&state) == "playing" {
                let remaining = app
                    .deadlines
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(&id)
                    .map(|d| d.saturating_sub(now_ms()).max(0) as u64)
                    .unwrap_or_else(|| state["hostRemainingMs"].as_u64().unwrap_or(TURN_TIMER_MS));
                state["hostRemainingMs"] = serde_json::json!(remaining);
                update_game_state(app, &id, &state)?;
            }
        }
        clear_turn_timer(app, &id);
    }
    Ok(())
}

/// Preserve a completed/abandoned match before its lobby starts a rematch.
/// Only public summary data is archived; hands and market are never included.
pub fn archive_match(app: &AppState, lobby_id: &str, state: &Value) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string().to_uppercase();
    let players = app.db.query_all("SELECT u.id, u.username FROM lobby_players lp JOIN users u ON u.id = lp.user_id WHERE lp.lobby_id = ? ORDER BY lp.joined_at", [lobby_id], |r| Ok(serde_json::json!({"id":r.get::<_,String>(0)?, "username":r.get::<_,String>(1)?}))).map_err(|e|e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let summary = serde_json::json!({"id":id, "status":if status_of(state)=="finished" {"finished"} else {"archived"}, "created_at":now,"updated_at":now,"player_count":players.len(),"players":players,"mode":state["mode"],"scores":state["scores"],"winner":if status_of(state)=="finished" {state["lastWinner"].clone()} else {Value::Null}});
    let mut conn = app.db.conn();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO completed_matches (id, lobby_id, summary_json) VALUES (?, ?, ?)",
        rusqlite::params![id, lobby_id, summary.to_string()],
    )
    .map_err(|e| e.to_string())?;
    for player in players {
        let uid = player["id"].as_str().unwrap_or_default();
        if !uid.starts_with("ai_") {
            tx.execute(
                "INSERT INTO completed_match_players (match_id,user_id) VALUES (?,?)",
                rusqlite::params![id, uid],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::Db, domain::Registry};
    use std::{
        collections::{HashMap, HashSet},
        sync::{Mutex, RwLock},
    };

    fn setup(players: &[&str]) -> (Arc<AppState>, GameState) {
        let app = Arc::new(AppState {
            host_id: uuid::Uuid::new_v4().to_string(),
            db: Db::open_in_memory().unwrap(),
            games: crate::lifecycle::Games::default(),
            turn_timers: Mutex::new(HashMap::new()),
            deadlines: RwLock::new(HashMap::new()),
            stop_votes: RwLock::new(HashMap::new()),
            registry: RwLock::new(Registry::default()),
            ai_spawns: Mutex::new(HashSet::new()),
        });
        for p in players {
            app.db
                .execute(
                    "INSERT OR IGNORE INTO users (id, username) VALUES (?, ?)",
                    [p, p],
                )
                .unwrap();
        }
        app.db
            .execute(
                "INSERT INTO lobbies (id, creator_id, max_players) VALUES ('test', ?, ?)",
                rusqlite::params![players[0], players.len()],
            )
            .unwrap();
        let mut core = domino_core::create_game(players, domino_core::GameMode::OneOhOne);
        core.round_number = 2;
        core.turn_index = 0;
        core.board = vec![[6, 6]];
        core.open_ends = [Some(6), Some(6)];
        core.market = vec![];
        (app, core)
    }
    #[test]
    fn abandoned_waiting_rooms_expire_but_started_matches_survive() {
        let (app, core) = setup(&["A", "B"]);
        let mut idle = std::collections::HashMap::new();
        assert_eq!(cleanup_unstarted_tables(&app, &mut idle, 1_000).unwrap(), 0);
        assert_eq!(
            cleanup_unstarted_tables(&app, &mut idle, 90_999).unwrap(),
            0
        );
        save(&app, &core);
        assert_eq!(
            cleanup_unstarted_tables(&app, &mut idle, 91_000).unwrap(),
            0
        );
        assert!(idle.is_empty());
        app.db
            .execute("DELETE FROM game_states WHERE lobby_id = 'test'", [])
            .unwrap();
        assert_eq!(
            cleanup_unstarted_tables(&app, &mut idle, 92_000).unwrap(),
            0
        );
        assert_eq!(
            cleanup_unstarted_tables(&app, &mut idle, 182_000).unwrap(),
            1
        );
        let count: i64 = app
            .db
            .query_one(
                "SELECT COUNT(*) FROM lobby_players WHERE lobby_id = 'test'",
                [],
                |r| r.get(0),
            )
            .unwrap()
            .unwrap();
        assert_eq!(count, 0);
    }
    #[test]
    fn new_games_and_rematches_read_persisted_table_rules() {
        let (app, _) = setup(&["A", "B"]);
        assert_eq!(
            create_table_game(&app, "test", &["A", "B"])
                .unwrap()
                .draw_rule,
            domino_core::DrawRule::Anytime
        );
        app.db
            .execute(
                "UPDATE lobbies SET draw_rule = 'when_blocked' WHERE id = 'test'",
                [],
            )
            .unwrap();
        for _ in 0..2 {
            assert_eq!(
                create_table_game(&app, "test", &["A", "B"])
                    .unwrap()
                    .draw_rule,
                domino_core::DrawRule::WhenBlocked
            );
        }
    }
    fn save(app: &AppState, core: &GameState) {
        app.db
            .execute(
                "INSERT OR REPLACE INTO game_states (lobby_id, state_json) VALUES ('test', ?)",
                [serde_json::to_string(core).unwrap()],
            )
            .unwrap();
    }
    async fn tick() {
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(AI_DELAY_MS)).await;
        tokio::task::yield_now().await;
    }

    #[tokio::test(start_paused = true)]
    async fn ai_draws_repeatedly_then_plays_without_stalling() {
        let (app, mut core) = setup(&["ai_1", "human"]);
        core.hands.insert("ai_1".into(), vec![[0, 0]]);
        core.hands.insert("human".into(), vec![[1, 1]]);
        core.market = vec![[0, 2], [2, 3], [0, 6], [4, 5]];
        save(&app, &core);
        check_and_run_ai_turn(&app, "test", &serde_json::json!(core));
        tick().await;
        tick().await;
        tick().await;
        let next = to_core(&load_state(&app, "test").unwrap()).unwrap();
        assert_eq!(next.market.len(), 0);
        assert_eq!(next.board.len(), 2);
        assert_eq!(next.turn_index, 1);
        assert!(app.ai_spawns.lock().unwrap().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn human_deadline_starts_after_bot_turn() {
        let (app, mut core) = setup(&["ai_1", "human", "other"]);
        core.hands.insert("ai_1".into(), vec![[6, 0], [3, 3]]);
        core.hands.insert("human".into(), vec![[1, 1]]);
        core.hands.insert("other".into(), vec![[2, 2]]);
        save(&app, &core);
        check_and_run_ai_turn(&app, "test", &serde_json::json!(core));
        tick().await;
        assert!(app.deadlines.read().unwrap().contains_key("test"));
        clear_turn_timer(&app, "test");
    }

    #[tokio::test(start_paused = true)]
    async fn tables_without_turn_timer_never_set_a_deadline() {
        let (app, mut core) = setup(&["human", "other"]);
        core.hands.insert("human".into(), vec![[6, 1]]);
        core.hands.insert("other".into(), vec![[2, 2]]);
        save(&app, &core);
        let state = serde_json::json!(core);
        assert!(turn_timer_enabled(&app, "test"));
        start_turn_timer(&app, "test", &state);
        assert!(app.deadlines.read().unwrap().contains_key("test"));
        clear_turn_timer(&app, "test");

        app.db
            .execute("UPDATE lobbies SET turn_timer = 0 WHERE id = 'test'", [])
            .unwrap();
        assert!(!turn_timer_enabled(&app, "test"));
        start_turn_timer(&app, "test", &state);
        assert!(!app.deadlines.read().unwrap().contains_key("test"));
        assert!(app.turn_timers.lock().unwrap().is_empty());
        tokio::time::advance(Duration::from_millis(TURN_TIMER_MS + 1_000)).await;
        tokio::task::yield_now().await;
        let after = to_core(&load_state(&app, "test").unwrap()).unwrap();
        assert_eq!(after.turn_index, 0, "nobody may move for the waiting player");
        assert_eq!(after.board.len(), 1);
    }

    #[test]
    fn rematch_archive_preserves_public_summary_independent_of_live_state() {
        let (app, mut core) = setup(&["human", "ai_1"]);
        core.status = GameStatus::Finished;
        core.last_winner = Some("human".into());
        core.scores.get_mut("human").unwrap().permanent = 105;
        save(&app, &core);
        let id = archive_match(&app, "test", &serde_json::json!(core)).unwrap();
        core.status = GameStatus::Playing;
        core.scores.get_mut("human").unwrap().permanent = 0;
        save(&app, &core);
        let raw: String = app
            .db
            .query_one(
                "SELECT summary_json FROM completed_matches WHERE id = ?",
                [&id],
                |r| r.get(0),
            )
            .unwrap()
            .unwrap();
        let archived: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(archived["scores"]["human"]["permanent"], 105);
        assert_eq!(archived["winner"], "human");
        assert!(archived.get("hands").is_none());
        app.db
            .execute("DELETE FROM lobbies WHERE id = 'test'", [])
            .unwrap();
        assert!(app
            .db
            .query_one(
                "SELECT 1 FROM completed_matches WHERE id = ?",
                [&id],
                |_| Ok(())
            )
            .unwrap()
            .is_some());
    }

    #[test]
    fn finishing_state_records_stats_once_and_cascades_on_delete() {
        let (app, mut core) = setup(&["human", "other"]);
        save(&app, &core);
        core.status = GameStatus::Finished;
        core.last_winner = Some("human".into());
        let state = serde_json::json!(core);
        update_game_state(&app, "test", &state).unwrap();
        update_game_state(&app, "test", &state).unwrap();
        let games: i64 = app
            .db
            .query_one(
                "SELECT games_played FROM player_stats WHERE user_id = 'human'",
                [],
                |r| r.get(0),
            )
            .unwrap()
            .unwrap();
        assert_eq!(games, 1);
        app.db
            .execute("DELETE FROM lobbies WHERE id = 'test'", [])
            .unwrap();
        assert!(load_state(&app, "test").is_none());
    }
    #[test]
    fn completion_is_atomic_and_does_not_recount_legacy_totals() {
        let (app, mut core) = setup(&["human", "other"]);
        save(&app, &core);
        app.db.execute("INSERT INTO player_stats(user_id,wins,losses,total_score,games_played) VALUES ('human',7,3,900,10)",[]).unwrap();
        core.status = GameStatus::Finished;
        core.last_winner = Some("human".into());
        core.scores.get_mut("human").unwrap().permanent = 105;
        let state = from_core(&serde_json::json!({}), &core);
        app.db.execute("CREATE TRIGGER fail_dated BEFORE INSERT ON dated_player_stats WHEN NEW.user_id='other' BEGIN SELECT RAISE(ABORT,'injected'); END",[]).unwrap();
        assert!(update_game_state(&app, "test", &state).is_err());
        assert_eq!(status_of(&load_state(&app, "test").unwrap()), "playing");
        assert_eq!(
            app.db
                .query_one(
                    "SELECT COUNT(*) FROM leaderboard_match_completions",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            Some(0)
        );
        app.db.execute("DROP TRIGGER fail_dated", []).unwrap();
        update_game_state(&app, "test", &state).unwrap();
        update_game_state(&app, "test", &state).unwrap();
        assert_eq!(
            app.db
                .query_one(
                    "SELECT games_played,total_score FROM player_stats WHERE user_id='human'",
                    [],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
                )
                .unwrap(),
            Some((11, 1005))
        );
        assert_eq!(
            app.db
                .query_one("SELECT COUNT(*) FROM dated_player_stats", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            Some(2)
        );
        assert_eq!(
            load_state(&app, "test").unwrap()["matchId"],
            state["matchId"]
        );
    }

    #[test]
    fn legacy_finished_state_is_not_given_a_fabricated_completion_date() {
        let (app, mut core) = setup(&["human", "other"]);
        core.status = GameStatus::Finished;
        save(&app, &core);
        update_game_state(&app, "test", &serde_json::json!(core)).unwrap();
        assert!(load_state(&app, "test").unwrap()["matchId"]
            .as_str()
            .is_some());
        assert_eq!(
            app.db
                .query_one(
                    "SELECT COUNT(*) FROM leaderboard_match_completions",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            Some(0)
        );
    }
    #[test]
    fn telefon_table_creation_and_restored_topology_keep_their_mode() {
        let (app, _) = setup(&["A", "B"]);
        app.db
            .execute(
                "UPDATE lobbies SET game_mode = 'telefon' WHERE id = 'test'",
                [],
            )
            .unwrap();
        for _ in 0..2 {
            let core = create_table_game(&app, "test", &["A", "B"]).unwrap();
            assert_eq!(core.mode, domino_core::GameMode::Telefon);
            let actor = &core.players[core.turn_index];
            let placed =
                domino_core::play_tile(&core, actor, [2, 3], domino_core::End::Left).unwrap();
            save(&app, &placed);
            let restored = crate::domain::to_core(&load_state(&app, "test").unwrap()).unwrap();
            assert_eq!(restored, placed);
            assert_eq!(restored.scores[actor].permanent, 5);
        }
    }
}
