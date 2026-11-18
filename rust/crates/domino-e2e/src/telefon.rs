use super::*;

pub async fn run(api: &Api) -> Result<()> {
    let (ht, host) = api.guest("telefonhost").await?;
    let (pt, peer) = api.guest("telefonpeer").await?;
    api.call(
        Method::POST,
        "/api/lobbies",
        &ht,
        json!({"maxPlayers":2,"mode":"unknown"}),
        400,
    )
    .await?;
    let created = api
        .post(
            "/api/lobbies",
            &ht,
            json!({"maxPlayers":2,"mode":"telefon","drawRule":"when_blocked"}),
        )
        .await?;
    let table = created["id"].as_str().unwrap();
    let joined = api
        .post(
            &format!("/api/lobbies/{table}/join"),
            &pt,
            json!({"mode":"101"}),
        )
        .await?;
    assert_eq!(joined["mode"], "telefon");
    let mut hw = Wire::connect(&api.base, &ht).await?;
    let mut pw = Wire::connect(&api.base, &pt).await?;
    hw.command("join_lobby", json!(table)).await?;
    pw.command("join_lobby", json!(table)).await?;
    api.post(
        &format!("/api/lobbies/{table}/start"),
        &ht,
        json!({"mode":"101"}),
    )
    .await?;
    let mut hs = hw.state().await?;
    let mut ps = pw.state().await?;
    for emoji in domino_core::REACTIONS {
        assert_eq!(
            hw.command("send_reaction", json!({"lobbyId":table,"emoji":emoji}))
                .await?["success"],
            true
        );
    }
    let mut branches = 0;
    for step in 0..12000 {
        assert_eq!(hs["mode"], "telefon");
        privacy(&hs, &host);
        privacy(&ps, &peer);
        if hs["status"] == "finished" {
            break;
        }
        if hs["status"] == "round_end" {
            assert_eq!(
                hw.command("next_round", json!({"lobbyId":table})).await?["success"],
                true
            );
        } else {
            let actor = hs["players"][hs["turnIndex"].as_u64().unwrap() as usize]
                .as_str()
                .unwrap();
            let view = if actor == host { &hs } else { &ps };
            let core: GameState = serde_json::from_value(view.clone())?;
            let (event, body) = match get_ai_move(&core, actor) {
                AIAction::Play { tile, end } => {
                    if matches!(end, domino_core::End::Telefon(_, 2 | 3)) {
                        branches += 1;
                    }
                    (
                        "play_tile",
                        json!({"lobbyId":table,"tile":tile,"end":end,"mode":"101"}),
                    )
                }
                AIAction::Draw => ("draw_market", json!({"lobbyId":table})),
                AIAction::Pass if view["marketCount"].as_u64().unwrap() > 0 => {
                    ("draw_market", json!({"lobbyId":table}))
                }
                AIAction::Pass => ("pass_turn", json!({"lobbyId":table})),
            };
            let wire = if actor == host { &mut hw } else { &mut pw };
            let ack = wire.command(event, body).await?;
            assert_eq!(ack["success"], true, "{ack}");
        }
        hs = hw.state().await?;
        ps = pw.state().await?;
        if step == 10 {
            let mut reconnected = Wire::connect(&api.base, &ht).await?;
            reconnected.command("join_lobby", json!(table)).await?;
            let restored = reconnected.state().await?;
            assert_eq!(restored["telefon"], hs["telefon"]);
            assert_eq!(restored["scores"], hs["scores"]);
        }
        assert!(step < 11999);
    }
    assert_eq!(hs["status"], "finished");
    let winner = hs["lastWinner"].as_str().unwrap();
    assert!(hs["scores"][winner]["permanent"].as_i64().unwrap() >= 365);
    let history = api
        .call(Method::GET, "/api/matches/history", &ht, json!({}), 200)
        .await?;
    let row = history["matches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == table)
        .unwrap();
    assert_eq!(row["mode"], "telefon");
    assert_eq!(row["winner"], winner);
    let stats = api
        .call(
            Method::GET,
            &format!("/api/stats/{winner}"),
            &ht,
            json!({}),
            200,
        )
        .await?;
    assert_eq!(stats["wins"], 1);
    assert_eq!(
        hw.command("rematch", json!({"lobbyId":table,"mode":"101"}))
            .await?["success"],
        true
    );
    let rematch = hw.state().await?;
    assert_eq!(rematch["mode"], "telefon");
    assert!(rematch["telefon"].is_null());
    hw.command("stop_game_request", json!({"lobbyId":table}))
        .await?;
    pw.command("stop_game_request", json!({"lobbyId":table}))
        .await?;
    println!("PASS Telefon: full 365-point match ({branches} side moves), private hands, reconnect, history, winner statistics, rematch mode and all 16 reactions");
    Ok(())
}
