use super::*;

pub async fn run(api: &Api) -> Result<()> {
    let route = "/api/auth/reset-password";
    api.call(
        Method::POST,
        route,
        "",
        json!({"password":"new-password-123"}),
        401,
    )
    .await?;
    let (guest_token, _) = api.guest("passwordcheck").await?;
    api.call(
        Method::POST,
        route,
        &guest_token,
        json!({"password":"new-password-123"}),
        400,
    )
    .await?;
    let upgrade = api
        .post(
            "/api/auth/upgrade",
            &guest_token,
            json!({"password":"old-password-123"}),
        )
        .await?;
    let username = upgrade["user"]["username"].as_str().unwrap();
    let other = api
        .post(
            "/api/auth/login",
            "",
            json!({"username":username,"password":"old-password-123"}),
        )
        .await?;
    let other_token = other["token"].as_str().unwrap();
    let mut current_wire = Wire::connect(&api.base, &guest_token).await?;
    let mut revoked_wire = Wire::connect(&api.base, other_token).await?;
    for body in [
        json!({}),
        json!({"password":false}),
        json!({"password":"short"}),
        json!({"password":"x".repeat(1025)}),
    ] {
        api.call(Method::POST, route, &guest_token, body, 400)
            .await?;
    }
    api.call(Method::GET, "/api/auth/me", other_token, json!({}), 200)
        .await?;
    // A supplied target identity must never redirect the reset to another user.
    let (untouched_token, untouched_id) = api.guest("untouched").await?;
    let response = api
        .post(
            route,
            &guest_token,
            json!({"password":"new-password-456","userId":untouched_id}),
        )
        .await?;
    assert_eq!(response["success"], true);
    api.call(Method::GET, "/api/auth/me", &guest_token, json!({}), 200)
        .await?;
    api.call(Method::GET, "/api/auth/me", other_token, json!({}), 401)
        .await?;
    api.call(
        Method::GET,
        "/api/auth/me",
        &untouched_token,
        json!({}),
        200,
    )
    .await?;
    api.call(
        Method::POST,
        "/api/auth/login",
        "",
        json!({"username":username,"password":"old-password-123"}),
        401,
    )
    .await?;
    api.post(
        "/api/auth/login",
        "",
        json!({"username":username,"password":"new-password-456"}),
    )
    .await?;
    api.call(
        Method::POST,
        route,
        other_token,
        json!({"password":"hijacked-password"}),
        401,
    )
    .await?;
    assert!(Wire::connect(&api.base, other_token).await.is_err());
    let ack = current_wire.command("join_lobby", json!("INVALID")).await?;
    assert!(
        ack["error"].is_string(),
        "Current socket must still be connected and authenticated"
    );
    match timeout(
        Duration::from_secs(5),
        revoked_wire.command("join_lobby", json!("INVALID")),
    )
    .await?
    {
        Err(_) => {}
        Ok(ack) => assert!(
            ack["error"].is_string(),
            "Revoked socket remained authorized"
        ),
    }
    println!("PASS authenticated password reset without old password; validation, account isolation, old/new login, current session retention and other-session revocation");
    Ok(())
}
