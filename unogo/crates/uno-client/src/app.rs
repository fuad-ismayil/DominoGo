use crate::{
    browser,
    i18n::{self, t},
    net,
    screens::*,
    store::*,
    ui::*,
};
use dioxus::prelude::*;
use serde_json::json;
#[component]
pub fn App() -> Element {
    use_hook(|| {
        i18n::init();
        dioxus::dioxus_core::spawn_forever(async {
            let invite = crate::model::invite_code(
                &browser::window().location().search().unwrap_or_default(),
            );
            browser::clean_query();
            if browser::saved("unogo.token").is_some() {
                match net::request("GET", "/api/auth/me", None).await {
                    Ok(v) => accept_auth(&v),
                    Err(e) => {
                        if e.status == 401 {
                            clear_session();
                        } else {
                            error(e);
                        }
                    }
                }
            }
            if matches!(&invite, Ok(Some(_))) && USER.read().is_none() {
                match net::request(
                    "POST",
                    "/api/auth/guest",
                    Some(json!({ "prefix" : "Guest" })),
                )
                .await
                {
                    Ok(v) => accept_auth(&v),
                    Err(e) => error(e),
                }
            }
            if USER.read().is_some() {
                SCREEN.signal().set(Screen::MainMenu);
                match invite {
                    Ok(Some(id)) => enter(&id).await,
                    Err(message) => ERROR.signal().set(t(message)),
                    _ => {
                        if let Some(id) = browser::saved("unogo.review") {
                            restore_review(&id).await;
                        } else if let Some(id) = browser::saved("unogo.table") {
                            restore_table(&id).await;
                        }
                    }
                }
                refresh_home().await;
            } else if let Err(message) = invite {
                ERROR.signal().set(t(message));
            }
            LOADING.signal().set(false);
        });
    });
    use_future(move || async move {
        let mut cycle = 0;
        loop {
            let reaction = TABLE
                .read()
                .as_ref()
                .and_then(|t| t.game.as_ref())
                .is_some_and(|g| matches!(g.phase, uno_core::Phase::TouchFive { .. }));
            browser::sleep(if reaction { 80 } else { 300 }).await;
            cycle += 1;
            if *BUSY.read() || *LOADING.read() || USER.read().is_none() {
                continue;
            }
            let epoch = *EPOCH.read();
            let token = browser::saved("unogo.token");
            if cycle % 8 == 0 && !reaction {
                refresh_invitations().await;
            }
            if cycle % 20 == 0 && !reaction {
                if let Err(e) = net::request("GET", "/api/auth/me", None).await {
                    if epoch == *EPOCH.read() && token == browser::saved("unogo.token") {
                        error(e);
                    }
                }
            }
            if *SCREEN.read() == Screen::Table {
                let id = TABLE.read().as_ref().map(|t| t.id.clone());
                if let Some(id) = id {
                    match net::request("GET", &format!("/api/lobbies/{id}"), None).await {
                        Ok(v) => {
                            if epoch == *EPOCH.read()
                                && !*BUSY.read()
                                && token == browser::saved("unogo.token")
                            {
                                set_table(v);
                            }
                        }
                        Err(e) => {
                            if epoch == *EPOCH.read() && !*BUSY.read() {
                                if [403, 404].contains(&e.status) {
                                    TABLE.signal().set(None);
                                    browser::remove("unogo.table");
                                    SCREEN.signal().set(Screen::MainMenu);
                                }
                                error(e);
                            }
                        }
                    }
                }
            } else if cycle % 8 == 0 {
                match *SCREEN.read() {
                    Screen::MainMenu => refresh_home().await,
                    Screen::Friends => refresh_friends().await,
                    _ => {}
                }
            }
        }
    });
    let screen = *SCREEN.read();
    let user = USER.read().clone();
    let game = screen == Screen::Table && TABLE.read().as_ref().is_some_and(|t| t.game.is_some());
    rsx! {
        style { { include_str!("../../../../rust/crates/domino-client/assets/index.css")
                } }
        style { {include_str!("../../../../rust/crates/domino-client/assets/menus.css")} }
        style { { include_str!("../../../../rust/crates/domino-client/assets/club-system.css")
                } }
        style { {include_str!("../assets/unogo.css")} }
        div { class: if game { "app-shell menu-shell uno-shell in-game" } else { "app-shell menu-shell uno-shell" },
            header { class: "topbar club-topbar",
                button {
                    class: "brand-button",
                    aria_label: t("UnoGo home"),
                    disabled: *BUSY.read(),
                    onclick: move |_| {
                        if USER.read().is_some() {
                            nav(Screen::MainMenu);
                        }
                    },
                    Brand {}
                }
                div { class: "topbar-account",
                    if let Some(user) = &user {
                        span { "{user.username}" }
                        button {
                            class: "profile-button glass-circle",
                            aria_label: t("Account"),
                            onclick: move |_| ACCOUNT.signal().set(true),
                            "{user.username.chars().next().unwrap_or('U').to_uppercase()}"
                        }
                    } else {
                        LanguagePicker {}
                    }
                }
            }
            if !ERROR.read().is_empty() {
                div { class: "notice uno-error", role: "alert",
                    span { "{ERROR}" }
                    button {
                        class: "icon-button",
                        aria_label: t("Dismiss"),
                        onclick: move | _ | ERROR.signal()
                                .set(String::new()),
                        Icon { name: "close" }
                    }
                }
            }
            if *LOADING.read() {
                main { class: "loading-state", role: "status",
                    Icon { name: "cards" }
                    {t("Opening your table…")}
                }
            } else {
                main {
                    match screen {
                        Screen::Entry | Screen::Login | Screen::Register | Screen::Upgrade => {
                            rsx! {
                                Auth {}
                            }
                        }
                        Screen::MainMenu => rsx! {
                            Home {}
                        },
                        Screen::Create | Screen::Join => rsx! {
                            Setup {}
                        },
                        Screen::Table => rsx! {
                            TableScreen {}
                        },
                        Screen::Friends => rsx! {
                            Friends {}
                        },
                        Screen::Leaders => rsx! {
                            Leaders {}
                        },
                        Screen::Review => rsx! {
                            Review {}
                        },
                    }
                }
            }
            if user.is_some() && !game && screen != Screen::Table {
                nav {
                    class: "club-navigation glass-capsule",
                    aria_label: t("Navigation"),
                    for (target, label, icon) in [
                        (Screen::MainMenu, "Home", "home"),
                        (Screen::Friends, "Friends", "people"),
                        (Screen::Leaders, "Leaderboard", "trophy"),
                    ]
                    {
                        button {
                            class: if screen == target { "nav-item active" } else { "nav-item" },
                            disabled: *
                                    BUSY.read(),
                            onclick: move |_| nav(target),
                            Icon { name: icon }
                            {t(label)}
                        }
                    }
                    button {
                        class: "nav-item",
                        onclick: move |_| ACCOUNT.signal().set(true),
                        Icon { name: "user" }
                        {t("Account")}
                    }
                }
            }
            if !game {
                footer { class: "club-footer", Rules {} }
            }
        }
        if USER.read().is_some() && RECEIPT.read().is_some(){ ChallengeReceipt{} }
        if *ACCOUNT.read() {
            Account {}
        }
        if USER.read().is_some() {
            if let Some(invite)=INVITATIONS.read().first().cloned(){
                aside {class:"invite-toast glass",aria_label:t("Table invitation"),
                    strong {"{invite[\"sender\"].as_str().unwrap_or_default()} · UnoGo"}
                    span {{t("invited you to a table")}}
                    button {disabled:*BUSY.read(),onclick:{let invite=invite.clone();move |_|{
                        let id=invite["tableId"].as_str().unwrap_or_default().to_string();BUSY.signal().set(true);*EPOCH.write()+=1;
                        dioxus::dioxus_core::spawn_forever(async move{enter(&id).await;refresh_invitations().await;BUSY.signal().set(false);});
                    }},{t("Join table")}}
                    button {class:"text-button",disabled:*BUSY.read(),onclick:{let id=invite["id"].as_str().unwrap_or_default().to_string();move |_|{let id=id.clone();dioxus::dioxus_core::spawn_forever(async move{match net::request("DELETE",&format!("/api/invitations/{id}"),Some(json!({}))).await{Ok(_)=>refresh_invitations().await,Err(e)=>error(e)}});}}, {t("Decline")}}
                }
            }
        }
    }
}
