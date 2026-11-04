use super::*;

pub async fn run(api: &Api) -> Result<()> {
    let (host_token, host) = api.guest("drawhost").await?;
    let (peer_token, peer) = api.guest("drawpeer").await?;
    for invalid in [json!(true), json!("unknown"), Value::Null] {
        api.call(
            Method::POST,
            "/api/lobbies",
            &host_token,
            json!({"maxPlayers":2,"drawRule":invalid}),
            400,
        )
        .await?;
    }
    for rule in ["anytime", "when_blocked"] {
        let opposite = if rule == "anytime" {
            "when_blocked"
        } else {
            "anytime"
        };
        let created = api
            .post(
                "/api/lobbies",
                &host_token,
                json!({"maxPlayers":2,"drawRule":rule}),
            )
            .await?;
        let table = created["id"].as_str().unwrap();
        let joined = api
            .post(
                &format!("/api/lobbies/{table}/join"),
                &peer_token,
                json!({"drawRule":opposite}),
            )
            .await?;
        assert_eq!(joined["drawRule"], rule);
        let mut host_wire = Wire::connect(&api.base, &host_token).await?;
        let mut peer_wire = Wire::connect(&api.base, &peer_token).await?;
        host_wire.command("join_lobby", json!(table)).await?;
        peer_wire.command("join_lobby", json!(table)).await?;
        api.post(
            &format!("/api/lobbies/{table}/start"),
            &host_token,
            json!({"drawRule":opposite}),
        )
        .await?;
        let mut hs = host_wire.state().await?;
        let mut ps = peer_wire.state().await?;
        let actor = hs["players"][hs["turnIndex"].as_u64().unwrap() as usize]
            .as_str()
            .unwrap();
        let initial: GameState = serde_json::from_value(if actor == host {
            hs.clone()
        } else {
            ps.clone()
        })?;
        assert!(!legal_moves(&initial, actor).is_empty());
        let wire = if actor == host {
            &mut host_wire
        } else {
            &mut peer_wire
        };
        let ack = wire
            .command("draw_market", json!({"lobbyId":table,"drawRule":opposite}))
            .await?;
        if rule == "anytime" {
            assert_eq!(ack["success"], true);
            hs = host_wire.state().await?;
            ps = peer_wire.state().await?;
            assert_eq!(hs["marketCount"], 12);
            assert_eq!(hs["turnIndex"], initial.turn_index);
        } else {
            assert!(ack["error"].is_string(), "Restricted draw was accepted");
        }
        // Play one entire round with two real client connections.
        let mut turns = 0;
        while hs["status"] == "playing" {
            turns += 1;
            assert!(turns < 500);
            assert_eq!(hs["drawRule"], rule);
            assert_eq!(ps["drawRule"], rule);
            privacy(&hs, &host);
            privacy(&ps, &peer);
            let actor = hs["players"][hs["turnIndex"].as_u64().unwrap() as usize]
                .as_str()
                .unwrap();
            let view = if actor == host { &hs } else { &ps };
            let core: GameState = serde_json::from_value(view.clone())?;
            let (event, mut body) = match get_ai_move(&core, actor) {
                AIAction::Play { tile, end } => {
                    ("play_tile", json!({"lobbyId":table,"tile":tile,"end":end}))
                }
                AIAction::Draw => ("draw_market", json!({"lobbyId":table})),
                AIAction::Pass if view["marketCount"].as_u64().unwrap() > 0 => {
                    ("draw_market", json!({"lobbyId":table}))
                }
                AIAction::Pass => ("pass_turn", json!({"lobbyId":table})),
            };
            body["drawRule"] = json!(opposite);
            let played = body.get("tile").cloned();
            let wire = if actor == host {
                &mut host_wire
            } else {
                &mut peer_wire
            };
            assert_eq!(wire.command(event, body).await?["success"], true);
            hs = host_wire.state().await?;
            ps = peer_wire.state().await?;
            if let Some(tile) = played {
                let last = &hs["lastPlayedTile"];
                assert!(*last == tile || *last == json!([tile[1], tile[0]]));
                if hs["status"] != "playing" {
                    assert_eq!(hs["roundFinishingTile"], *last);
                }
            }
        }
        privacy(&hs, &host);
        privacy(&ps, &peer);
        assert_eq!(
            hs["hands"], ps["hands"],
            "Completed hands must be visible to every player"
        );
        if hs["status"] == "round_end" {
            assert_eq!(
                host_wire
                    .command("next_round", json!({"lobbyId":table,"drawRule":opposite}))
                    .await?["success"],
                true
            );
            hs = host_wire.state().await?;
            ps = peer_wire.state().await?;
            assert_eq!(hs["drawRule"], rule);
            assert_eq!(ps["drawRule"], rule);
        }
        let mut reconnect = Wire::connect(&api.base, &host_token).await?;
        reconnect.command("join_lobby", json!(table)).await?;
        assert_eq!(reconnect.state().await?["drawRule"], rule);
        if hs["status"] == "playing" {
            host_wire
                .command("stop_game_request", json!({"lobbyId":table}))
                .await?;
            peer_wire
                .command("stop_game_request", json!({"lobbyId":table}))
                .await?;
            assert_eq!(host_wire.state().await?["drawRule"], rule);
            peer_wire.state().await?;
        }
        assert_eq!(
            host_wire
                .command("rematch", json!({"lobbyId":table,"drawRule":opposite}))
                .await?["success"],
            true
        );
        assert_eq!(host_wire.state().await?["drawRule"], rule);
        assert_eq!(peer_wire.state().await?["drawRule"], rule);
        // Stop the fresh match so this test leaves no turn watchdog running.
        host_wire
            .command("stop_game_request", json!({"lobbyId":table}))
            .await?;
        peer_wire
            .command("stop_game_request", json!({"lobbyId":table}))
            .await?;
        println!("PASS {rule}: creation, draw enforcement, tampered-payload rejection, full round, next round, reconnect and rematch");
    }
    Ok(())
}
