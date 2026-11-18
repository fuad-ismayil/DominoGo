use crate::{browser, i18n::t, net, store::*, ui::*};
use dioxus::prelude::*;
use serde_json::json;
#[component]
pub fn Setup() -> Element {
    let mut count = use_signal(|| 4usize);
    let mut code = use_signal(String::new);
    let create = *SCREEN.read() == Screen::Create;
    rsx! {
        section { class: "setup-page menu-page",
            Back {}
            div { class: "setup-card glass",
                h1 {
                    if create {
                        {t("Set the table.")}
                    } else {
                        {t("You're invited.")}
                    }
                }
                p { class: "muted",
                    if create {
                        {
                            t(
                                "Your table, your people. Pick the number of seats, then invite friends or add a bot.",
                            )
                        }
                    } else {
                        {t("Ask your host for the seven-character table code and join the next round.")}
                    }
                }
                form {
                    onsubmit: move |e| {
                        e.prevent_default();
                        if *BUSY.read() {
                            return;
                        }
                        BUSY.signal().set(true);
                        let code = code();
                        let seats = count();
                        dioxus::dioxus_core::spawn_forever(async move {
                            if create {
                                match net::request(
                                        "POST",
                                        "/api/lobbies",
                                        Some(json!({ "maxPlayers" : seats })),
                                    )
                                    .await
                                {
                                    Ok(v) => {
                                        set_table(v);
                                        SCREEN.signal().set(Screen::Table);
                                    }
                                    Err(e) => error(e),
                                }
                            } else {
                                enter(&code).await;
                            }
                            BUSY.signal().set(false);
                        });
                    },
                    if create {
                        fieldset { class: "seat-picker uno-seat-picker",
                            legend { {t("How many players?")} }
                            for n in 2..=10 {
                                button {
                                    r#type: "button",
                                    class: if count() == n { "seat-option selected glass" } else { "seat-option glass" },
                                    aria_pressed: count() == n,
                                    onclick: move |_| count.set(n),
                                    strong { "{n}" }
                                }
                            }
                        }
                        div { class: "uno-fixed-rules",
                            strong { {t("Classic · Single round")} }
                            p {
                                {t("Seven cards each. Draw one or play one. No stacking. First empty hand wins.")}
                            }
                            small { {t("Fixed for this table. Cannot be changed during a match.")} }
                        }
                        div { class: "setup-note",
                            Icon { name: "bot" }
                            span { {t("Play solo with bots, with friends, or a little of both.")} }
                        }
                    } else {
                        label { r#for: "table-code", {t("Table code")} }
                        input {
                            id: "table-code",
                            class: "code-input",
                            required: true,
                            minlength: 7,
                            maxlength: 7,
                            pattern: "[A-Za-z0-9]{7}",
                            autocomplete: "off",
                            spellcheck: "false",
                            placeholder: "ABC1234",
                            value: "{code}",
                            oninput: move | e | code.set(e.value()
                                    .to_ascii_uppercase()),
                        }
                    }
                    button {
                        r#type: "submit",
                        class: "setup-submit",
                        disabled: *BUSY.read(),
                        if *BUSY.read() {
                            {t("Opening your table…")}
                        } else if create {
                            {t("Create table")}
                        } else {
                            {t("Join table")}
                        }
                        Icon { name: "arrow" }
                    }
                }
                small { class: "setup-footnote", {t("Private tables · Classic 108-card deck")} }
            }
        }
    }
}
#[component]
pub fn TableScreen() -> Element {
    let table = TABLE.read().clone();
    match table {
        Some(t) if t.game.is_some() => {
            rsx! {
                super::GameBoard {}
            }
        }
        Some(_) => {
            rsx! {
                Lobby {}
            }
        }
        None => {
            rsx! {
                Back {}
                p { {t("Table not found")} }
            }
        }
    }
}
#[component]
pub fn Lobby() -> Element {
    let mut copied = use_signal(|| false);
    let mut show_friends = use_signal(|| false);
    let mut sent = use_signal(Vec::<String>::new);
    use_future(move || async move {
        if USER.read().as_ref().is_some_and(|u| !u.is_guest) {
            refresh_friends().await;
        }
    });
    let friends = FRIENDS.read().clone();
    let Some(table) = TABLE.read().clone() else {
        return rsx! {};
    };
    let user = USER.read().clone().unwrap_or_default();
    let host = table.host_id == user.id;
    let link = browser::invite_url(&table.id);
    rsx! {
        section { class: "lobby-page menu-page",
            div { class: "lobby-heading",
                h1 { {t("Your table is ready.")} }
                p { class: "muted", { t("Classic · Single round")
                        } }
            }
            div { class: "lobby-columns",
                section { class: "lobby-seats glass",
                    div { class: "section-heading",
                        h2 { {t("Seats at the table")} }
                        span { class: "muted", "{table.players.len()} / {table.max_players}" }
                    }
                    for slot in 0..table.max_players {
                        {
                            let player = table.players.iter().find(|s| s.slot == slot).cloned();
                            rsx! {
                                div { class: if player.is_some() { "seat-row" } else { "seat-row empty-seat" },
                                    span { class: "seat-avatar glass-circle",
                                        if let Some(p) = &player {
                                            if p.user_id.is_none() {
                                                Icon { name: "bot" }
                                            } else {
                                                "{p.name.chars().next().unwrap_or('U').to_uppercase()}"
                                            }
                                        } else {
                                            Icon { name: "plus" }
                                        }
                                    }
                                    div {
                                        if let Some(p) = &player {
                                            strong { "{p.name}" }
                                            small {
                                                if p.user_id.as_deref() == Some(&table.host_id) {
                                                    {t("Host")}
                                                } else if p.user_id.is_none() {
                                                    {t("Bot player")}
                                                } else {
                                                    {t("Ready to play")}
                                                }
                                            }
                                        } else {
                                            strong { {t("Open seat")} }
                                            small { {t("Invite a friend or add a bot")} }
                                        }
                                    }
                                    if host && player.as_ref().is_some_and(|p| p.user_id.as_deref() != Some(&user.id)) {
                                        button {
                                            class: "round-control",
                                            aria_label: t("Remove player from table"),
                                            disabled: *BUSY.read(),
                                            onclick: {
                                                let id = table.id.clone();
                                                move |_| mutate("DELETE", format!("/api/lobbies/{id}/seats/{slot}"), json!({}))
                                            },
                                            Icon { name: "close" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if host {
                        div { class: "lobby-controls",
                            button {
                                class: "secondary glass-capsule",
                                disabled: * BUSY.read() || table.players.len() >= table
                                        .max_players,
                                onclick: {
                                    let id = table.id.clone();
                                    move |_| mutate("POST", format!("/api/lobbies/{id}/add-bot"), json!({}))
                                },
                                Icon { name: "bot" }
                                {t("Add a bot")}
                            }
                            button {
                                disabled: *BUSY.read() || table.players.len() < 2,
                                onclick: {
                                    let id = table.id.clone();
                                    move |_| {
                                        browser::unlock_sound();
                                        mutate("POST", format!("/api/lobbies/{id}/start"), json!({}));
                                    }
                                },
                                {t("Start match")}
                                Icon { name: "arrow" }
                            }
                        }
                    } else {
                        p { class: "waiting-message",
                            {t("Waiting for your host to start the match…")}
                        }
                    }
                }
                section { class: "lobby-sharing glass",
                    h2 { {t("Share your code")} }
                    p { class: "muted", {t("Your friends can enter this code from Join table.")} }
                    div { class: "share-code",
                        code { "{table.id}" }
                        button {
                            class: "round-control glass-circle",
                            aria_label: t("Copy table code"),
                            onclick: {
                                let id = table.id.clone();
                                move |_| {
                                    let id = id.clone();
                                    dioxus::dioxus_core::spawn_forever(async move {
                                        copied.set(browser::copy(&id).await);
                                    });
                                }
                            },
                            Icon { name: "copy" }
                        }
                    }
                    div { class: "share-invite-link",
                        h3 { {t("A seat in one tap.")} }
                        p {
                            {
                                t(
                                    "Send a link. Your friend joins the waiting room; the round starts when you're ready.",
                                )
                            }
                        }
                        label { r#for: "invite-link", {t("Invite link")} }
                        input {
                            id: "invite-link",
                            readonly: true,
                            value: "{link}",
                            onfocus: move | _ |
                                    browser::select("invite-link"),
                        }
                        button {
                            class: "secondary glass-capsule",
                            onclick: move |_| {
                                let link = link.clone();
                                dioxus::dioxus_core::spawn_forever(async move {
                                    let ok = browser::copy(&link).await;
                                    copied.set(ok);
                                    if !ok {
                                        browser::select("invite-link");
                                    }
                                });
                            },
                            Icon { name: "copy" }
                            {t("Copy invite link")}
                        }
                        small { class: "muted", role: "status",
                            if copied() {
                                {t("Copied. Send it to your friends.")}
                            } else {
                                {t("You can also select the link above to copy it.")}
                            }
                        }
                    }
                    if !user.is_guest {
                        button {class:"secondary glass-capsule",onclick:move |_|show_friends.set(!show_friends()),Icon{name:"people"}{t("Invite a friend")}}
                        if show_friends(){
                            div {class:"invite-list",
                                if friends["friends"].as_array().is_none_or(|rows|rows.is_empty()){p {class:"muted",{t("No friends to invite yet. Share the code above, or add friends from Home.")}}}
                                if let Some(rows)=friends["friends"].as_array(){for friend in rows {
                                    div {class:"friend-invite-row",span {strong {"{friend[\"username\"].as_str().unwrap_or_default()}"}}
                                        button {class:"secondary compact-button",disabled:*BUSY.read()||sent().iter().any(|id|Some(id.as_str())==friend["id"].as_str()),onclick:{let table_id=table.id.clone();let friend_id=friend["id"].as_str().unwrap_or_default().to_string();move |_|{
                                            let table_id=table_id.clone();let friend_id=friend_id.clone();BUSY.signal().set(true);
                                            dioxus::dioxus_core::spawn_forever(async move{match net::request("POST",&format!("/api/lobbies/{table_id}/invite"),Some(json!({"userId":friend_id}))).await{Ok(_)=>sent.write().push(friend_id),Err(e)=>error(e)}BUSY.signal().set(false);});
                                        }},{t("Invite")}}
                                    }
                                }}
                            }
                        }
                    }
                }
            }
            button {
                class: "leave-table text-button",
                disabled: *BUSY.read(),
                onclick: {
                    let id = table.id.clone();
                    move |_| {
                        if *BUSY.read() {
                            return;
                        }
                        let id = id.clone();
                        BUSY.signal().set(true);
                        dioxus::dioxus_core::spawn_forever(async move {
                            match net::request(
                                    "POST",
                                    &format!("/api/lobbies/{id}/leave"),
                                    Some(json!({})),
                                )
                                .await
                            {
                                Ok(_) => {
                                    TABLE.signal().set(None);
                                    browser::remove("unogo.table");
                                    SCREEN.signal().set(Screen::MainMenu);
                                }
                                Err(e) => error(e),
                            }
                            BUSY.signal().set(false);
                        });
                    }
                },
                Icon { name: "back" }
                {t("Leave table")}
            }
        }
    }
}
