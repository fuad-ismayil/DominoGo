use super::*;

pub async fn run(api: &Api) -> Result<()> {
    let (host_token, _) = api.guest("invitehost").await?;
    let (a_token, a) = api.guest("inviteseat").await?;
    let (b_token, b) = api.guest("inviteseat").await?;
    let full = api
        .post("/api/lobbies", &host_token, json!({"maxPlayers":2}))
        .await?;
    let full = full["id"].as_str().unwrap();
    api.post(
        &format!("/api/lobbies/{full}/add-bot"),
        &host_token,
        json!({}),
    )
    .await?;
    let denied = api
        .call(
            Method::POST,
            &format!("/api/lobbies/{full}/join"),
            &a_token,
            json!({}),
            400,
        )
        .await?;
    assert_eq!(denied["code"], "TABLE_FULL");
    let member = api
        .post(&format!("/api/lobbies/{full}/join"), &host_token, json!({}))
        .await?;
    assert_eq!(member["players"].as_array().unwrap().len(), 2);
    assert!(member["players"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"].as_str().unwrap().starts_with("ai_")));
    let race = api
        .post("/api/lobbies", &host_token, json!({"maxPlayers":3}))
        .await?;
    let race = race["id"].as_str().unwrap();
    api.post(
        &format!("/api/lobbies/{race}/add-bot"),
        &host_token,
        json!({}),
    )
    .await?;
    let route = format!("{}/api/lobbies/{race}/join", api.base);
    let (ar, br) = tokio::join!(
        api.client
            .post(&route)
            .bearer_auth(&a_token)
            .json(&json!({}))
            .send(),
        api.client
            .post(&route)
            .bearer_auth(&b_token)
            .json(&json!({}))
            .send()
    );
    let ar = ar?;
    let br = br?;
    let a_won = ar.status().as_u16() == 200;
    let b_won = br.status().as_u16() == 200;
    assert_ne!(a_won, b_won, "Exactly one visitor must get the final seat");
    let av: Value = ar.json().await?;
    let bv: Value = br.json().await?;
    let denied = if a_won { &bv } else { &av };
    assert_eq!(denied["code"], "TABLE_FULL");
    let member = api
        .post(&format!("/api/lobbies/{race}/join"), &host_token, json!({}))
        .await?;
    assert_eq!(member["players"].as_array().unwrap().len(), 3);
    assert_eq!(member["status"], "waiting", "Joining must not start a game");
    let winner_token = if a_won { &a_token } else { &b_token };
    let winner_id = if a_won { &a } else { &b };
    let again = api
        .post(
            &format!("/api/lobbies/{race}/join"),
            winner_token,
            json!({}),
        )
        .await?;
    assert_eq!(
        again["players"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["id"] == *winner_id)
            .count(),
        1
    );
    api.post(
        &format!("/api/lobbies/{full}/start"),
        &host_token,
        json!({}),
    )
    .await?;
    let denied = api
        .call(
            Method::POST,
            &format!("/api/lobbies/{full}/join"),
            &a_token,
            json!({}),
            400,
        )
        .await?;
    assert_eq!(denied["code"], "TABLE_FULL");
    let underway = api
        .post("/api/lobbies", &host_token, json!({"maxPlayers":3}))
        .await?;
    let underway = underway["id"].as_str().unwrap();
    api.post(
        &format!("/api/lobbies/{underway}/add-bot"),
        &host_token,
        json!({}),
    )
    .await?;
    api.post(
        &format!("/api/lobbies/{underway}/start"),
        &host_token,
        json!({}),
    )
    .await?;
    let denied = api
        .call(
            Method::POST,
            &format!("/api/lobbies/{underway}/join"),
            &a_token,
            json!({}),
            400,
        )
        .await?;
    assert_eq!(denied["code"], "TABLE_STARTED");
    api.call(
        Method::POST,
        "/api/lobbies/INVALID/join",
        &a_token,
        json!({}),
        404,
    )
    .await?;
    println!("PASS invite admission: bots occupy seats, waiting for host, atomic final-seat race, idempotent member rejoin, full/started/missing outcomes");
    Ok(())
}
