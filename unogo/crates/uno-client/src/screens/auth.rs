use crate::{browser, i18n::t, net, store::*, ui::*};
use dioxus::prelude::*;
use serde_json::json;
#[component]
pub fn Auth() -> Element {
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut guest = use_signal(|| "Guest".to_string());
    let mut visible = use_signal(|| false);
    let screen = *SCREEN.read();
    let entry = screen == Screen::Entry;
    let title = match screen {
        Screen::Login => "Welcome back.",
        Screen::Register => "A seat of your own.",
        Screen::Upgrade => "Keep your good games.",
        _ => "The classics bring us together.",
    };
    rsx! {
        section { class: "auth-layout",
            div { class: "auth-story",
                div { class: "uno-auth-cards", aria_hidden: "true",
                    for (color, n) in [
                        (uno_core::Color::Red, 7),
                        (uno_core::Color::Yellow, 3),
                        (uno_core::Color::Blue, 2),
                    ]
                    {
                        div {
                            CardFace {
                                card: uno_core::Card {
                                    id: n,
                                    color: Some(color),
                                    value: uno_core::Value::Number(n),
                                },
                            }
                        }
                    }
                }
                h1 { {t("Good friends. One more round.")} }
                p {
                    {
                        t(
                            "Your people, your table. Match a color, change the game, and remember to call UNO.",
                        )
                    }
                }
            }
            div { class: "auth-card glass",
                h2 { {t(title)} }
                p { class: "muted",
                    if entry {
                        {t("Pick a name and pull up a chair. No account needed.")}
                    } else if screen == Screen::Upgrade {
                        {
                            t("Your matches and friends stay with you. Add a password to keep this account.")
                        }
                    } else {
                        {t("Use your DominoGo account here too.")}
                    }
                }
                form {
                    onsubmit: move |e| {
                        e.prevent_default();
                        if *BUSY.read() {
                            return;
                        }
                        BUSY.signal().set(true);
                        ERROR.signal().set(String::new());
                        browser::unlock_sound();
                        let name = if entry {
                            guest().trim().to_string()
                        } else {
                            username().trim().to_string()
                        };
                        let pass = password();
                        dioxus::dioxus_core::spawn_forever(async move {
                            let (path, body) = match screen {
                                Screen::Entry => ("guest", json!({ "prefix" : name })),
                                Screen::Register => {
                                    ("register", json!({ "username" : name, "password" : pass }))
                                }
                                Screen::Upgrade => {
                                    ("upgrade", json!({ "newUsername" : name, "password" : pass }))
                                }
                                _ => ("login", json!({ "username" : name, "password" : pass })),
                            };
                            match net::request("POST", &format!("/api/auth/{path}"), Some(body)).await {
                                Ok(v) => {
                                    accept_auth(&v);
                                    if let Ok(profile) = net::request("GET", "/api/auth/me", None).await
                                    {
                                        accept_auth(&profile);
                                    }
                                    SCREEN.signal().set(Screen::MainMenu);
                                    ACCOUNT.signal().set(false);
                                    refresh_home().await;
                                }
                                Err(e) => ERROR.signal().set(t(&e.message)),
                            }
                            BUSY.signal().set(false);
                        });
                    },
                    if entry {
                        label { r#for: "guest-name", {t("What should we call you?")} }
                        input {
                            id: "guest-name",
                            name: "guest-name",
                            maxlength: 16,
                            required: true,
                            autocomplete: "nickname",
                            value: "{guest}",
                            oninput: move |e| guest.set(e.value()),
                        }
                    } else {
                        label { r#for: "username", {t("Username")} }
                        input {
                            id: "username",
                            name: "username",
                            required: true,
                            maxlength: 32,
                            autocomplete: "username",
                            value: "{username}",
                            oninput: move |e| username.set(e.value()),
                        }
                        label { r#for: "password", {t("Password")} }
                        div { class: "password-field",
                            input {
                                id: "password",
                                name: "password",
                                r#type: if visible() { "text" } else { "password" },
                                autocomplete: if screen == Screen::Login { "current-password" } else { "new-password" },
                                minlength: if screen == Screen::Login { 1 } else { 8 },
                                maxlength: 1024,
                                required: true,
                                value: "{password}",
                                oninput: move | e | password.set(e
                                        .value()),
                            }
                            button {
                                r#type: "button",
                                class: "password-toggle",
                                onclick: move |_| visible.set(!visible()),
                                if visible() {
                                    {t("Hide")}
                                } else {
                                    {t("Show")}
                                }
                            }
                        }
                    }
                    button { r#type: "submit", disabled: *BUSY.read(),
                        if *BUSY.read() {
                            {t("Please wait…")}
                        } else if entry {
                            {t("Play as guest")}
                        } else if screen == Screen::Register {
                            {t("Create account")}
                        } else if screen == Screen::Upgrade {
                            {t("Save my account")}
                        } else {
                            {t("Sign in")}
                        }
                        Icon { name: "arrow" }
                    }
                }
                if entry {
                    div { class: "auth-divider",
                        span { {t("Already a regular?")} }
                    }
                    button {
                        class: "secondary glass-capsule",
                        disabled: *BUSY.read(),
                        onclick: move |_| nav(Screen::Login),
                        {t("Sign in")}
                    }
                    button {
                        class: "text-button",
                        disabled: *BUSY.read(),
                        onclick: move | _ |
                                nav(Screen::Register),
                        {t("Create an account")}
                    }
                } else {
                    button {
                        class: "text-button",
                        disabled: *BUSY.read(),
                        onclick: move |_| nav(
                            if screen == Screen::Upgrade { Screen::MainMenu } else { Screen::Entry },
                        ),
                        Icon { name: "back" }
                        {t("Back")}
                    }
                }
            }
        }
    }
}
