use crate::{privacy, Api, Result, Wire};
use serde_json::json;
pub async fn run(
    api: &Api,
    token: &str,
    user: &str,
    outsider_token: &str,
    outsider_user: &str,
) -> Result<()> {
    // Two independently authenticated human clients exercise deadline delivery,
    // unanimous voting and the real production watchdog (no test-only clocks).
    let table = api
        .post("/api/lobbies", token, json!({"maxPlayers":2}))
        .await?;
    let table = table["id"].as_str().unwrap().to_owned();
    api.post(
        &format!("/api/lobbies/{table}/join"),
        outsider_token,
        json!({}),
    )
    .await?;
    let mut host_wire = Wire::connect(&api.base, token).await?;
    let mut peer_wire = Wire::connect(&api.base, outsider_token).await?;
    host_wire.emit("join_lobby", json!(table)).await?;
    peer_wire.emit("join_lobby", json!(table)).await?;
    api.post(&format!("/api/lobbies/{table}/start"), token, json!({}))
        .await?;
    let host_state = host_wire.state().await?;
    let peer_state = peer_wire.state().await?;
    privacy(&host_state, user);
    privacy(&peer_state, outsider_user);
    assert!(
        host_state["turnDeadline"].is_number(),
        "multiplayer deadline missing"
    );
    assert_eq!(host_state["turnDeadline"], peer_state["turnDeadline"]);
    let initial_deadline = host_state["turnDeadline"].as_i64().unwrap();
    let mut timed_state = host_wire.state().await?;
    while timed_state == host_state {
        timed_state = host_wire.state().await?;
    }
    privacy(&timed_state, user);
    assert_ne!(
        timed_state["board"], host_state["board"],
        "timeout did not make its legal opening move"
    );
    assert!(
        timed_state["turnDeadline"].as_i64().unwrap() >= initial_deadline + 29_000,
        "next player did not receive a fresh turn deadline"
    );
    assert_eq!(
        host_wire
            .command("stop_game_request", json!({"lobbyId":table}))
            .await?["success"],
        true
    );
    assert_eq!(
        peer_wire
            .command("stop_game_request", json!({"lobbyId":table}))
            .await?["success"],
        true
    );
    loop {
        if host_wire.state().await?["status"] == "stopped" {
            break;
        }
    }
    assert!(
        peer_wire
            .command("continue_game", json!({"lobbyId":table}))
            .await?["error"]
            .is_string(),
        "non-host resumed stopped match"
    );
    assert_eq!(
        host_wire
            .command("continue_game", json!({"lobbyId":table}))
            .await?["success"],
        true
    );
    let resumed = host_wire.state().await?;
    assert_eq!(resumed["status"], "playing");
    assert!(
        resumed["turnDeadline"].is_number(),
        "resumed state omitted new turn deadline"
    );
    assert_eq!(
        host_wire
            .command("stop_game_request", json!({"lobbyId":table}))
            .await?["success"],
        true
    );
    assert_eq!(
        peer_wire
            .command("stop_game_request", json!({"lobbyId":table}))
            .await?["success"],
        true
    );
    println!("PASS two-human private views, real 30-second timeout, fresh deadlines, unanimous stop and host-only resume");
    Ok(())
}
