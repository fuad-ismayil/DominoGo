use crate::{privacy, Api, Result, Wire};
use domino_core::{legal_moves, GameState};
use reqwest::Method;
use serde_json::json;

async fn account(api: &Api, prefix: &str) -> Result<(String, String, String)> {
    let (token, id) = api.guest(prefix).await?;
    let me = api
        .call(Method::GET, "/api/auth/me", &token, json!({}), 200)
        .await?;
    let name = me["user"]["username"].as_str().unwrap().to_owned();
    api.post(
        "/api/auth/upgrade",
        &token,
        json!({"password":"network-acceptance-pass"}),
    )
    .await?;
    Ok((token, id, name))
}

pub async fn run(api: &Api) -> Result<()> {
    let (a_token, a, _) = account(api, "socialhost").await?;
    let (b_token, b, b_name) = account(api, "socialpeer").await?;
    let mut a_wire = Wire::connect(&api.base, &a_token).await?;
    let mut b_wire = Wire::connect(&api.base, &b_token).await?;
    api.post("/api/friends/request", &a_token, json!({"username":b_name}))
        .await?;
    let inbox = api
        .call(Method::GET, "/api/friends", &b_token, json!({}), 200)
        .await?;
    let request = inbox["incoming"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["userId"] == a)
        .unwrap();
    api.post(
        "/api/friends/accept",
        &b_token,
        json!({"friendshipId":request["id"]}),
    )
    .await?;
    let friends = api
        .call(Method::GET, "/api/friends", &a_token, json!({}), 200)
        .await?;
    assert!(friends["friends"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == b && f["isOnline"] == true));
    let table = api
        .post("/api/lobbies", &a_token, json!({"maxPlayers":3}))
        .await?;
    let table = table["id"].as_str().unwrap().to_owned();
    a_wire.emit("join_lobby", json!(table)).await?;
    assert_eq!(
        a_wire
            .command("invite_to_lobby", json!({"lobbyId":table,"friendUserId":b}))
            .await?["success"],
        true
    );
    let inbox = api
        .call(Method::GET, "/api/friends", &b_token, json!({}), 200)
        .await?;
    let invitation = inbox["pendingGameInvites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["lobbyId"] == table)
        .unwrap();
    let joined = b_wire
        .command("accept_game_invite", json!({"inviteId":invitation["id"]}))
        .await?;
    assert_eq!(joined["success"], true);
    assert_eq!(joined["players"].as_array().unwrap().len(), 2);
    let kicked = api
        .post(
            &format!("/api/lobbies/{table}/kick"),
            &a_token,
            json!({"targetUserId":b}),
        )
        .await?;
    assert_eq!(kicked["players"].as_array().unwrap().len(), 1);
    api.post(
        &format!("/api/lobbies/{table}/add-bot"),
        &a_token,
        json!({}),
    )
    .await?;
    api.post(&format!("/api/lobbies/{table}/join"), &b_token, json!({}))
        .await?;
    api.post(&format!("/api/lobbies/{table}/leave"), &a_token, json!({}))
        .await?;
    let transferred = api
        .post(&format!("/api/lobbies/{table}/join"), &b_token, json!({}))
        .await?;
    assert_eq!(
        transferred["creatorId"], b,
        "host was not transferred to remaining human"
    );
    api.post(
        &format!("/api/lobbies/{table}/kick"),
        &b_token,
        json!({"targetUserId":"ai_1"}),
    )
    .await?;
    api.post(&format!("/api/lobbies/{table}/join"), &a_token, json!({}))
        .await?;
    api.call(
        Method::POST,
        &format!("/api/lobbies/{table}/add-bot"),
        &a_token,
        json!({}),
        403,
    )
    .await?;
    println!("PASS friendship request/accept, presence, invite/accept, join/kick, host transfer and host permissions");
    a_wire.emit("join_lobby", json!(table)).await?;
    b_wire.emit("join_lobby", json!(table)).await?;
    api.post(&format!("/api/lobbies/{table}/start"), &b_token, json!({}))
        .await?;
    let a_state = a_wire.state().await?;
    let b_state = b_wire.state().await?;
    privacy(&a_state, &a);
    privacy(&b_state, &b);
    let core: GameState = serde_json::from_value(a_state.clone())?;
    let actor = core.players[core.turn_index].clone();
    let (active, view, actor_token) = if actor == a {
        (&mut a_wire, &a_state, &a_token)
    } else {
        (&mut b_wire, &b_state, &b_token)
    };
    let core: GameState = serde_json::from_value(view.clone())?;
    let (tile, end) = legal_moves(&core, &actor)[0];
    let mut duplicate = Wire::connect(&api.base, actor_token).await?;
    duplicate.emit("join_lobby", json!(table)).await?;
    duplicate.state().await?;
    let payload = json!({"lobbyId":table,"tile":tile,"end":end});
    let (first, second) = tokio::join!(
        active.command("play_tile", payload.clone()),
        duplicate.command("play_tile", payload)
    );
    let replies = [first?, second?];
    assert_eq!(
        replies.iter().filter(|r| r["success"] == true).count(),
        1,
        "simultaneous actions both succeeded or both failed: {replies:?}"
    );
    assert_eq!(replies.iter().filter(|r| r["error"].is_string()).count(), 1);
    let mut snapshot = Wire::connect(&api.base, &a_token).await?;
    snapshot.emit("join_lobby", json!(table)).await?;
    let latest = snapshot.state().await?;
    privacy(&latest, &a);
    assert_eq!(
        latest["board"].as_array().unwrap().len(),
        1,
        "double action mutated board twice"
    );
    assert_eq!(
        a_wire
            .command("stop_game_request", json!({"lobbyId":table}))
            .await?["success"],
        true
    );
    assert_eq!(
        b_wire
            .command("stop_game_request", json!({"lobbyId":table}))
            .await?["success"],
        true
    );
    api.call(
        Method::DELETE,
        &format!("/api/friends/{b}"),
        &a_token,
        json!({}),
        200,
    )
    .await?;
    let friends = api
        .call(Method::GET, "/api/friends", &b_token, json!({}), 200)
        .await?;
    assert!(friends["friends"].as_array().unwrap().is_empty());
    println!("PASS concurrent same-turn submissions mutate state exactly once; friendship removal");
    Ok(())
}
