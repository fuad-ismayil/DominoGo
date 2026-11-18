use crate::{i18n::t, net, store::*, ui::*};
use dioxus::prelude::*;
use serde_json::{json, Value};
fn rules_label(row: &Value) -> String {
    let version = row["rulesVersion"]
        .as_str()
        .or_else(|| row["game"]["rules_version"].as_str())
        .unwrap_or("classic-108-single-v1");
    t(if version == uno_core::RULES_VERSION {
        "Friends · Single round"
    } else {
        "Classic · Single round"
    })
}
pub fn names(row: &Value) -> String {
    row["players"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| p["name"].as_str())
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .unwrap_or_default()
}
#[component]
pub fn Home() -> Element {
    let mut loading = use_signal(|| true);
    let mut hiding = use_signal(|| None::<String>);
    use_future(move || async move {
        refresh_home().await;
        loading.set(false);
    });
    let user = USER.read().clone().unwrap_or_default();
    let history = HISTORY.read().clone();
    let stats = STATS.read().clone();
    rsx! {
        section { class: "club-home",
            div { class: "home-columns",
                section { class: "club-welcome",
                    h1 {
                        {t("Hello,")}
                        br {}
                        span { "{user.username}" }
                    }
                    p { class: "welcome-copy",
                        {
                            t(
                                "Good friends, a little strategy, and a hand full of possibilities. Your next round starts here.",
                            )
                        }
                    }
                    if stats["gamesPlayed"].as_u64().unwrap_or(0) > 0 {
                        div { class: "club-stats",
                            for (key, label) in [("wins", "Wins"), ("losses", "Losses"), ("winRate", "Win rate")] {
                                div {
                                    strong {
                                        "{stats[key]}"
                                        if key == "winRate" {
                                            "%"
                                        }
                                    }
                                    span { {t(label)} }
                                }
                            }
                        }
                    }
                    div { class: "play-actions",
                        button {
                            class: "host-action", disabled: *BUSY.read(),
                            onclick: move |_| nav(Screen::Create),
                            span { class: "action-symbol",
                                Icon { name: "plus" }
                            }
                            span {
                                strong { {t("Create a table")} }
                                small { {t("Bring friends or play the bots")} }
                            }
                            Icon { name: "arrow" }
                        }
                        button {
                            class: "join-action glass-capsule", disabled: *BUSY.read(),
                            onclick: move | _ |
                                    nav(Screen::Join),
                            Icon { name: "people" }
                            span { {t("Join with a code")} }
                            Icon { name: "arrow" }
                        }
                    }
                    if user.is_guest {
                        button {
                            class: "save-account",
                            onclick: move |_| nav(Screen::Upgrade),
                            Icon { name: "user" }
                            span {
                                strong { {t("Make your seat permanent")} }
                                small { {t("Save your progress with a password.")} }
                            }
                            Icon { name: "arrow" }
                        }
                        small { class: "guest-countdown",
                            {t("Guest access expires after 24 hours of inactivity.")}
                        }
                    }
                    div { class: "club-shortcuts",
                        button {
                            class: "shortcut",
                            onclick: move | _ |
                                    nav(Screen::Friends),
                            Icon { name: "people" }
                            {t("Friends")}
                        }
                        button {
                            class: "shortcut",
                            onclick: move |_| nav(Screen::Leaders),
                            Icon { name: "trophy" }
                            {t("Leaderboard")}
                        }
                    }
                }
                section { class: "history-panel glass",
                    div { class: "section-heading",
                        h2 { {t("Your tables")} }
                        button {
                            class: "round-control glass-circle",
                            aria_label: t("Refresh tables"),
                            disabled: loading(),
                            onclick: move |_| {
                                loading.set(true);
                                dioxus::dioxus_core::spawn_forever(async move {
                                    refresh_home().await;
                                    loading.set(false);
                                });
                            },
                            Icon { name: "refresh" }
                        }
                    }
                    if loading() {
                        p { role: "status", class: "muted", {t("Loading your tables…")} }
                    } else if history.is_empty() {
                        div { class: "history-empty",
                            div { class: "empty-orbit glass-circle",
                                Icon { name: "history" }
                            }
                            h3 { {t("Every good game starts with a table.")} }
                            p { {t("Create one or join a friend. Your rounds will appear here.")} }
                        }
                    }
                    div { class: "history-list",
                        for row in history {
                            {
                                let active = row.get("status").is_some();
                                let id = if active { row["id"].as_str() } else { row["matchId"].as_str() }
                                    .unwrap_or_default()
                                    .to_owned();
                                let status = match row["status"].as_str() {
                                    Some("waiting") => "Waiting for players",
                                    Some("paused") => "Paused",
                                    Some("playing") => "In progress",
                                    _ => "Finished",
                                };
                                rsx! {
                                    article { class: "match-card",
                                        div { class: "match-heading",
                                            strong { "{names(&row)}" }
                                            if !active {
                                                button {
                                                    class: "round-control",
                                                    aria_label: t("Hide from history"),
                                                    onclick: {
                                                        let id = id.clone();
                                                        move |_| hiding.set(Some(id.clone()))
                                                    },
                                                    Icon { name: "close" }
                                                }
                                            }
                                        }
                                        div { class: "match-meta",
                                            span { class: "match-state", "{t(status)}" }
                                            span { {rules_label(&row)} }
                                        }
                                        if hiding().as_ref() == Some(&id) {
                                            div { class: "inline-confirm",
                                                span { {t("Hide this table?")} }
                                                button {
                                                    class: "compact-button secondary",
                                                    onclick: {
                                                        let id = id.clone();
                                                        move |_| {
                                                            let id = id.clone();
                                                            dioxus::dioxus_core::spawn_forever(async move {
                                                                match net::request(
                                                                        "DELETE",
                                                                        &format!("/api/matches/{id}"),
                                                                        Some(json!({})),
                                                                    )
                                                                    .await
                                                                {
                                                                    Ok(_) => {
                                                                        hiding.set(None);
                                                                        refresh_home().await;
                                                                    }
                                                                    Err(e) => error(e),
                                                                }
                                                            });
                                                        }
                                                    },
                                                    {t("Hide")}
                                                }
                                                button { class: "text-button", onclick: move | _ | hiding
                                                            .set(None), {t("Cancel")} }
                                            }
                                        } else {
                                            button {
                                                class: "resume-table glass-capsule",
                                                disabled: *BUSY.read(),
                                                onclick: {
                                                    let row = row.clone();
                                                    let id = id.clone();
                                                    move |_| {
                                                        if active {
                                                            BUSY.signal().set(true);
                                                            let id = id.clone();
                                                            dioxus::dioxus_core::spawn_forever(async move {
                                                                enter(&id).await;
                                                                BUSY.signal().set(false);
                                                            });
                                                        } else {
                                                            open_review(&row);
                                                        }
                                                    }
                                                },
                                                if active {
                                                    {t("Return to table")}
                                                } else {
                                                    {t("Match results")}
                                                }
                                                Icon { name: "arrow" }
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
pub fn Review() -> Element {
    let summary = REVIEW.read().clone();
    let winner = summary["winner"]
        .as_u64()
        .and_then(|i| summary["players"][i as usize]["name"].as_str())
        .map(str::to_owned);
    rsx! {
        section { class: "menu-page",
            Back {}
            div { class: "results-plane glass",
                h1 {
                    if let Some(name) = winner {
                        "{name} · {t(\"wins!\")}"
                    } else {
                        {t("No contest")}
                    }
                }
                p { class: "muted", {rules_label(&summary)} }
                ResultRows { summary: summary.clone() }
                if let Ok(card) = serde_json::from_value::<
                    uno_core::Card,
                >(summary["finishingCard"].clone())
                {
                    div { class: "finishing-card",
                        CardFace { card }
                    }
                }
                button {
                    class: "secondary",
                    onclick: move |_| nav(Screen::MainMenu),
                    {t("Back home")}
                }
            }
        }
    }
}
#[component]
pub fn ResultRows(summary: Value) -> Element {
    rsx! {
        div { class: "result-rows",
            if let Some(players) = summary["players"].as_array() {
                for (i, p) in players.iter().enumerate() {
                    div { class: "result-row",
                        span { "{p[\"name\"].as_str().unwrap_or_default()}" }
                        strong { "{summary[\"counts\"][i]} {t(\"cards\")}" }
                    }
                }
            }
        }
    }
}
