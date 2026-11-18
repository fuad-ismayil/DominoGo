use super::*;
use crate::domain::Registry;
use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, RwLock},
};
use tower::ServiceExt;

fn setup() -> Arc<AppState> {
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
    for user in ["alice", "bob", "eve"] {
        app.db
            .execute("INSERT INTO users(id,username) VALUES (?,?)", [user, user])
            .unwrap();
        app.db
            .execute(
                "INSERT INTO sessions(id,user_id,expires_at) VALUES (?,?,datetime('now','+1 day'))",
                [user, user],
            )
            .unwrap();
    }
    app
}
async fn req(
    app: &Arc<AppState>,
    user: &str,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = router(app.clone())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("Authorization", format!("Bearer {user}"))
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let raw = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&raw)
            .unwrap_or_else(|_| json!({"text":String::from_utf8_lossy(&raw)})),
    )
}
async fn create_room(app: &Arc<AppState>, capacity: usize) -> String {
    let (status, v) = req(
        app,
        "alice",
        "POST",
        "/api/lobbies",
        json!({"maxPlayers":capacity}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    v["id"].as_str().unwrap().into()
}

#[tokio::test]
async fn capacity_rejoin_permissions_and_wrong_game() {
    let app = setup();
    let id = create_room(&app, 2).await;
    let url = format!("/api/lobbies/{id}");
    assert_eq!(
        req(&app, "bob", "GET", &url, json!({})).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        req(&app, "bob", "POST", &format!("{url}/start"), json!({}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        req(&app, "bob", "POST", &format!("{url}/join"), json!({}))
            .await
            .0,
        StatusCode::OK
    );
    let a = req(&app, "eve", "POST", &format!("{url}/join"), json!({})).await;
    assert_eq!(a.1["code"], "TABLE_FULL");
    let a = req(&app, "bob", "POST", &format!("{url}/join"), json!({})).await;
    assert_eq!(a.1["players"].as_array().unwrap().len(), 2);
    assert_eq!(
        req(&app, "alice", "POST", &format!("{url}/start"), json!({}))
            .await
            .0,
        StatusCode::OK
    );
    let a = req(&app, "eve", "POST", &format!("{url}/join"), json!({})).await;
    assert_eq!(a.1["code"], "TABLE_STARTED");
    app.db
        .execute(
            "INSERT INTO lobbies(id,creator_id,max_players) VALUES ('DOMINO1','alice',2)",
            [],
        )
        .unwrap();
    assert_eq!(
        req(
            &app,
            "alice",
            "POST",
            "/api/lobbies/DOMINO1/join",
            json!({})
        )
        .await
        .1["code"],
        "WRONG_GAME"
    );
    assert!(app
        .db
        .execute(
            "INSERT INTO lobbies(id,creator_id,max_players) VALUES (?,'alice',2)",
            [&id]
        )
        .is_err());
    assert!(app
        .db
        .execute(
            "INSERT INTO uno_tables(id,host_id,capacity,saved_at) VALUES ('DOMINO1','alice',2,0)",
            []
        )
        .is_err());
}

#[tokio::test]
async fn private_views_stale_commands_and_revocation() {
    let app = setup();
    let id = create_room(&app, 2).await;
    let url = format!("/api/lobbies/{id}");
    req(&app, "bob", "POST", &format!("{url}/join"), json!({})).await;
    req(&app, "alice", "POST", &format!("{url}/start"), json!({})).await;
    let (_, a) = req(&app, "alice", "GET", &url, json!({})).await;
    let (_, b) = req(&app, "bob", "GET", &url, json!({})).await;
    let ah = a["game"]["hand"].as_array().unwrap();
    let bh = b["game"]["hand"].as_array().unwrap();
    assert!(ah.iter().all(|c| !bh.iter().any(|d| d["id"] == c["id"])));
    for k in ["hands", "draw", "rng", "pending", "evidence"] {
        assert!(a["game"].get(k).is_none());
    }
    let stale = req(
        &app,
        "alice",
        "POST",
        &format!("{url}/commands"),
        json!({"matchId":"other","sequence":1,"action":"Draw"}),
    )
    .await;
    assert_eq!(stale.1["code"], "STALE_MATCH");
    app.db
        .execute("DELETE FROM sessions WHERE id='bob'", [])
        .unwrap();
    assert_eq!(
        req(&app, "bob", "GET", &url, json!({})).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        req(&app, "bob", "POST", &format!("{url}/join"), json!({}))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn complete_round_stats_rematch_and_per_user_history() {
    let app = setup();
    let id = create_room(&app, 2).await;
    let url = format!("/api/lobbies/{id}");
    req(&app, "bob", "POST", &format!("{url}/join"), json!({})).await;
    let (_, initial) = req(&app, "alice", "POST", &format!("{url}/start"), json!({})).await;
    let match_id = initial["matchId"].as_str().unwrap();
    let mut finished = false;
    for _ in 0..20_000 {
        let (_, v) = req(&app, "alice", "GET", &url, json!({})).await;
        let view: uno_core::View = serde_json::from_value(v["game"].clone()).unwrap();
        if matches!(view.phase, Phase::Finished { .. }) {
            finished = true;
            break;
        }
        if matches!(view.phase,Phase::TouchFive{..}) && !view.touch_ready {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            continue;
        }
        let seat = match view.phase {
            Phase::Uno { player } => player,
            Phase::TouchFive { ref taps,.. } => (0..view.counts.len()).find(|s|!taps.contains(s)).unwrap(),
            _ => view.turn,
        };
        let user = ["alice", "bob"][seat];
        let (_, v) = req(&app, user, "GET", &url, json!({})).await;
        let view: uno_core::View = serde_json::from_value(v["game"].clone()).unwrap();
        let action = bot_action(&view, seat).unwrap();
        let payload = json!({"matchId":match_id,"sequence":v["acceptedSequence"].as_u64().unwrap()+1,"action":action});
        let (status, result) = req(
            &app,
            user,
            "POST",
            &format!("{url}/commands"),
            payload.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
        // Replaying the same accepted command must not execute another mutation.
        assert_eq!(
            req(&app, user, "POST", &format!("{url}/commands"), payload)
                .await
                .0,
            StatusCode::CONFLICT
        );
    }
    assert!(finished);
    let rows: i64 = app
        .db
        .query_one("SELECT COUNT(*) FROM uno_result_players", [], |r| r.get(0))
        .unwrap()
        .unwrap();
    assert_eq!(rows, 2);
    let legacy: i64 = app
        .db
        .query_one("SELECT COUNT(*) FROM player_stats", [], |r| r.get(0))
        .unwrap()
        .unwrap();
    assert_eq!(legacy, 0);
    for user in ["alice", "bob"] {
        let (_, s) = req(&app, user, "GET", &format!("/api/stats/{user}"), json!({})).await;
        assert_eq!(s["gamesPlayed"], 1);
    }
    assert_eq!(
        req(
            &app,
            "alice",
            "DELETE",
            &format!("/api/matches/{match_id}"),
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        req(&app, "alice", "GET", "/api/matches/history", json!({}))
            .await
            .1
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        req(&app, "bob", "GET", "/api/matches/history", json!({}))
            .await
            .1
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let (_, new) = req(
        &app,
        "alice",
        "POST",
        &format!("{url}/rematch"),
        json!({"matchId":match_id}),
    )
    .await;
    assert_ne!(new["matchId"], match_id);
    let old: i64 = app
        .db
        .query_one(
            "SELECT COUNT(*) FROM uno_results WHERE match_id=?",
            [match_id],
            |r| r.get(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(old, 1);
}

#[tokio::test]
async fn pause_votes_and_recovery_freeze_deadlines() {
    let app = setup();
    let id = create_room(&app, 2).await;
    let url = format!("/api/lobbies/{id}");
    req(&app, "bob", "POST", &format!("{url}/join"), json!({})).await;
    let (_, initial) = req(&app, "alice", "POST", &format!("{url}/start"), json!({})).await;
    let body = json!({"matchId":initial["matchId"]});
    let (_, v) = req(&app, "alice", "POST", &format!("{url}/pause"), body.clone()).await;
    assert_eq!(v["status"], "playing");
    let (_, v) = req(&app, "bob", "POST", &format!("{url}/pause"), body.clone()).await;
    assert_eq!(v["status"], "paused");
    recover(&app.db).unwrap();
    let (_, v) = req(&app, "alice", "GET", &url, json!({})).await;
    assert_eq!(v["status"], "paused");
    assert!(v["game"]["deadline"].is_null());
    let (_, v) = req(&app, "alice", "POST", &format!("{url}/resume"), body).await;
    assert_eq!(v["status"], "playing");
    assert!(v["game"]["deadline"].as_u64().unwrap() > now());
}

#[tokio::test]
async fn bots_use_saved_clock_and_guest_survives_cleanup() {
    let app = setup();
    let id = create_room(&app, 10).await;
    let url = format!("/api/lobbies/{id}");
    for _ in 1..10 {
        assert_eq!(
            req(&app, "alice", "POST", &format!("{url}/add-bot"), json!({}))
                .await
                .0,
            StatusCode::OK
        );
    }
    let (status, _) = req(&app, "alice", "POST", &format!("{url}/start"), json!({})).await;
    assert_eq!(status, StatusCode::OK);
    advance_all(&app.db, now() + 2_000).unwrap();
    app.db
        .execute(
            "UPDATE users SET is_guest=1,last_active_at=datetime('now','-2 days') WHERE id='alice'",
            [],
        )
        .unwrap();
    app.db.cleanup_stale_guests().unwrap();
    assert!(app
        .db
        .query_one("SELECT 1 FROM users WHERE id='alice'", [], |_| Ok(()))
        .unwrap()
        .is_some());
    let (_, v) = req(&app, "alice", "GET", &url, json!({})).await;
    assert_eq!(v["players"].as_array().unwrap().len(), 10);
    let bots: i64 = app
        .db
        .query_one(
            "SELECT COUNT(*) FROM users WHERE username LIKE 'Bot %'",
            [],
            |r| r.get(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(bots, 0);
}

#[tokio::test]
async fn recovery_preserves_elapsed_time_since_turn_started() {
    let app = setup();
    let id = create_room(&app, 2).await;
    let url = format!("/api/lobbies/{id}");
    req(&app, "bob", "POST", &format!("{url}/join"), json!({})).await;
    let (_, initial) = req(&app, "alice", "POST", &format!("{url}/start"), json!({})).await;
    let original_deadline = initial["game"]["deadline"].as_u64().unwrap();
    advance_all(&app.db, now() + 8_000).unwrap();
    recover(&app.db).unwrap();
    let (_, restored) = req(&app, "alice", "GET", &url, json!({})).await;
    let restored_deadline = restored["game"]["deadline"].as_u64().unwrap();
    assert!((7_500..=8_500).contains(&original_deadline.saturating_sub(restored_deadline)));
    assert_eq!(initial["game"]["hand"], restored["game"]["hand"]);
}

#[tokio::test]
async fn completed_guest_expiry_keeps_other_players_archived_history() {
    let app = setup();
    app.db.execute("INSERT INTO uno_tables(id,host_id,capacity,status,saved_at) VALUES ('EXPIRED','bob',2,'finished',0)",[]).unwrap();
    app.db.execute("INSERT INTO uno_seats(table_id,seat,user_id,name) VALUES ('EXPIRED',0,'bob','bob'),('EXPIRED',1,'alice','alice')",[]).unwrap();
    app.db.execute("INSERT INTO uno_results(match_id,table_id,completed_at,summary_json) VALUES ('old','EXPIRED',0,'{}')",[]).unwrap();
    app.db.execute("INSERT INTO uno_result_players(match_id,user_id,won) VALUES ('old','bob',0),('old','alice',1)",[]).unwrap();
    app.db
        .execute(
            "UPDATE users SET is_guest=1,last_active_at=datetime('now','-2 days') WHERE id='bob'",
            [],
        )
        .unwrap();
    app.db.cleanup_stale_guests().unwrap();
    assert!(app
        .db
        .query_one("SELECT 1 FROM users WHERE id='bob'", [], |_| Ok(()))
        .unwrap()
        .is_none());
    let (_, history) = req(&app, "alice", "GET", "/api/matches/history", json!({})).await;
    assert_eq!(history.as_array().unwrap().len(), 1);
    assert_eq!(
        req(&app, "alice", "GET", "/api/stats/alice", json!({}))
            .await
            .1["wins"],
        1
    );
}

#[tokio::test]
async fn no_contest_is_history_without_win_loss_or_leaderboard_credit() {
    let app = setup();
    app.db.execute("INSERT INTO uno_results(match_id,table_id,completed_at,summary_json) VALUES ('none','NOCOUNT',?,'{}')",[now()]).unwrap();
    app.db
        .execute(
            "INSERT INTO uno_result_players(match_id,user_id,won) VALUES ('none','alice',NULL)",
            [],
        )
        .unwrap();
    assert_eq!(
        req(&app, "alice", "GET", "/api/stats/alice", json!({}))
            .await
            .1["gamesPlayed"],
        0
    );
    for period in ["day", "week", "month", "all"] {
        let (_, ranking) = req(
            &app,
            "alice",
            "GET",
            &format!("/api/leaderboard?period={period}"),
            json!({}),
        )
        .await;
        assert_eq!(ranking["players"].as_array().unwrap().len(), 0);
    }
    assert_eq!(
        req(&app, "alice", "GET", "/api/matches/history", json!({}))
            .await
            .1
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn shared_friend_invites_and_reactions_are_scoped_and_idempotent() {
    let app = setup();
    let id = create_room(&app, 3).await;
    let url = format!("/api/lobbies/{id}");
    assert_eq!(
        req(
            &app,
            "alice",
            "POST",
            &format!("{url}/invite"),
            json!({"userId":"bob"})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    app.db.execute("INSERT INTO friendships(sender_id,receiver_id,status) VALUES ('alice','bob','accepted')",[]).unwrap();
    assert_eq!(
        req(
            &app,
            "alice",
            "POST",
            &format!("{url}/invite"),
            json!({"userId":"bob"})
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, invites) = req(&app, "bob", "GET", "/api/invitations", json!({})).await;
    assert_eq!(invites[0]["tableId"], id);
    assert_eq!(invites[0]["gameId"], "unogo");
    assert!(req(&app, "eve", "GET", "/api/invitations", json!({}))
        .await
        .1
        .as_array()
        .unwrap()
        .is_empty());
    req(&app, "bob", "POST", &format!("{url}/join"), json!({})).await;
    assert!(req(&app, "bob", "GET", "/api/invitations", json!({}))
        .await
        .1
        .as_array()
        .unwrap()
        .is_empty());
    let body = json!({"emoji":"👍","requestId":"retry-safe"});
    for _ in 0..2 {
        assert_eq!(
            req(
                &app,
                "alice",
                "POST",
                &format!("{url}/reactions"),
                body.clone()
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let (_, usage) = req(&app, "alice", "GET", "/api/reactions", json!({})).await;
    assert_eq!(usage["counts"]["👍"], 1);
    let shared = app.db.reaction_usage("alice").unwrap();
    assert_eq!(shared["👍"], 1);
    let (_, table) = req(&app, "bob", "GET", &url, json!({})).await;
    assert_eq!(table["reactions"].as_array().unwrap().len(), 1);
    assert_eq!(
        req(&app, "eve", "POST", &format!("{url}/reactions"), body)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}
