use crate::{i18n::t, net, store::*, ui::*};
use dioxus::prelude::*;
use serde_json::{json, Value};
fn social(path: String, body: Value) {
    if *BUSY.read() {
        return;
    }
    BUSY.signal().set(true);
    dioxus::dioxus_core::spawn_forever(async move {
        match net::request("POST", &path, Some(body)).await {
            Ok(_) => refresh_friends().await,
            Err(e) => error(e),
        }
        BUSY.signal().set(false);
    });
}
#[component]
pub fn Friends() -> Element {
    let mut name = use_signal(String::new);
    let mut loading = use_signal(|| true);
    use_future(move || async move {
        if USER.read().as_ref().is_some_and(|u| !u.is_guest) {
            refresh_friends().await;
        }
        loading.set(false);
    });
    if USER.read().as_ref().is_some_and(|u| u.is_guest) {
        return rsx! {
            Back {}
            section { class: "friends-panel glass",
                h1 { {t("Friends")} }
                p {
                    {
                        t(
                            "Save your account to add friends. Your existing DominoGo friends will be here too.",
                        )
                    }
                }
                button { onclick: move |_| nav(Screen::Upgrade), {t("Save my account")} }
            }
        };
    }
    let friends = FRIENDS.read().clone();
    rsx! {
        section { class: "menu-page",
            Back {}
            h1 { {t("Friends")} }
            div { class: "friends-columns",
                section { class: "friends-panel glass",
                    div { class: "section-heading",
                        h2 { {t("Your friends")} }
                        button {
                            class: "round-control glass-circle",
                            aria_label: t("Refresh friends"),
                            disabled: * BUSY
                                    .read(),
                            onclick: move |_| {
                                dioxus::dioxus_core::spawn_forever(refresh_friends());
                            },
                            Icon { name: "refresh" }
                        }
                    }
                    if loading() {
                        p { role: "status", {t("Loading…")} }
                    } else if friends["friends"].as_array().is_none_or(|f| f.is_empty()) {
                        p { class: "muted", {t("Add a friend by username, or share a table link.")} }
                    }
                    if let Some(rows) = friends["friends"].as_array() {
                        for f in rows {
                            div { class: "friend-row",
                                span { class: "seat-avatar glass-circle",
                                    "{f[\"username\"].as_str().unwrap_or_default().chars().next().unwrap_or('U').to_uppercase()}"
                                }
                                div {
                                    strong { "{f[\"username\"].as_str().unwrap_or_default()}" }
                                    small {
                                        if f["isOnline"] == true {
                                            {t("Online")}
                                        } else {
                                            {t("Offline")}
                                        }
                                    }
                                }
                                button {
                                    class: "text-button",
                                    disabled: *BUSY.read(),
                                    onclick: {
                                        let id = f["id"].as_str().unwrap_or_default().to_string();
                                        move |_| {
                                            let id = id.clone();
                                            BUSY.signal().set(true);
                                            dioxus::dioxus_core::spawn_forever(async move {
                                                match net::request(
                                                        "DELETE",
                                                        &format!("/api/friends/{id}"),
                                                        Some(json!({})),
                                                    )
                                                    .await
                                                {
                                                    Ok(_) => refresh_friends().await,
                                                    Err(e) => error(e),
                                                }
                                                BUSY.signal().set(false);
                                            });
                                        }
                                    },
                                    {t("Remove")}
                                }
                            }
                        }
                    }
                }
                div { class: "social-side",
                    section { class: "add-friend-panel glass",
                        h2 { {t("Add a friend")} }
                        form {
                            onsubmit: move |e| {
                                e.prevent_default();
                                social("/api/friends/request".into(), json!({ "username" : name().trim() }));
                            },
                            label { r#for: "friend-name", {t("Username")} }
                            input {
                                id: "friend-name",
                                required: true,
                                maxlength: 32,
                                value: "{name}",
                                oninput: move |e| name.set(e.value()),
                            }
                            button { r#type: "submit", disabled: * BUSY
                                        .read(), {t("Send request")} }
                        }
                    }
                    section { class: "requests-panel glass",
                        h2 { {t("Friend requests")} }
                        for (key, label, action) in [("incoming", "Incoming", "accept"), ("outgoing", "Sent", "cancel")] {
                            h3 { {t(label)} }
                            if let Some(rows) = friends[key].as_array() {
                                if rows.is_empty() {
                                    p { class: "muted", {t("No requests")} }
                                }
                                for f in rows {
                                    div { class: "request-row",
                                        strong { "{f[\"username\"].as_str().unwrap_or_default()}" }
                                        button {
                                            class: "secondary compact-button",
                                            disabled: *BUSY.read(),
                                            onclick: {
                                                let id = f["id"].clone();
                                                move |_| social(format!("/api/friends/{action}"), json!({ "friendshipId" : id }))
                                            },
                                            if action == "accept" {
                                                {t("Accept")}
                                            } else {
                                                {t("Cancel")}
                                            }
                                        }
                                        if action == "accept" {
                                            button {
                                                class: "text-button",
                                                disabled: *BUSY.read(),
                                                onclick: {
                                                    let id = f["id"].clone();
                                                    move |_| social("/api/friends/reject".into(), json!({ "friendshipId" : id }))
                                                },
                                                {t("Decline")}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
#[component]
pub fn Leaders() -> Element {
    use_future(move || async move {
        refresh_ranking().await;
    });
    let ranking = RANKING.read().clone();
    let period = PERIOD.read().clone();
    rsx! {
        section { class: "menu-page",
            Back {}
            div { class: "ranking-panel glass",
                div { class: "section-heading",
                    h1 { {t("Leaderboard")} }
                    button {
                        class: "round-control glass-circle",
                        aria_label: t("Refresh leaderboard"),
                        onclick: move |_| {
                            dioxus::dioxus_core::spawn_forever(refresh_ranking());
                        },
                        Icon { name: "refresh" }
                    }
                }
                p { class: "muted",
                    {t("A friendly tally of UNO wins. No points or skill rating. Periods use UTC.")}
                }
                div {
                    class: "period-tabs",
                    role: "group",
                    aria_label: t("Leaderboard period"),
                    for (key, label) in [("day", "Day"), ("week", "Week"), ("month", "Month"), ("all", "All time")] {
                        button {
                            class: if period == key { "selected" } else { "secondary" },
                            aria_pressed: period == key,
                            onclick: move |
                                    _ | { PERIOD.signal().set(key.into()); RANKING.signal().set(None);
                                PERIOD.signal().set(key.into());
                                RANKING.signal().set(None);
                                dioxus::dioxus_core::spawn_forever(refresh_ranking());
                            },
                            {t(label)}
                        }
                    }
                }
                if let Some(ranking) = ranking {
                    if let Some(rows) = ranking["players"].as_array() {
                        if rows.is_empty() {
                            div { class: "history-empty",
                                Icon { name: "trophy" }
                                h2 { {t("The next win could be yours.")} }
                                p {
                                    {t("Completed games appear here. Bots and unfinished rounds don't count.")}
                                }
                            }
                        } else {
                            div { class: "ranking-table-wrap",
                                table { class: "ranking-table",
                                    thead {
                                        tr {
                                            th { {t("Player")} }
                                            th { {t("Wins")} }
                                            th { {t("Win rate")} }
                                            th { {t("Games")} }
                                        }
                                    }
                                    tbody {
                                            for (i, row) in rows.iter().enumerate() {
                                                tr {
                                                    td {
                                                        span { class: "rank-number",
                                                            "{i+1}"
                                                        }
                                                        "{row[\"username\"].as_str().unwrap_or_default()}"
                                                    }
                                                    td { "{row[\"wins\"]}" }
                                                    td { "{row[\"winRate\"]}%" }
                                                    td { "{row[\"gamesPlayed\"]}" }
                                                }
                                            }
                                        }
                                }
                            }
                        }
                    }
                } else {
                    p { class: "muted", role: "status", {t("Loading…")} }
                }
            }
        }
    }
}
