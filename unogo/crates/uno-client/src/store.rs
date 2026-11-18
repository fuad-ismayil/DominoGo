use crate::{
    browser,
    i18n::t,
    model::{Table, User},
    net,
};
use dioxus::prelude::*;
use serde_json::{json, Value};
use uno_core::Action;
#[derive(Clone, Copy, PartialEq)]
pub enum Screen {
    Entry,
    Login,
    Register,
    Upgrade,
    MainMenu,
    Create,
    Join,
    Table,
    Friends,
    Leaders,
    Review,
}
pub static SCREEN: GlobalSignal<Screen> = Signal::global(|| Screen::Entry);
pub static USER: GlobalSignal<Option<User>> = Signal::global(|| None);
pub static TABLE: GlobalSignal<Option<Table>> = Signal::global(|| None);
pub static HISTORY: GlobalSignal<Vec<Value>> = Signal::global(Vec::new);
pub static STATS: GlobalSignal<Value> = Signal::global(|| json!({}));
pub static FRIENDS: GlobalSignal<Value> = Signal::global(|| json!({}));
pub static RANKING: GlobalSignal<Option<Value>> = Signal::global(|| None);
pub static PERIOD: GlobalSignal<String> = Signal::global(|| "all".into());
pub static REVIEW: GlobalSignal<Value> = Signal::global(|| json!({}));
pub static LOADING: GlobalSignal<bool> = Signal::global(|| true);
pub static BUSY: GlobalSignal<bool> = Signal::global(|| false);
pub static CONNECTED: GlobalSignal<bool> = Signal::global(|| false);
pub static ERROR: GlobalSignal<String> = Signal::global(String::new);
pub static SUCCESS: GlobalSignal<String> = Signal::global(String::new);
pub static ACCOUNT: GlobalSignal<bool> = Signal::global(|| false);
pub static ARMED: GlobalSignal<bool> = Signal::global(|| false);
pub static EPOCH: GlobalSignal<u64> = Signal::global(|| 0);
pub static RECEIPT: GlobalSignal<Option<Value>> = Signal::global(|| None);
pub static CLOCK_OFFSET: GlobalSignal<i64> = Signal::global(|| 0);
pub static INVITATIONS: GlobalSignal<Vec<Value>> = Signal::global(Vec::new);
pub static REACTION_USAGE: GlobalSignal<Value> = Signal::global(|| json!({"order":[],"counts":{}}));
pub static REACTING: GlobalSignal<bool> = Signal::global(|| false);

pub async fn refresh_invitations() {
    let user = USER.read().as_ref().map(|u| u.id.clone());
    if let Ok(v) = net::request("GET", "/api/invitations", None).await {
        if USER.read().as_ref().map(|u| u.id.clone()) == user {
            INVITATIONS
                .signal()
                .set(v.as_array().cloned().unwrap_or_default());
        }
    }
}
pub async fn refresh_reactions() {
    if let Ok(v) = net::request("GET", "/api/reactions", None).await {
        REACTION_USAGE.signal().set(v);
    }
}
pub fn react(emoji: String) {
    if *REACTING.read() || !*CONNECTED.read() {
        return;
    }
    let Some(table) = TABLE.read().clone() else {
        return;
    };
    REACTING.signal().set(true);
    let token = browser::saved("unogo.token");
    let request_id = format!(
        "{}-{}",
        browser::now(),
        (js_sys::Math::random() * u32::MAX as f64) as u32
    );
    dioxus::dioxus_core::spawn_forever(async move {
        match net::request(
            "POST",
            &format!("/api/lobbies/{}/reactions", table.id),
            Some(json!({"requestId":request_id,"emoji":emoji})),
        )
        .await
        {
            Ok(v) => {
                if token == browser::saved("unogo.token") {
                    REACTION_USAGE.write()["counts"] = v["counts"].clone();
                }
            }
            Err(e) => error(e),
        }
        REACTING.signal().set(false);
    });
}
pub fn nav(screen: Screen) {
    if *BUSY.read() {
        return;
    }
    *EPOCH.write() += 1;
    if screen != Screen::Review {
        browser::remove("unogo.review");
    }
    if screen != Screen::Table {
        browser::remove("unogo.table");
    }
    SCREEN.signal().set(screen);
    ERROR.signal().set(String::new());
    SUCCESS.signal().set(String::new());
}
pub fn error(e: net::Error) {
    if e.status == 401 {
        clear_session();
        ERROR
            .signal()
            .set(t("Your session expired. Sign in again."));
        return;
    }
    if e.status == 0 || e.code == "GAME_OFFLINE" {
        CONNECTED.signal().set(false);
    }
    let message = match e.code.as_str() {
        "TABLE_FULL" => "This table is full",
        "TABLE_STARTED" => "This table has already started",
        "WRONG_GAME" => "This table belongs to DominoGo. Open DominoGo to join.",
        "TABLE_NOT_FOUND" => "Table not found",
        "STALE_MATCH" => "The match changed. Your table has been refreshed.",
        "NOT_ALLOWED" => "This action is unavailable to this player",
        _ => &e.message,
    };
    ERROR.signal().set(t(message));
}
pub fn clear_session() {
    *EPOCH.write() += 1;
    browser::remove("unogo.token");
    browser::remove("unogo.table");
    browser::remove("unogo.review");
    REVIEW.signal().set(json!({}));
    USER.signal().set(None);
    TABLE.signal().set(None);
    RECEIPT.signal().set(None);
    HISTORY.signal().set(vec![]);
    FRIENDS.signal().set(json!({}));
    INVITATIONS.signal().set(vec![]);
    REACTION_USAGE.signal().set(json!({"order":[],"counts":{}}));
    ACCOUNT.signal().set(false);
    SCREEN.signal().set(Screen::Entry);
    CONNECTED.signal().set(false);
}
pub fn accept_auth(v: &Value) {
    if let Some(token) = v["token"].as_str() {
        browser::save("unogo.token", token);
    }
    if let Ok(user) = serde_json::from_value::<User>(v["user"].clone()) {
        USER.signal().set(Some(user));
        CONNECTED.signal().set(true);
    }
}
pub fn set_table(v: Value) {
    match serde_json::from_value::<Table>(v) {
        Ok(table) => {
            if let Some(now) = table.server_now {
                CLOCK_OFFSET
                    .signal()
                    .set(browser::now() as i64 - now as i64);
            }
            let changed = TABLE
                .read()
                .as_ref()
                .is_none_or(|old| old.id != table.id || old.match_id != table.match_id);
            if changed {
                ARMED.signal().set(false);
                RECEIPT.signal().set(None);
            }
            let sound = TABLE
                .read()
                .as_ref()
                .and_then(|old| old.game.as_ref())
                .zip(table.game.as_ref())
                .is_some_and(|(a, b)| a.top.id != b.top.id);
            if sound {
                browser::card_sound();
            }
            crate::motion::snapshot(TABLE.read().as_ref(), &table);
            browser::save("unogo.table", &table.id);
            TABLE.signal().set(Some(table));
            CONNECTED.signal().set(true);
            if *ERROR.read() == t("Connection lost. Reconnecting…") {
                ERROR.signal().set(String::new());
            }
            if *ERROR.read() == t("This game is stopped on the host.") {
                ERROR.signal().set(String::new());
            }
        }
        Err(_) => ERROR
            .signal()
            .set(t("Could not read the saved round. Try again.")),
    }
}
pub async fn enter(id: &str) {
    browser::remove("unogo.review");
    match net::request(
        "POST",
        &format!("/api/lobbies/{}/join", id.trim().to_ascii_uppercase()),
        Some(json!({})),
    )
    .await
    {
        Ok(v) => {
            set_table(v);
            SCREEN.signal().set(Screen::Table);
        }
        Err(e) => error(e),
    }
}
pub fn open_review(row: &Value) {
    if let Some(id) = row["matchId"].as_str() {
        browser::save("unogo.review", id);
    }
    REVIEW.signal().set(row.clone());
    nav(Screen::Review);
}
pub async fn restore_review(id: &str) {
    match net::request("GET", "/api/matches/history", None).await {
        Ok(rows) => {
            if let Some(row) = rows
                .as_array()
                .and_then(|rows| rows.iter().find(|row| row["matchId"].as_str() == Some(id)))
            {
                open_review(row);
            } else {
                browser::remove("unogo.review");
            }
        }
        Err(e) => error(e), // Keep the saved destination for the next retry.
    }
}
pub async fn restore_table(id: &str) {
    // Restoring an existing seat is read-only; joining is reserved for invitations.
    match net::request("GET", &format!("/api/lobbies/{id}"), None).await {
        Ok(v) => {
            set_table(v);
            SCREEN.signal().set(Screen::Table);
        }
        Err(e) => {
            if matches!(e.status, 403 | 404) {
                browser::remove("unogo.table");
            }
            error(e);
        }
    }
}
pub async fn refresh_home() {
    let Some(user) = USER.read().clone() else {
        return;
    };
    if let Ok(v) = net::request("GET", "/api/matches/history", None).await {
        if USER.read().as_ref().is_some_and(|u| u.id == user.id) {
            HISTORY
                .signal()
                .set(v.as_array().cloned().unwrap_or_default());
        }
    }
    if let Ok(v) = net::request("GET", &format!("/api/stats/{}", user.id), None).await {
        if USER.read().as_ref().is_some_and(|u| u.id == user.id) {
            STATS.signal().set(v);
        }
    }
}
pub async fn refresh_friends() {
    let epoch = *EPOCH.read();
    match net::request("GET", "/api/friends", None).await {
        Ok(v) => {
            if epoch == *EPOCH.read() {
                FRIENDS.signal().set(v);
            }
        }
        Err(e) => {
            if epoch == *EPOCH.read() {
                error(e);
            }
        }
    }
}
pub async fn refresh_ranking() {
    let period = PERIOD.read().clone();
    match net::request("GET", &format!("/api/leaderboard?period={period}"), None).await {
        Ok(v) => {
            if period == *PERIOD.read() {
                RANKING.signal().set(Some(v));
            }
        }
        Err(e) => error(e),
    }
}
pub fn mutate(method: &str, path: String, body: Value) {
    if *BUSY.read() {
        return;
    }
    BUSY.signal().set(true);
    ERROR.signal().set(String::new());
    *EPOCH.write() += 1;
    let method = method.to_owned();
    let token = browser::saved("unogo.token");
    dioxus::dioxus_core::spawn_forever(async move {
        let response = net::request(&method, &path, Some(body)).await;
        if browser::saved("unogo.token") != token {
            BUSY.signal().set(false);
            return;
        }
        match response {
            Ok(v) => {
                if v.get("gameId").is_some() && v.get("players").is_some() {
                    set_table(v);
                    SCREEN.signal().set(Screen::Table);
                } else {
                    SUCCESS.signal().set(t("Saved."));
                }
            }
            Err(e) => error(e),
        }
        BUSY.signal().set(false);
    });
}
pub fn room_control(operation: &str) {
    let Some(table) = TABLE.read().clone() else {
        return;
    };
    mutate(
        "POST",
        format!("/api/lobbies/{}/{operation}", table.id),
        json!({ "matchId" : table.match_id }),
    );
}
pub fn command(action: Action) {
    if *BUSY.read() || !*CONNECTED.read() {
        return;
    }
    let Some(table) = TABLE.read().clone() else {
        return;
    };
    BUSY.signal().set(true);
    ERROR.signal().set(String::new());
    *EPOCH.write() += 1;
    let epoch = *EPOCH.read();
    let token = browser::saved("unogo.token");
    let touch = matches!(action, Action::TouchFive { .. });
    browser::unlock_sound();
    dioxus::dioxus_core::spawn_forever(async move {
        let path = format!("/api/lobbies/{}/commands", table.id);
        let body = json!(
            { "matchId" : table.match_id, "sequence" : table.accepted_sequence + 1,
            "action" : action }
        );
        let response = net::request("POST", &path, Some(body)).await;
        if epoch != *EPOCH.read() || browser::saved("unogo.token") != token {
            BUSY.signal().set(false);
            return;
        }
        match response {
            Ok(v) => {
                if v["receipt"]["challenge_evidence"].is_array() {
                    RECEIPT.signal().set(Some(v["receipt"].clone()));
                }
                set_table(v["table"].clone());
                ARMED.signal().set(false);
            }
            Err(e) => {
                // A tap can arrive just after the shared challenge resolves.
                // Refresh its authoritative outcome instead of flashing a stale-action error.
                if !touch || e.status != 409 {
                    error(e);
                }
                if USER.read().is_some() {
                    if let Ok(v) =
                        net::request("GET", &format!("/api/lobbies/{}", table.id), None).await
                    {
                        if epoch == *EPOCH.read() {
                            set_table(v);
                        }
                    }
                }
            }
        }
        BUSY.signal().set(false);
    });
}
