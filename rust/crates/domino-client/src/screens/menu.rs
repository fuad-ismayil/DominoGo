use super::{score::ScoreReadout, ui::Icon};
use crate::i18n::{t, ta, tf};
use crate::{app::*, net, store::*};
use dioxus::prelude::*;

#[component]
pub fn Menu() -> Element {
    let history = MATCH_HISTORY.read().clone();
    let stats = PLAYER_STATS.read().clone();
    let session = SESSION.read().clone();
    let mut hiding = use_signal(|| None::<String>);
    let mut refreshing = use_signal(|| false);
    let mut countdown = use_signal(|| None::<u64>);
    use_future(move || async move {
        loop {
            if let Some(expires) = SESSION.read().guest_expires_at.clone() {
                let end = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(&expires)).get_time();
                let seconds = ((end - crate::browser::now_ms()) / 1000.0).max(0.0) as u64;
                countdown.set(Some(seconds));
            }
            crate::game::timers::sleep_ms(30_000).await;
        }
    });
    // Scoped to this screen: leaving the menu cancels the polling task.
    use_future(move || async move {
        loop {
            if !crate::browser::document().hidden() {
                refresh_menu().await;
            }
            crate::game::timers::sleep_ms(3_000).await;
        }
    });
    let requests = FRIENDS.read().incoming.len();
    rsx! { section { class: "club-home",
        div { class: "home-columns",
            section { class: "club-welcome glass",
                h1 { {t("Hello,")} br {} span { "{session.username}" } }
                p { class: "welcome-copy", {t("Good company. A little strategy. Your next round starts here.")} }
                if let Some(s) = stats.filter(|s| s.games_played > 0) { div { class: "club-stats", div { strong { "{s.wins}" } span { {t("Wins")} } } div { strong { "{s.losses}" } span { {t("Losses")} } } div { strong { "{s.win_rate}%" } span { {t("Win rate")} } } small { class:"stats-detail", {tf("{s.games_played} games · {s.avg_score} average points", &[("s.games_played", format!("{}", (s.games_played))), ("s.avg_score", format!("{}", (s.avg_score)))])} } } }
                div { class: "play-actions",
                    button { class: "host-action", onclick: move |_| nav(Screen::CreateLobby), span { class: "action-symbol", Icon { name: "plus" } } span { strong { {t("Create a table")} } small { {t("Bring friends or play the bots")} } } Icon { name: "arrow" } }
                    button { class: "join-action glass-capsule", onclick: move |_| nav(Screen::JoinLobby), Icon { name: "people" } span { {t("Join with a code")} } Icon { name: "arrow" } }
                }
                if session.is_guest { button { class: "save-account", onclick: move |_| nav(Screen::UpgradeAccount), Icon { name: "shield" } span { strong { {t("Make your seat permanent")} } small { {t("Save your progress with a password.")} } } Icon { name: "arrow" } } if let Some(seconds)=countdown() { small { class: "guest-countdown", if seconds==0 {{t("Guest session expires after inactivity")}} else {{ta("Guest access · {}h {:02}m remaining", &[ (seconds/3600).to_string(),format!("{:02}", seconds%3600/60)])}} } } }
                div { class: "club-shortcuts",
                    button { class: "shortcut glass", onclick: move |_| { nav(Screen::Friends); spawn(refresh_friends()); }, Icon { name: "people" } span { {t("Friends")} } if requests > 0 { span { class: "notification-dot", "{requests}" } } }
                    button { class: "shortcut glass", onclick: move |_| { nav(Screen::Leaderboard); }, Icon { name: "trophy" } span { {t("Leaderboard")} } }
                }
            }
            section { class: "history-panel glass",
                div { class: "section-heading", div {  h2 { {t("Your tables")} } } button { class: "round-control glass-circle", title: t("Refresh match history"), aria_label: t("Refresh match history"), disabled: refreshing(), onclick: move |_| { if refreshing() { return; } refreshing.set(true); spawn(async move { refresh_menu().await; refreshing.set(false); }); }, Icon { name: "refresh" } } }
                if history.is_empty() { div { class: "history-empty", div { class: "empty-orbit glass-circle", Icon { name: "history" } } h3 { {t("Every regular starts somewhere.")} } p { {t("Create your first table. Your matches and scores will appear here.")} } } }
                div { class: "history-list",
                    for row in history {
                        { let status = match row.status.as_str() { "playing" | "round_end" => {t("In progress")}, "finished" => {t("Finished")}, "stopped" => {t("Paused")}, "archived" => {t("Archived")}, _ => {t("Waiting for players")} }; let playable = row.status != "archived" && (row.status != "finished" || row.id.len() == 7); let names = row.players.iter().map(|p| player_label(&p.id,&p.username)).collect::<Vec<_>>().join(&format!(" {} ",t("vs"))); let date = row.updated_at.as_ref().or(row.created_at.as_ref()).map(|raw| js_sys::Date::new(&wasm_bindgen::JsValue::from_str(raw)).to_locale_string(crate::i18n::language().locale(), &wasm_bindgen::JsValue::UNDEFINED).as_string().unwrap_or_default()).unwrap_or_default();
                        rsx! { article { class: "match-card",
                            div { class: "match-heading", strong { "{names}" } button { class: "round-control", aria_label: t("Hide match from your history"), title: t("Hide from history"), onclick: { let id = row.id.clone(); move |_| hiding.set(Some(id.clone())) }, Icon { name: "close" } } }
                            div { class: "match-meta", span { class: if playable { "match-state live" } else { "match-state" }, "{status}" } time { "{date}" } }
                            if let Some(scores) = &row.scores { div { class: "history-scores", for p in &row.players { div { class: if row.winner.as_ref() == Some(&p.id) { "history-score winner" } else { "history-score" }, span { {player_label(&p.id,&p.username)} } ScoreReadout {score:scores.get(&p.id).copied().unwrap_or_default(),target:false} } } } }
                            if hiding().as_ref() == Some(&row.id) { div { class: "inline-confirm", span { {t("Hide this table?")} } button { class: "compact-button secondary", onclick: { let id = row.id.clone(); move |_| { let id=id.clone(); spawn(async move { if result(net::matches_delete(&id).await).await.is_some() { hiding.set(None); refresh_menu().await; } }); } }, {t("Hide")} } button { class: "compact-button text-button", onclick: move |_| hiding.set(None), {t("Cancel")} } } } else if playable { button { class: "resume-table glass-capsule", onclick: { let id=row.id.clone(); move |_| { spawn(enter_lobby(id.clone())); } }, if row.status == "finished" { {t("Match results")} } else if row.status == "waiting" { {t("Return to table")} } else { {t("Rejoin match")} } Icon { name: "arrow" } } }
                        } } }
                    }
                }
            }
        }
        div { class: "club-rules", crate::screens::rules::Rules {} }
    } }
}
