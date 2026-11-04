//! Black-box acceptance test. Run against an isolated server/database with:
//! DOMINO_E2E_URL=http://127.0.0.1:3010 cargo run -p domino-e2e
//! No database access or privileged test endpoints are used.
mod draw_rule;
mod invite;
mod multiplayer;
mod password;
mod social;
mod waiting;
use domino_core::{get_ai_move, legal_moves, AIAction, GameState};
use futures_util::{SinkExt, StreamExt};
use reqwest::{Client, Method};
use serde_json::{json, Value};
use std::{collections::VecDeque, error::Error, time::Duration};
use tokio::{
    net::TcpStream,
    time::{timeout, Instant},
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

struct Api {
    base: String,
    client: Client,
}
impl Api {
    async fn call(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Value,
        expected: u16,
    ) -> Result<Value> {
        let mut request = self
            .client
            .request(method, format!("{}{}", self.base, path));
        if !token.is_empty() {
            request = request.bearer_auth(token);
        }
        let response = request.json(&body).send().await?;
        let status = response.status().as_u16();
        let text = response.text().await?;
        assert_eq!(status, expected, "{path}: {text}");
        Ok(serde_json::from_str(&text)?)
    }
    async fn post(&self, path: &str, token: &str, body: Value) -> Result<Value> {
        self.call(Method::POST, path, token, body, 200).await
    }
    async fn guest(&self, prefix: &str) -> Result<(String, String)> {
        let user = self
            .post("/api/auth/guest", "", json!({"prefix":prefix}))
            .await?;
        Ok((
            user["token"].as_str().unwrap().into(),
            user["user"]["id"].as_str().unwrap().into(),
        ))
    }
}

struct Wire {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    states: VecDeque<Value>,
    ack: u64,
}
impl Wire {
    async fn connect(base: &str, token: &str) -> Result<Self> {
        let url = format!(
            "{}/socket.io/?EIO=4&transport=websocket",
            base.replacen("http", "ws", 1)
        );
        let (ws, _) = connect_async(url).await?;
        let mut wire = Self {
            ws,
            states: VecDeque::new(),
            ack: 0,
        };
        let open = wire.packet().await?;
        assert!(open.starts_with('0'), "missing Engine.IO open: {open}");
        wire.send(format!("40{}", json!({"token":token}))).await?;
        let connected = wire.packet().await?;
        if !connected.starts_with("40") {
            return Err(format!("Socket.IO authentication failed: {connected}").into());
        }
        Ok(wire)
    }
    async fn send(&mut self, text: String) -> Result<()> {
        self.ws.send(Message::Text(text.into())).await?;
        Ok(())
    }
    async fn packet(&mut self) -> Result<String> {
        loop {
            let message = timeout(Duration::from_secs(45), self.ws.next())
                .await?
                .ok_or("WebSocket closed")??;
            match message {
                Message::Text(text) if text == "2" => self.send("3".into()).await?,
                Message::Text(text) if text == "41" || text == "1" => {
                    return Err("Socket.IO disconnected".into())
                }
                Message::Text(text) => return Ok(text.to_string()),
                Message::Ping(data) => self.ws.send(Message::Pong(data)).await?,
                Message::Close(_) => return Err("WebSocket closed".into()),
                _ => {}
            }
        }
    }
    fn collect(&mut self, packet: &str) -> Result<()> {
        if let Some(body) = packet.strip_prefix("42") {
            let event: Value = serde_json::from_str(body)?;
            if event[0] == "game_state_update" {
                self.states.push_back(event[1].clone());
            }
        }
        Ok(())
    }
    async fn emit(&mut self, event: &str, value: Value) -> Result<()> {
        self.send(format!("42{}", json!([event, value]))).await
    }
    async fn command(&mut self, event: &str, value: Value) -> Result<Value> {
        self.ack += 1;
        let id = self.ack;
        self.send(format!("42{id}{}", json!([event, value])))
            .await?;
        let prefix = format!("43{id}[");
        loop {
            let packet = self.packet().await?;
            if packet.starts_with(&prefix) {
                let data: Value = serde_json::from_str(&packet[2 + id.to_string().len()..])?;
                return Ok(data[0].clone());
            }
            self.collect(&packet)?;
        }
    }
    async fn state(&mut self) -> Result<Value> {
        loop {
            if let Some(state) = self.states.pop_front() {
                return Ok(state);
            }
            let packet = self.packet().await?;
            self.collect(&packet)?;
        }
    }
}
fn privacy(state: &Value, user: &str) {
    let hands = state["hands"].as_object().expect("hands object");
    assert_eq!(
        hands.len(),
        if matches!(state["status"].as_str(), Some("round_end" | "finished")) {
            state["players"].as_array().unwrap().len()
        } else {
            1
        },
        "hand visibility does not match round status"
    );
    assert!(hands.contains_key(user), "own hand missing");
    assert_eq!(state["market"], json!([]), "market contents leaked");
    assert!(state["marketCount"].is_number());
    let core: GameState = serde_json::from_value(state.clone()).expect("valid core schema");
    assert!(
        core.board.windows(2).all(|w| w[0][1] == w[1][0]),
        "broken chain"
    );
    let count = hands[user].as_array().unwrap().len()
        + state["opponentHandCounts"]
            .as_object()
            .unwrap()
            .values()
            .map(|n| n.as_u64().unwrap() as usize)
            .sum::<usize>()
        + core.board.len()
        + state["marketCount"].as_u64().unwrap() as usize;
    assert_eq!(count, 28, "tile conservation across private/public views");
}

async fn run(api: &Api) -> Result<()> {
    api.call(Method::GET, "/api/auth/me", "bad-session", json!({}), 401)
        .await?;
    let (token, user) = api.guest("acceptance").await?;
    let (outsider_token, outsider_user) = api.guest("outsider").await?;
    let me = api
        .call(Method::GET, "/api/auth/me", &token, json!({}), 200)
        .await?;
    assert_eq!(me["user"]["id"], user);
    api.call(
        Method::POST,
        "/api/lobbies",
        &token,
        json!({"maxPlayers":5}),
        400,
    )
    .await?;
    let lobby = api
        .post("/api/lobbies", &token, json!({"maxPlayers":2}))
        .await?;
    let lobby = lobby["id"].as_str().unwrap().to_owned();
    api.call(
        Method::POST,
        &format!("/api/lobbies/{lobby}/start"),
        &token,
        json!({}),
        400,
    )
    .await?;
    api.post(&format!("/api/lobbies/{lobby}/add-bot"), &token, json!({}))
        .await?;
    api.call(
        Method::POST,
        &format!("/api/lobbies/{lobby}/join"),
        &outsider_token,
        json!({}),
        400,
    )
    .await?;
    api.call(
        Method::POST,
        &format!("/api/lobbies/{lobby}/add-bot"),
        &outsider_token,
        json!({}),
        403,
    )
    .await?;
    let username = format!(
        "acceptance{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    api.post(
        "/api/auth/upgrade",
        &token,
        json!({"newUsername":username,"password":"acceptance-password-42"}),
    )
    .await?;
    api.call(
        Method::POST,
        "/api/auth/login",
        "",
        json!({"username":username,"password":"wrong-password"}),
        401,
    )
    .await?;
    let login = api
        .post(
            "/api/auth/login",
            "",
            json!({"username":username,"password":"acceptance-password-42"}),
        )
        .await?;
    assert_eq!(login["user"]["id"], user);
    let extra_token = login["token"].as_str().unwrap();
    let mut extra_socket = Wire::connect(&api.base, extra_token).await?;
    api.post("/api/auth/logout-others", &token, json!({}))
        .await?;
    api.call(Method::GET, "/api/auth/me", extra_token, json!({}), 401)
        .await?;
    match timeout(
        Duration::from_secs(5),
        extra_socket.command("next_round", json!({"lobbyId":lobby})),
    )
    .await?
    {
        Err(_) => {}
        Ok(ack) => assert!(
            ack["error"].is_string(),
            "revoked device performed an action"
        ),
    }
    println!("PASS guest upgrade, password login and other-device revocation");
    let mut wire = Wire::connect(&api.base, &token).await?;
    let joined = wire.command("join_lobby", json!(lobby)).await?;
    assert_eq!(
        joined["success"], true,
        "table subscription must be acknowledged"
    );
    api.post(&format!("/api/lobbies/{lobby}/start"), &token, json!({}))
        .await?;
    let mut outsider = Wire::connect(&api.base, &outsider_token).await?;
    let denied = outsider.command("join_lobby", json!(lobby)).await?;
    assert!(
        denied["error"].is_string(),
        "nonmember subscription needs explicit rejection"
    );
    let rejected = outsider
        .command("next_round", json!({"lobbyId":lobby}))
        .await?;
    assert!(rejected["error"].is_string(), "nonmember advanced game");
    assert!(
        outsider.states.is_empty(),
        "nonmember received private state"
    );
    let reaction = wire
        .command("send_reaction", json!({"lobbyId":lobby,"emoji":"👍"}))
        .await?;
    assert_eq!(reaction["success"], true, "reaction must be acknowledged");
    let denied_reaction = outsider
        .command("send_reaction", json!({"lobbyId":lobby,"emoji":"👍"}))
        .await?;
    assert!(
        denied_reaction["error"].is_string(),
        "nonmember reaction rejected"
    );
    println!("PASS authentication, lobby validation, capacity and membership");
    let first = wire.state().await?;
    privacy(&first, &user);
    // Reconnection restores precisely the requesting player's private hand.
    let mut reconnected = Wire::connect(&api.base, &token).await?;
    reconnected.emit("join_lobby", json!(lobby)).await?;
    let restored = reconnected.state().await?;
    privacy(&restored, &user);
    assert_eq!(first["hands"], restored["hands"]);
    reconnected.ws.close(None).await?;
    println!("PASS reconnect and private hand restoration");
    let stopped = wire
        .command("stop_game_request", json!({"lobbyId":lobby}))
        .await?;
    assert_eq!(stopped["success"], true);
    loop {
        if wire.state().await?["status"] == "stopped" {
            break;
        }
    }
    let resumed = wire
        .command("continue_game", json!({"lobbyId":lobby}))
        .await?;
    assert_eq!(resumed["success"], true);
    let resumed_state = wire.state().await?;
    assert_ne!(resumed_state["status"], "stopped");
    println!("PASS unanimous stop and host continuation");
    let mut pending = Some(resumed_state);
    let mut last_processed: Option<Value> = None;
    let mut rounds = 0;
    let mut updates = 0;
    let mut checked_forced_play = false;
    let mut checked_turn_ownership = false;
    let started = Instant::now();
    loop {
        assert!(
            started.elapsed() < Duration::from_secs(1200),
            "full match timed out"
        );
        let state = if let Some(state) = pending.take() {
            state
        } else {
            wire.state().await?
        };
        if last_processed.as_ref() == Some(&state) {
            continue;
        }
        last_processed = Some(state.clone());
        updates += 1;
        privacy(&state, &user);
        if state["status"] == "finished" {
            rounds += 1;
            assert!(state["scores"]
                .as_object()
                .unwrap()
                .values()
                .any(|v| v["permanent"].as_i64().unwrap() >= 101));
            break;
        }
        if state["status"] == "round_end" {
            rounds += 1;
            println!("Round {rounds} completed; scores {}", state["scores"]);
            let ack = wire.command("next_round", json!({"lobbyId":lobby})).await?;
            assert_eq!(ack["success"], true, "next round failed: {ack}");
            continue;
        }
        let core: GameState = serde_json::from_value(state.clone())?;
        if core.players[core.turn_index] != user {
            if !checked_turn_ownership {
                let rejected = wire.command("pass_turn", json!({"lobbyId":lobby})).await?;
                assert!(rejected["error"].is_string(), "out-of-turn pass accepted");
                checked_turn_ownership = true;
            }
            continue;
        }
        if !checked_forced_play && !legal_moves(&core, &user).is_empty() {
            let rejected = wire.command("pass_turn", json!({"lobbyId":lobby})).await?;
            assert!(
                rejected["error"].is_string(),
                "pass accepted with a playable tile"
            );
            if state["marketCount"].as_u64().unwrap() > 0 {
                let ack = wire
                    .command("draw_market", json!({"lobbyId":lobby}))
                    .await?;
                assert_eq!(ack["success"], true, "voluntary draw rejected: {ack}");
                let drawn = wire.state().await?;
                assert_eq!(drawn["turnIndex"], state["turnIndex"]);
                assert_eq!(drawn["board"], state["board"]);
                let count = 2.min(state["marketCount"].as_u64().unwrap()) as usize;
                assert_eq!(
                    drawn["hands"][&user].as_array().unwrap().len(),
                    core.hands[&user].len() + count
                );
                assert_eq!(
                    drawn["marketCount"].as_u64().unwrap(),
                    state["marketCount"].as_u64().unwrap() - count as u64
                );
                pending = Some(drawn);
                checked_forced_play = true;
                continue;
            }
            checked_forced_play = true;
        }
        let (event, body) = if legal_moves(&core, &user).is_empty() {
            (
                if state["marketCount"].as_u64().unwrap() > 0 {
                    "draw_market"
                } else {
                    "pass_turn"
                },
                json!({"lobbyId":lobby}),
            )
        } else {
            let AIAction::Play { tile, end } = get_ai_move(&core, &user) else {
                panic!("legal move unavailable")
            };
            ("play_tile", json!({"lobbyId":lobby,"tile":tile,"end":end}))
        };
        let ack = wire.command(event, body).await?;
        assert_eq!(ack["success"], true, "move rejected: {ack}");
    }
    println!(
        "PASS full match: {rounds} rounds, {updates} state updates, {:.1}s",
        started.elapsed().as_secs_f32()
    );
    let history = api
        .call(Method::GET, "/api/matches/history", &token, json!({}), 200)
        .await?;
    assert!(history["matches"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["id"] == lobby && m["status"] == "finished"));
    let stats = api
        .call(
            Method::GET,
            &format!("/api/stats/{user}"),
            &token,
            json!({}),
            200,
        )
        .await?;
    assert_eq!(stats["gamesPlayed"], 1);
    assert_eq!(
        stats["wins"].as_i64().unwrap() + stats["losses"].as_i64().unwrap(),
        1
    );
    println!("PASS match history and exactly-once statistics");
    let rematch = wire.command("rematch", json!({"lobbyId":lobby})).await?;
    assert_eq!(rematch["success"], true);
    let new_match = wire.state().await?;
    privacy(&new_match, &user);
    assert_eq!(new_match["roundNumber"], 1);
    assert!(new_match["scores"]
        .as_object()
        .unwrap()
        .values()
        .all(|v| v["permanent"] == 0 && v["temporary"] == 0));
    let history = api
        .call(Method::GET, "/api/matches/history", &token, json!({}), 200)
        .await?;
    let archived = history["matches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] != lobby && m["status"] == "finished")
        .expect("rematch erased previous match");
    let archive_id = archived["id"].as_str().unwrap();
    api.call(
        Method::DELETE,
        &format!("/api/matches/{archive_id}"),
        &token,
        json!({}),
        200,
    )
    .await?;
    let history = api
        .call(Method::GET, "/api/matches/history", &token, json!({}), 200)
        .await?;
    assert!(!history["matches"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["id"] == archive_id));
    assert!(history["matches"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["id"] == lobby));
    let stats = api
        .call(
            Method::GET,
            &format!("/api/stats/{user}"),
            &token,
            json!({}),
            200,
        )
        .await?;
    assert_eq!(
        stats["gamesPlayed"], 1,
        "rematch duplicated completed-match statistics"
    );
    assert_eq!(
        wire.command("stop_game_request", json!({"lobbyId":lobby}))
            .await?["success"],
        true
    );
    println!("PASS rematch, preserved history and per-user history hiding");
    multiplayer::run(api, &token, &user, &outsider_token, &outsider_user).await?;
    social::run(api).await?;
    api.post("/api/auth/logout", &token, json!({})).await?;
    api.call(Method::GET, "/api/auth/me", &token, json!({}), 401)
        .await?;
    assert!(
        Wire::connect(&api.base, &token).await.is_err(),
        "revoked token opened a socket"
    );
    println!("PASS logout/session revocation");
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let base = std::env::var("DOMINO_E2E_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3010".into())
        .trim_end_matches('/')
        .to_owned();
    let api = Api {
        base,
        client: Client::builder().timeout(Duration::from_secs(15)).build()?,
    };
    if std::env::args().any(|arg| arg == "--waiting-only") {
        return waiting::run(&api).await;
    }
    if std::env::args().any(|arg| arg == "--invite-only") {
        return invite::run(&api).await;
    }
    if std::env::args().any(|arg| arg == "--draw-rule-only") {
        return draw_rule::run(&api).await;
    }
    if std::env::args().any(|arg| arg == "--password-only") {
        return password::run(&api).await;
    }
    if std::env::args().any(|arg| arg == "--systems-only") {
        let (host_token, host) = api.guest("multihost").await?;
        let (peer_token, peer) = api.guest("multipeer").await?;
        multiplayer::run(&api, &host_token, &host, &peer_token, &peer).await?;
        social::run(&api).await?;
        return Ok(());
    }
    timeout(Duration::from_secs(1250), run(&api)).await??;
    password::run(&api).await?;
    draw_rule::run(&api).await?;
    invite::run(&api).await?;
    println!("All Rust network acceptance checks passed.");
    Ok(())
}
