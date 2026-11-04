use super::*;
pub async fn run(api: &Api) -> Result<()> {
    let (token, _) = api.guest("waitingcheck").await?;
    let active = api
        .post("/api/lobbies", &token, json!({"maxPlayers":2}))
        .await?;
    let active = active["id"].as_str().unwrap();
    api.post(&format!("/api/lobbies/{active}/add-bot"), &token, json!({}))
        .await?;
    let abandoned = api
        .post("/api/lobbies", &token, json!({"maxPlayers":2}))
        .await?;
    let abandoned = abandoned["id"].as_str().unwrap();
    api.post(
        &format!("/api/lobbies/{abandoned}/add-bot"),
        &token,
        json!({}),
    )
    .await?;
    let mut wire = Wire::connect(&api.base, &token).await?;
    wire.command("join_lobby", json!(active)).await?;
    let history = api
        .call(Method::GET, "/api/matches/history", &token, json!({}), 200)
        .await?;
    assert!(history["matches"].as_array().unwrap().is_empty());
    println!("PASS waiting rooms excluded from history; checking real 90-second cleanup grace");
    for _ in 0..11 {
        tokio::time::sleep(Duration::from_secs(10)).await;
        assert_eq!(
            wire.command("join_lobby", json!(active)).await?["success"],
            true
        );
    }
    api.call(
        Method::POST,
        &format!("/api/lobbies/{abandoned}/join"),
        &token,
        json!({}),
        404,
    )
    .await?;
    api.post(&format!("/api/lobbies/{active}/join"), &token, json!({}))
        .await?;
    api.post(&format!("/api/lobbies/{active}/start"), &token, json!({}))
        .await?;
    let history = api
        .call(Method::GET, "/api/matches/history", &token, json!({}), 200)
        .await?;
    assert!(history["matches"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["id"] == active));
    api.post(&format!("/api/lobbies/{active}/leave"), &token, json!({}))
        .await?;
    api.post(&format!("/api/lobbies/{active}/join"), &token, json!({}))
        .await?;
    let leave = api
        .post("/api/lobbies", &token, json!({"maxPlayers":2}))
        .await?;
    let leave = leave["id"].as_str().unwrap();
    api.post(&format!("/api/lobbies/{leave}/leave"), &token, json!({}))
        .await?;
    api.call(
        Method::POST,
        &format!("/api/lobbies/{leave}/join"),
        &token,
        json!({}),
        404,
    )
    .await?;
    println!("PASS abandoned bot-filled rooms deleted, active waiting room retained, started match preserved/visible, explicit waiting-room leave deletes immediately");
    Ok(())
}
