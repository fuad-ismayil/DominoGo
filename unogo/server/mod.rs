//! UnoGo endpoints on the shared account authority. All private state stays in SQLite.
#![allow(clippy::result_large_err)]
use crate::{auth::AuthUser, db::Db, domain::AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use uno_core::{bot_action, Action, Game, Phase, Receipt};

type Result<T> = std::result::Result<T, ApiError>;
#[derive(Debug)]
pub(crate) struct ApiError(pub(crate) StatusCode, pub(crate) String, pub(crate) String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"code":self.1,"error":self.2}))).into_response()
    }
}
impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        eprintln!("UnoGo storage: {e}");
        fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STORAGE",
            "Could not save the table",
        )
    }
}
impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        eprintln!("UnoGo state: {e}");
        fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "STATE",
            "Could not read the saved round",
        )
    }
}
impl From<uno_core::Error> for ApiError {
    fn from(e: uno_core::Error) -> Self {
        fail(StatusCode::CONFLICT, "ACTION_REJECTED", &e.to_string())
    }
}
fn fail(status: StatusCode, code: &str, message: &str) -> ApiError {
    ApiError(status, code.into(), message.into())
}
fn denied() -> ApiError {
    fail(
        StatusCode::FORBIDDEN,
        "NOT_ALLOWED",
        "This action is unavailable to this player",
    )
}
fn now() -> u64 {
    crate::domain::now_ms().max(0) as u64
}
fn code(raw: &str) -> Result<String> {
    if raw.len() != 7 || !raw.bytes().all(|c| c.is_ascii_alphanumeric()) {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "TABLE_CODE",
            "Enter a seven-character table code",
        ));
    }
    Ok(raw.to_ascii_uppercase())
}

#[derive(Clone, Serialize, Deserialize)]
struct Saved {
    game: Game,
    #[serde(default)]
    host_paused: bool,
    #[serde(default)]
    pause_votes: Vec<String>,
}
struct Room {
    id: String,
    host: String,
    capacity: usize,
    status: String,
    match_id: Option<String>,
    saved: Option<Saved>,
    saved_at: u64,
    bot_due: u64,
    seats: Vec<Seat>,
    reactions: Vec<Value>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Seat {
    slot: usize,
    user_id: Option<String>,
    name: String,
}

fn seats(conn: &Connection, id: &str) -> Result<Vec<Seat>> {
    Ok(conn.prepare("SELECT seat,user_id,COALESCE(u.username,s.name) FROM uno_seats s LEFT JOIN users u ON u.id=s.user_id WHERE table_id=? ORDER BY seat")?.query_map([id], |r| Ok(Seat {slot:r.get(0)?,user_id:r.get(1)?,name:r.get(2)?}))?.collect::<rusqlite::Result<_>>()?)
}
fn load(conn: &Connection, id: &str) -> Result<Room> {
    let row = conn.query_row("SELECT host_id,capacity,status,match_id,state_json,saved_at,bot_due FROM uno_tables WHERE id=?", [id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,usize>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,u64>(5)?,r.get::<_,u64>(6)?))).optional()?;
    let Some((host, capacity, status, match_id, raw, saved_at, bot_due)) = row else {
        if conn
            .query_row("SELECT 1 FROM lobbies WHERE id=?", [id], |_| Ok(()))
            .optional()?
            .is_some()
        {
            return Err(fail(
                StatusCode::CONFLICT,
                "WRONG_GAME",
                "This code belongs to DominoGo. Open domino.driveatbaku.games to join.",
            ));
        }
        return Err(fail(
            StatusCode::NOT_FOUND,
            "TABLE_NOT_FOUND",
            "Table not found",
        ));
    };
    Ok(Room {
        id: id.into(),
        host,
        capacity,
        status,
        match_id,
        saved: raw.map(|s| serde_json::from_str(&s)).transpose()?,
        saved_at,
        bot_due,
        seats: seats(conn, id)?,
        reactions: conn.prepare("SELECT user_id,emoji,created_at,id FROM uno_reactions WHERE table_id=? AND created_at>? ORDER BY created_at DESC LIMIT 20")?.query_map(params![id,now().saturating_sub(3_000)],|r|Ok(json!({"userId":r.get::<_,String>(0)?,"emoji":r.get::<_,String>(1)?,"createdAt":r.get::<_,u64>(2)?,"id":r.get::<_,String>(3)?})))?.collect::<rusqlite::Result<_>>()?,
    })
}
fn member(room: &Room, user: &str) -> Result<usize> {
    room.seats
        .iter()
        .position(|s| s.user_id.as_deref() == Some(user))
        .ok_or_else(denied)
}
fn host(room: &Room, user: &str) -> Result<()> {
    if room.host == user {
        Ok(())
    } else {
        Err(denied())
    }
}
fn waiting(room: &Room) -> Result<()> {
    if room.status == "waiting" {
        Ok(())
    } else {
        Err(fail(
            StatusCode::CONFLICT,
            "TABLE_STARTED",
            "This table has already started",
        ))
    }
}
fn session(conn: &Connection, auth: &AuthUser) -> Result<()> {
    if conn
        .query_row(
            "SELECT 1 FROM sessions WHERE id=? AND user_id=? AND expires_at>datetime('now')",
            params![auth.token, auth.id],
            |_| Ok(()),
        )
        .optional()?
        .is_none()
    {
        return Err(fail(
            StatusCode::UNAUTHORIZED,
            "SESSION_EXPIRED",
            "Sign in again",
        ));
    }
    Ok(())
}
fn snapshot(room: &Room, user: &str) -> Result<Value> {
    let seat = member(room, user)?;
    let view = room.saved.as_ref().map(|s| s.game.view(seat)).transpose()?;
    let sequence = room
        .saved
        .as_ref()
        .map(|s| s.game.accepted_sequence(seat))
        .transpose()?
        .unwrap_or(0);
    Ok(
        json!({"gameId":"unogo","serverNow":now(),"id":room.id,"hostId":room.host,"maxPlayers":room.capacity,"status":room.status,"matchId":room.match_id,"players":room.seats,"localSeat":seat,"acceptedSequence":sequence,"game":view,"pauseVotes":room.saved.as_ref().map(|s|&s.pause_votes),"reactions":room.reactions,"inviteUrl":format!("https://uno.driveatbaku.games/?join={}",room.id)}),
    )
}
fn persist(tx: &Transaction<'_>, room: &mut Room, time: u64) -> Result<()> {
    if let Some(saved) = &room.saved {
        let view = saved.game.view(0)?;
        room.status = match view.phase {
            Phase::Finished { .. } => "finished",
            _ if view.paused => "paused",
            _ => "playing",
        }
        .into();
        tx.execute("UPDATE uno_tables SET status=?,match_id=?,state_json=?,saved_at=?,bot_due=? WHERE id=?",params![room.status,room.match_id,serde_json::to_string(saved)?,time,room.bot_due,room.id])?;
        if let Phase::Finished { winner } = view.phase {
            let match_id = room.match_id.as_ref().expect("started match identity");
            let summary = json!({"gameId":"unogo","matchId":match_id,"tableId":room.id,"players":room.seats,"winner":winner,"counts":view.counts,"finishingCard":view.top,"rulesVersion":view.rules_version});
            let inserted = tx.execute("INSERT OR IGNORE INTO uno_results(match_id,table_id,completed_at,summary_json) VALUES (?,?,?,?)",params![match_id,room.id,time,summary.to_string()])?;
            if inserted == 1 {
                for (seat, player) in room.seats.iter().enumerate() {
                    if let Some(user) = &player.user_id {
                        tx.execute(
                            "INSERT INTO uno_result_players(match_id,user_id,won) VALUES (?,?,?)",
                            params![match_id, user, winner.map(|w| i64::from(w == seat))],
                        )?;
                    }
                }
            }
        }
    }
    room.saved_at = time;
    Ok(())
}
fn tick(tx: &Transaction<'_>, room: &mut Room, time: u64, bots: bool) -> Result<()> {
    let time = time.max(room.saved_at);
    if let Some(saved) = room.saved.as_mut() {
        let mut changed = saved.game.tick(time)?;
        let view = saved.game.view(0)?;
        if !view.paused && bots {
            if let Phase::TouchFive {challenge,starts_at,..}=view.phase {
                for (seat, occupant) in room.seats.iter().enumerate() {
                    if occupant.user_id.is_some() || time<starts_at+350+(seat as u64*181+challenge*97)%700 {continue;}
                    let bot_view=saved.game.view(seat)?;
                    if !matches!(bot_view.phase,Phase::TouchFive{challenge:id,..} if id==challenge){break;}
                    if let Some(action)=bot_action(&bot_view,seat){
                        saved.game.apply(seat,saved.game.accepted_sequence(seat)?+1,action,time)?;
                        changed=true;
                    }
                }
            }
        }
        if !view.paused
            && !matches!(view.phase, Phase::Finished { .. })
            && !matches!(view.phase, Phase::TouchFive { .. })
            && bots
            && time >= room.bot_due
        {
            let actor = match view.phase {
                Phase::Uno { player } => player,
                _ => view.turn,
            };
            if room.seats[actor].user_id.is_none() {
                let bot_view = saved.game.view(actor)?;
                if let Some(action) = bot_action(&bot_view, actor) {
                    saved.game.apply(
                        actor,
                        saved.game.accepted_sequence(actor)? + 1,
                        action,
                        time,
                    )?;
                    changed = true;
                }
            }
        }
        if changed {
            room.bot_due = time.saturating_add(1_500);
        }
        // Checkpoint elapsed decision time, even on otherwise idle human turns.
        if changed || time.saturating_sub(room.saved_at) >= 1_000 {
            persist(tx, room, time)?;
        }
    }
    Ok(())
}

pub(crate) fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/api/health",
            get(|State(app):State<Arc<AppState>>| async move {
                Json(json!({"gameId":"unogo","hostId":app.host_id,"rulesVersion":uno_core::RULES_VERSION}))
            }),
        )
        .route("/api/lobbies", post(create))
        .route("/api/lobbies/{id}", get(read))
        .route("/api/lobbies/{id}/join", post(join))
        .route("/api/lobbies/{id}/leave", post(leave))
        .route("/api/lobbies/{id}/add-bot", post(add_bot))
        .route("/api/lobbies/{id}/seats/{slot}", delete(kick))
        .route("/api/lobbies/{id}/start", post(start))
        .route("/api/lobbies/{id}/commands", post(command))
        .route("/api/lobbies/{id}/pause", post(pause))
        .route("/api/lobbies/{id}/cancel-pause", post(cancel_pause))
        .route("/api/lobbies/{id}/resume", post(resume))
        .route("/api/lobbies/{id}/rematch", post(rematch))
        .route("/api/lobbies/{id}/invite", post(invite))
        .route("/api/invitations", get(invitations))
        .route("/api/invitations/{id}", delete(decline_invite))
        .route("/api/reactions", get(reaction_usage))
        .route("/api/lobbies/{id}/reactions", post(react))
        .route("/api/matches/history", get(history))
        .route("/api/matches/{id}", delete(hide))
        .route("/api/stats/{id}", get(stats))
        .route("/api/leaderboard", get(leaderboard))
        .with_state(state.clone())
        .merge(crate::rest::shared_router(state))
        // Personalized responses must not survive logout in a browser/proxy cache.
        .layer(axum::middleware::map_response(
            |mut response: Response| async move {
                response.headers_mut().insert(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("no-store"),
                );
                response
            },
        ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Create {
    #[serde(default = "default_capacity")]
    max_players: usize,
}
fn default_capacity() -> usize {
    4
}
async fn create(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<Create>,
) -> Result<Json<Value>> {
    if !(2..=10).contains(&body.max_players) {
        return Err(fail(StatusCode::BAD_REQUEST, "SEATS", "Choose 2–10 seats"));
    }
    let mut conn = app.db.conn();
    let tx = conn.transaction()?;
    session(&tx, &auth)?;
    let id = loop {
        let candidate = crate::rest::random_base36(7).to_uppercase();
        if tx
            .query_row(
                "SELECT 1 FROM lobbies WHERE id=? UNION ALL SELECT 1 FROM uno_tables WHERE id=?",
                params![candidate, candidate],
                |_| Ok(()),
            )
            .optional()?
            .is_none()
        {
            break candidate;
        }
    };
    tx.execute(
        "INSERT INTO uno_tables(id,host_id,capacity,saved_at) VALUES (?,?,?,?)",
        params![id, auth.id, body.max_players, now()],
    )?;
    tx.execute(
        "INSERT INTO uno_seats(table_id,seat,user_id,name) VALUES (?,0,?,?)",
        params![id, auth.id, auth.username],
    )?;
    let response = snapshot(&load(&tx, &id)?, &auth.id)?;
    tx.commit()?;
    Ok(Json(response))
}
async fn read(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let id = code(&id)?;
    let mut conn = app.db.conn();
    let tx = conn.transaction()?;
    session(&tx, &auth)?;
    let mut room = load(&tx, &id)?;
    member(&room, &auth.id)?;
    tick(&tx, &mut room, now(), false)?;
    let response = snapshot(&room, &auth.id)?;
    tx.commit()?;
    Ok(Json(response))
}
async fn join(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let id = code(&id)?;
    let mut conn = app.db.conn();
    let tx = conn.transaction()?;
    session(&tx, &auth)?;
    let mut room = load(&tx, &id)?;
    if member(&room, &auth.id).is_err() {
        waiting(&room)?;
        if room.seats.len() >= room.capacity {
            return Err(fail(
                StatusCode::CONFLICT,
                "TABLE_FULL",
                "This table is full",
            ));
        }
        let slot = (0..room.capacity)
            .find(|i| !room.seats.iter().any(|s| s.slot == *i))
            .unwrap();
        tx.execute(
            "INSERT INTO uno_seats(table_id,seat,user_id,name) VALUES (?,?,?,?)",
            params![id, slot, auth.id, auth.username],
        )?;
        room.seats = seats(&tx, &id)?;
    }
    tx.execute(
        "UPDATE uno_invitations SET status='accepted' WHERE table_id=? AND receiver_id=?",
        params![id, auth.id],
    )?;
    let response = snapshot(&room, &auth.id)?;
    tx.commit()?;
    Ok(Json(response))
}
async fn add_bot(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let id = code(&id)?;
    let mut conn = app.db.conn();
    let tx = conn.transaction()?;
    session(&tx, &auth)?;
    let room = load(&tx, &id)?;
    host(&room, &auth.id)?;
    waiting(&room)?;
    if room.seats.len() >= room.capacity {
        return Err(fail(
            StatusCode::CONFLICT,
            "TABLE_FULL",
            "This table is full",
        ));
    }
    let slot = (0..room.capacity)
        .find(|i| !room.seats.iter().any(|s| s.slot == *i))
        .unwrap();
    tx.execute(
        "INSERT INTO uno_seats(table_id,seat,name) VALUES (?,?,?)",
        params![id, slot, format!("Bot {}", slot + 1)],
    )?;
    let response = snapshot(&load(&tx, &id)?, &auth.id)?;
    tx.commit()?;
    Ok(Json(response))
}
async fn kick(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path((id, slot)): Path<(String, usize)>,
) -> Result<Json<Value>> {
    let id = code(&id)?;
    let mut conn = app.db.conn();
    let tx = conn.transaction()?;
    session(&tx, &auth)?;
    let room = load(&tx, &id)?;
    host(&room, &auth.id)?;
    waiting(&room)?;
    if !room
        .seats
        .iter()
        .any(|s| s.slot == slot && s.user_id.as_deref() != Some(&auth.id))
    {
        return Err(denied());
    }
    tx.execute(
        "DELETE FROM uno_seats WHERE table_id=? AND seat=?",
        params![id, slot],
    )?;
    let response = snapshot(&load(&tx, &id)?, &auth.id)?;
    tx.commit()?;
    Ok(Json(response))
}
async fn leave(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let id = code(&id)?;
    let mut conn = app.db.conn();
    let tx = conn.transaction()?;
    session(&tx, &auth)?;
    let room = load(&tx, &id)?;
    member(&room, &auth.id)?;
    if room.status == "waiting" {
        tx.execute(
            "DELETE FROM uno_seats WHERE table_id=? AND user_id=?",
            params![id, auth.id],
        )?;
        let remaining = seats(&tx, &id)?;
        if let Some(next) = remaining.iter().find_map(|s| s.user_id.as_ref()) {
            if room.host == auth.id {
                tx.execute(
                    "UPDATE uno_tables SET host_id=? WHERE id=?",
                    params![next, id],
                )?;
            }
        } else {
            tx.execute("DELETE FROM uno_tables WHERE id=?", [&id])?;
        }
    }
    tx.commit()?;
    Ok(Json(json!({"success":true})))
}
async fn start(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let id = code(&id)?;
    let mut conn = app.db.conn();
    let tx = conn.transaction()?;
    session(&tx, &auth)?;
    let mut room = load(&tx, &id)?;
    host(&room, &auth.id)?;
    waiting(&room)?;
    if room.seats.len() < 2 {
        return Err(fail(
            StatusCode::CONFLICT,
            "PLAYERS",
            "At least two occupied seats are needed",
        ));
    }
    let time = now();
    room.saved = Some(Saved {
        host_paused: false,
        game: Game::new(room.seats.len(), time)?,
        pause_votes: vec![],
    });
    room.match_id = Some(uuid::Uuid::new_v4().to_string());
    room.bot_due = time + 1_500;
    persist(&tx, &mut room, time)?;
    let response = snapshot(&room, &auth.id)?;
    tx.commit()?;
    Ok(Json(response))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Command {
    match_id: String,
    sequence: u64,
    action: Action,
}
async fn command(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Command>,
) -> Result<Json<Value>> {
    let id = code(&id)?;
    let mut conn = app.db.conn();
    let tx = conn.transaction()?;
    session(&tx, &auth)?;
    let mut room = load(&tx, &id)?;
    let seat = member(&room, &auth.id)?;
    if room.match_id.as_deref() != Some(&body.match_id) {
        return Err(fail(
            StatusCode::CONFLICT,
            "STALE_MATCH",
            "Refresh the current match before playing",
        ));
    }
    let time = now().max(room.saved_at);
    tick(&tx, &mut room, time, false)?;
    let saved = room.saved.as_mut().ok_or_else(denied)?;
    let result = saved.game.apply(seat, body.sequence, body.action, time);
    // Commit deadline transitions even if the submitted action has just become stale.
    let receipt: Receipt = match result {
        Ok(r) => r,
        Err(e) => {
            tx.commit()?;
            return Err(e.into());
        }
    };
    room.bot_due = time + 1_500;
    persist(&tx, &mut room, time)?;
    let response = json!({"table":snapshot(&room,&auth.id)?,"receipt":receipt});
    tx.commit()?;
    Ok(Json(response))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Match {
    match_id: String,
}
enum Control {
    Pause,
    Cancel,
    Resume,
    Rematch,
}
fn control(db: &Db, auth: &AuthUser, id: &str, expected: &str, op: Control) -> Result<Value> {
    let id = code(id)?;
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    session(&tx, auth)?;
    let mut room = load(&tx, &id)?;
    member(&room, &auth.id)?;
    if room.match_id.as_deref() != Some(expected) {
        return Err(fail(
            StatusCode::CONFLICT,
            "STALE_MATCH",
            "Refresh the current match",
        ));
    }
    let time = now().max(room.saved_at);
    tick(&tx, &mut room, time, false)?;
    match op {
        Control::Resume if room.status != "paused" => return Err(denied()),
        Control::Pause | Control::Cancel if room.status != "playing" => return Err(denied()),
        _ => {}
    }
    if matches!(op, Control::Rematch) {
        host(&room, &auth.id)?;
        if room.status != "finished" {
            return Err(denied());
        }
        room.saved = Some(Saved {
            host_paused: false,
            game: Game::new(room.seats.len(), time)?,
            pause_votes: vec![],
        });
        room.match_id = Some(uuid::Uuid::new_v4().to_string());
    } else {
        let saved = room.saved.as_mut().ok_or_else(denied)?;
        if matches!(saved.game.view(0)?.phase, Phase::Finished { .. }) {
            return Err(denied());
        }
        match op {
            Control::Pause => {
                if !saved.pause_votes.contains(&auth.id) {
                    saved.pause_votes.push(auth.id.clone());
                }
                if room
                    .seats
                    .iter()
                    .filter_map(|s| s.user_id.as_ref())
                    .all(|u| saved.pause_votes.contains(u))
                {
                    saved.game.pause(time)?;
                }
            }
            Control::Cancel => saved.pause_votes.retain(|u| u != &auth.id),
            Control::Resume => {
                host(&room, &auth.id)?;
                let saved = room.saved.as_mut().unwrap();
                saved.game.resume(time)?;
                saved.pause_votes.clear();
            }
            _ => unreachable!(),
        }
    }
    room.bot_due = time + 1_500;
    persist(&tx, &mut room, time)?;
    let response = snapshot(&room, &auth.id)?;
    tx.commit()?;
    Ok(response)
}
async fn pause(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Match>,
) -> Result<Json<Value>> {
    control(&app.db, &auth, &id, &body.match_id, Control::Pause).map(Json)
}
async fn cancel_pause(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Match>,
) -> Result<Json<Value>> {
    control(&app.db, &auth, &id, &body.match_id, Control::Cancel).map(Json)
}
async fn resume(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Match>,
) -> Result<Json<Value>> {
    control(&app.db, &auth, &id, &body.match_id, Control::Resume).map(Json)
}
async fn rematch(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Match>,
) -> Result<Json<Value>> {
    control(&app.db, &auth, &id, &body.match_id, Control::Rematch).map(Json)
}

async fn history(State(app): State<Arc<AppState>>, auth: AuthUser) -> Result<Json<Value>> {
    let conn = app.db.conn();
    session(&conn, &auth)?;
    let mut entries = Vec::new();
    let mut stmt=conn.prepare("SELECT t.id FROM uno_tables t JOIN uno_seats s ON s.table_id=t.id WHERE s.user_id=? AND t.status!='finished' ORDER BY t.saved_at DESC LIMIT 100")?;
    for id in stmt.query_map([&auth.id], |r| r.get::<_, String>(0))? {
        entries.push(snapshot(&load(&conn, &id?)?, &auth.id)?);
    }
    let mut stmt=conn.prepare("SELECT r.summary_json FROM uno_results r JOIN uno_result_players p ON p.match_id=r.match_id WHERE p.user_id=? AND p.hidden=0 ORDER BY r.completed_at DESC LIMIT 100")?;
    for raw in stmt.query_map([&auth.id], |r| r.get::<_, String>(0))? {
        entries.push(serde_json::from_str(&raw?)?);
    }
    Ok(Json(json!(entries)))
}
async fn hide(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let mut conn = app.db.conn();
    let tx = conn.transaction()?;
    session(&tx, &auth)?;
    if tx.execute(
        "UPDATE uno_result_players SET hidden=1 WHERE match_id=? AND user_id=?",
        params![id, auth.id],
    )? == 0
    {
        return Err(fail(
            StatusCode::NOT_FOUND,
            "MATCH_NOT_FOUND",
            "Completed match not found",
        ));
    }
    tx.commit()?;
    Ok(Json(json!({"success":true})))
}
async fn stats(
    State(app): State<Arc<AppState>>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let conn = app.db.conn();
    let (wins,losses):(u64,u64)=conn.query_row("SELECT COALESCE(SUM(won=1),0),COALESCE(SUM(won=0),0) FROM uno_result_players WHERE user_id=?",[id],|r|Ok((r.get(0)?,r.get(1)?)))?;
    Ok(Json(
        json!({"gameId":"unogo","wins":wins,"losses":losses,"gamesPlayed":wins+losses,"winRate":(wins*100).checked_div(wins+losses).unwrap_or(0)}),
    ))
}

#[derive(Default, Deserialize)]
struct RankingQuery {
    period: Option<String>,
}
async fn leaderboard(
    State(app): State<Arc<AppState>>,
    _auth: AuthUser,
    Query(query): Query<RankingQuery>,
) -> Result<Json<Value>> {
    let period = query.period.as_deref().unwrap_or("all");
    if !["day", "week", "month", "all"].contains(&period) {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "PERIOD",
            "Choose day, week, month or all",
        ));
    }
    let (start, end) =
        crate::rest::ranking_window(period, chrono::Utc::now()).unwrap_or((0, i64::MAX));
    let conn = app.db.conn();
    let mut stmt=conn.prepare("SELECT u.id,u.username,SUM(p.won=1) AS wins,SUM(p.won=0) AS losses,COUNT(*) AS games FROM uno_result_players p JOIN uno_results r ON r.match_id=p.match_id JOIN users u ON u.id=p.user_id WHERE u.is_guest=0 AND p.won IS NOT NULL AND r.completed_at>=? AND r.completed_at<? GROUP BY u.id ORDER BY wins DESC,CAST(SUM(p.won=1) AS REAL)/COUNT(*) DESC,games DESC,u.id ASC LIMIT 50")?;
    let rows=stmt.query_map(params![start,end],|r|{
        let wins:u64=r.get(2)?;let losses:u64=r.get(3)?;let games:u64=r.get(4)?;
        Ok(json!({"id":r.get::<_,String>(0)?,"username":r.get::<_,String>(1)?,"wins":wins,"losses":losses,"gamesPlayed":games,"winRate":(wins*100).checked_div(games).unwrap_or(0)}))
    })?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Json(
        json!({"gameId":"unogo","period":period,"players":rows}),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Invite {
    user_id: String,
}
async fn invite(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Invite>,
) -> Result<Json<Value>> {
    let id = code(&id)?;
    let mut conn = app.db.conn();
    let tx = conn.transaction()?;
    session(&tx, &auth)?;
    let room = load(&tx, &id)?;
    member(&room, &auth.id)?;
    waiting(&room)?;
    if room.seats.len() >= room.capacity {
        return Err(fail(
            StatusCode::CONFLICT,
            "TABLE_FULL",
            "This table is full",
        ));
    }
    if body.user_id==auth.id || tx.query_row("SELECT 1 FROM friendships WHERE status='accepted' AND ((sender_id=? AND receiver_id=?) OR (sender_id=? AND receiver_id=?))",params![auth.id,body.user_id,body.user_id,auth.id],|_|Ok(())).optional()?.is_none(){return Err(denied());}
    tx.execute("INSERT INTO uno_invitations(id,table_id,sender_id,receiver_id) VALUES (?,?,?,?) ON CONFLICT(table_id,receiver_id) DO UPDATE SET status='pending',sender_id=excluded.sender_id",params![uuid::Uuid::new_v4().to_string(),id,auth.id,body.user_id])?;
    tx.commit()?;
    Ok(Json(json!({"success":true,"gameId":"unogo"})))
}
async fn invitations(State(app): State<Arc<AppState>>, auth: AuthUser) -> Result<Json<Value>> {
    let conn = app.db.conn();
    session(&conn, &auth)?;
    let mut stmt=conn.prepare("SELECT i.id,i.table_id,u.username FROM uno_invitations i JOIN uno_tables t ON t.id=i.table_id JOIN users u ON u.id=i.sender_id WHERE i.receiver_id=? AND i.status='pending' AND t.status='waiting' ORDER BY i.rowid DESC LIMIT 20")?;
    let rows=stmt.query_map([auth.id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"tableId":r.get::<_,String>(1)?,"sender":r.get::<_,String>(2)?,"gameId":"unogo"})))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Json(json!(rows)))
}
async fn decline_invite(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let mut conn = app.db.conn();
    let tx = conn.transaction()?;
    session(&tx, &auth)?;
    tx.execute(
        "UPDATE uno_invitations SET status='declined' WHERE id=? AND receiver_id=?",
        params![id, auth.id],
    )?;
    tx.commit()?;
    Ok(Json(json!({"success":true})))
}
async fn reaction_usage(State(app): State<Arc<AppState>>, auth: AuthUser) -> Result<Json<Value>> {
    Ok(Json(
        json!({"counts":app.db.reaction_usage(&auth.id)?,"order":domino_core::REACTIONS}),
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Reaction {
    request_id: String,
    emoji: String,
}
async fn react(
    State(app): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Reaction>,
) -> Result<Json<Value>> {
    if !domino_core::REACTIONS.contains(&body.emoji.as_str())
        || body.request_id.len() > 64
        || body.request_id.is_empty()
    {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            "REACTION",
            "Choose a supported reaction",
        ));
    }
    let id = code(&id)?;
    let mut conn = app.db.conn();
    let tx = conn.transaction()?;
    session(&tx, &auth)?;
    let room = load(&tx, &id)?;
    member(&room, &auth.id)?;
    let key = format!("unogo-{}-{}", auth.id, body.request_id);
    // The same successful send contributes one account-wide usage record.
    let inserted = tx.execute(
        "INSERT OR IGNORE INTO reaction_uses(id,user_id,emoji) VALUES (?,?,?)",
        params![key, auth.id, body.emoji],
    )?;
    if inserted == 1 {
        tx.execute(
            "INSERT INTO uno_reactions(id,table_id,user_id,emoji,created_at) VALUES (?,?,?,?,?)",
            params![key, id, auth.id, body.emoji, now()],
        )?;
        tx.execute(
            "DELETE FROM uno_reactions WHERE created_at<?",
            [now().saturating_sub(10_000)],
        )?;
    }
    tx.commit()?;
    drop(conn);
    Ok(Json(
        json!({"success":true,"counts":app.db.reaction_usage(&auth.id)?}),
    ))
}

pub(crate) fn recover(db: &Db) -> Result<()> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let ids = tx
        .prepare("SELECT id FROM uno_tables WHERE status IN ('playing','paused')")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let time = now();
    for id in ids {
        let mut room = load(&tx, &id)?;
        if let Some(saved) = room.saved.as_mut() {
            saved.game.rebase_clock(room.saved_at, time)?;
        }
        room.bot_due = time + 1_500;
        persist(&tx, &mut room, time)?;
    }
    tx.commit()?;
    Ok(())
}
pub(crate) fn spawn_clock(app: Arc<AppState>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let _gate = app.games.uno.gate.read().await;
            if !app.games.uno.enabled() {
                continue;
            }
            if let Err(e) = advance_all(&app.db, now()) {
                eprintln!("UnoGo clock: {}", e.2);
            }
        }
    })
}

pub(crate) fn host_enabled(db: &Db, enabled: bool) -> Result<()> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let ids = tx
        .prepare("SELECT id FROM uno_tables WHERE status IN ('playing','paused')")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for id in ids {
        let mut room = load(&tx, &id)?;
        let time = now().max(room.saved_at);
        if let Some(saved) = room.saved.as_mut() {
            if enabled && saved.host_paused {
                saved.game.resume(time)?;
                saved.host_paused = false;
            } else if !enabled && !saved.game.view(0)?.paused {
                saved.game.pause(time)?;
                saved.host_paused = true;
            }
            persist(&tx, &mut room, time)?;
        }
    }
    tx.commit()?;
    Ok(())
}
fn advance_all(db: &Db, time: u64) -> Result<()> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let ids = tx
        .prepare("SELECT id FROM uno_tables WHERE status='playing'")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for id in ids {
        let mut room = load(&tx, &id)?;
        tick(&tx, &mut room, time, true)?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests;
