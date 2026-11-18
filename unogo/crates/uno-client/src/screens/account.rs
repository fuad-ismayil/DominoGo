use crate::{browser, i18n::t, net, store::*, ui::*};
use dioxus::prelude::*;
use serde_json::json;
#[component]
pub fn Account() -> Element {
    use_effect(move || browser::focus(".account-sheet button"));
    use_drop(move || browser::focus(".profile-button"));
    let mut sound = use_signal(|| browser::saved("unogo.sound").as_deref() != Some("off"));
    let mut volume = use_signal(|| {
        browser::saved("unogo.volume")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.3)
    });
    let mut password = use_signal(String::new);
    let mut done = use_signal(String::new);
    use_future(move || async move {
        refresh_home().await;
    });
    let Some(user) = USER.read().clone() else {
        return rsx! {};
    };
    let stats = STATS.read().clone();
    rsx! {
        div {
            class: "modal-backdrop account-backdrop",
            onkeydown: move |e| {
                browser::trap_focus(&e); if e.key() == Key::Escape && !*BUSY.read() {
                    ACCOUNT.signal().set(false);
                }
            },
            section {
                class: "account-sheet glass",
                role: "dialog",
                aria_modal: "true",
                aria_label: t("Account"),
                div { class: "section-heading",
                    h2 { {t("Your account")} }
                    button {
                        class: "round-control glass-circle",
                        autofocus: true,
                        aria_label: t("Close account"),
                        disabled: *BUSY.read(),
                        onclick: move | _
                                | ACCOUNT.signal().set(false),
                        Icon { name: "close" }
                    }
                }
                div { class: "account-body",
                    h3 { "{user.username}" }
                    p { class: "muted",
                        if user.is_guest {
                            {t("Guest access expires after 24 hours of inactivity.")}
                        } else {
                            {t("Shared with DominoGo")}
                        }
                    }
                    if !ERROR.read().is_empty(){p{class:"uno-error",role:"alert","{ERROR}"}}
                    div { class: "account-stats",
                        for (key, label) in [
                            ("wins", "Wins"),
                            ("losses", "Losses"),
                            ("winRate", "Win rate"),
                            ("gamesPlayed", "Games"),
                        ]
                        {
                            div {
                                strong {
                                    "{stats[key].as_u64().unwrap_or(0)}"
                                    if key == "winRate" {
                                        "%"
                                    }
                                }
                                span { {t(label)} }
                            }
                        }
                    }
                    LanguagePicker {}
                    div { class: "account-setting",
                        label { r#for: "sound-toggle", {t("Card sounds")} }
                        button {
                            id: "sound-toggle",
                            class: "secondary",
                            role: "switch",
                            aria_checked: sound(),
                            onclick: move |_| {
                                let value = !sound();
                                sound.set(value);
                                browser::save("unogo.sound", if value { "on" } else { "off" });
                                browser::unlock_sound();
                                if value {
                                    browser::card_sound();
                                }
                            },
                            if sound() {
                                {t("On")}
                            } else {
                                {t("Off")}
                            }
                        }
                    }
                    label { r#for: "sound-volume", {t("Volume")} }
                    input {
                        id: "sound-volume",
                        r#type: "range",
                        min: 0,
                        max: 1,
                        step: 0.05,
                        value: "{volume}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<f32>() {
                                volume.set(v);
                                browser::save("unogo.volume", &v.to_string());
                            }
                        },
                    }
                    button {
                        class: "secondary",
                        onclick: move |_| {
                            if let Some(root) = browser::document().document_element() {
                                if browser::document().fullscreen_element().is_some() {
                                    browser::document().exit_fullscreen();
                                } else {
                                    let _ = root.request_fullscreen();
                                }
                            }
                        },
                        {t("Fullscreen")}
                    }
                    if user.is_guest {
                        button {
                            onclick: move |_| {
                                ACCOUNT.signal().set(false);
                                nav(Screen::Upgrade);
                            },
                            {t("Save my account")}
                        }
                    } else {
                        details { class: "account-password",
                            summary { {t("Change password")} }
                            form {
                                onsubmit: move |e| {
                                    e.prevent_default();
                                    if *BUSY.read() {
                                        return;
                                    }
                                    BUSY.signal().set(true);
                                    ERROR.signal().set(String::new());
                                    done.set(String::new());
                                    let pass = password();
                                    dioxus::dioxus_core::spawn_forever(async move {
                                        match net::request(
                                                "POST",
                                                "/api/auth/reset-password",
                                                Some(json!({ "password" : pass })),
                                            )
                                            .await
                                        {
                                            Ok(_) => {
                                                done.set(t("Password updated. Other devices have been signed out."));
                                                password.set(String::new());
                                            }
                                            Err(e) => error(e),
                                        }
                                        BUSY.signal().set(false);
                                    });
                                },
                                label { r#for: "new-password", {t("New password")} }
                                input {
                                    id: "new-password",
                                    r#type: "password",
                                    autocomplete: "new-password",
                                    required: true,
                                    minlength: 8,
                                    maxlength: 1024,
                                    value: "{password}",
                                    oninput: move | e |
                                            password.set(e.value()),
                                }
                                button { disabled: *BUSY.read(), { t("Update password")
                                        } }
                            }
                        }
                    }
                    if !done().is_empty() {
                        p { class: "account-success", role: "status", "{done}" }
                    }
                    div { class: "account-session-actions",
                        button {
                            class: "secondary",
                            disabled: *BUSY.read(),
                            onclick: move |_| {
                                BUSY.signal().set(true);
                                dioxus::dioxus_core::spawn_forever(async move {
                                    match net::request("POST", "/api/auth/logout-others", Some(json!({}))).await
                                    {
                                        Ok(_) => done.set(t("Other devices have been signed out.")),
                                        Err(e) => error(e),
                                    }
                                    BUSY.signal().set(false);
                                });
                            },
                            {t("Sign out other devices")}
                        }
                        button {
                            class: "text-button",
                            disabled: *BUSY.read(),
                            onclick: move |_| {
                                BUSY.signal().set(true);
                                dioxus::dioxus_core::spawn_forever(async move {
                                    match net::request("POST", "/api/auth/logout", Some(json!({}))).await {
                                        Ok(_) => clear_session(),
                                        Err(e) => error(e),
                                    }
                                    BUSY.signal().set(false);
                                });
                            },
                            {t("Sign out")}
                        }
                    }
                }
            }
        }
    }
}
