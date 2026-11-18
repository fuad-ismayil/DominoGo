use super::*;

pub async fn run(api: &Api) -> Result<()> {
    let (ht, host) = api.guest("emotehost").await?;
    let (pt, peer) = api.guest("emotepeer").await?;
    let table = api
        .post("/api/lobbies", &ht, json!({"maxPlayers":2}))
        .await?;
    let id = table["id"].as_str().unwrap();
    api.post(&format!("/api/lobbies/{id}/join"), &pt, json!({}))
        .await?;
    let mut h = Wire::connect(&api.base, &ht).await?;
    let mut p = Wire::connect(&api.base, &pt).await?;
    assert_eq!(h.usage().await?, json!({"userId":host,"counts":{}}));
    assert_eq!(p.usage().await?, json!({"userId":peer,"counts":{}}));
    h.command("join_lobby", json!(id)).await?;
    p.command("join_lobby", json!(id)).await?;
    for count in 1..=3 {
        assert_eq!(
            h.command("send_reaction", json!({"lobbyId":id,"emoji":"😭"}))
                .await?["success"],
            true
        );
        assert_eq!(
            h.usage().await?,
            json!({"userId":host,"counts":{"😭":count}})
        );
    }
    assert_eq!(
        p.command("send_reaction", json!({"lobbyId":id,"emoji":"😂"}))
            .await?["success"],
        true
    );
    assert_eq!(p.usage().await?, json!({"userId":peer,"counts":{"😂":1}}));
    let mut second = Wire::connect(&api.base, &ht).await?;
    assert_eq!(second.usage().await?["counts"], json!({"😭":3}));
    for count in 1..=4 {
        assert_eq!(
            second
                .command("send_reaction", json!({"lobbyId":id,"emoji":"👍"}))
                .await?["success"],
            true
        );
        let expected = json!({"userId":host,"counts":{"😭":3,"👍":count}});
        assert_eq!(second.usage().await?, expected);
        assert_eq!(h.usage().await?, expected);
    }
    assert!(h
        .command("send_reaction", json!({"lobbyId":id,"emoji":"invalid"}))
        .await?["error"]
        .is_string());
    assert!(h
        .command("send_reaction", json!({"lobbyId":"missing","emoji":"😭"}))
        .await?["error"]
        .is_string());
    // The client cannot write another account's counts or forge a frequency.
    assert_eq!(
        h.command(
            "send_reaction",
            json!({"lobbyId":id,"emoji":"😭","userId":peer,"count":500})
        )
        .await?["success"],
        true
    );
    assert_eq!(h.usage().await?["counts"], json!({"😭":4,"👍":4}));
    let mut restored = Wire::connect(&api.base, &ht).await?;
    assert_eq!(
        restored.usage().await?,
        json!({"userId":host,"counts":{"😭":4,"👍":4}})
    );
    let mut other = Wire::connect(&api.base, &pt).await?;
    assert_eq!(
        other.usage().await?,
        json!({"userId":peer,"counts":{"😂":1}})
    );
    println!("PASS emote usage: per-account counts, successful sends only, two-tab updates, reconnect persistence and rejected forged counts");
    Ok(())
}
