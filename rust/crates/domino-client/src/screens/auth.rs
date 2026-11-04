use super::ui::Icon;
use crate::i18n::t;
use crate::{app::*, net, store::*};
use dioxus::prelude::*;

#[component]
pub fn Auth() -> Element {
    let screen = *SCREEN.read();
    let mut username = use_signal(|| {
        if screen == Screen::UpgradeAccount {
            SESSION.read().username.clone()
        } else {
            String::new()
        }
    });
    let mut guest_name = use_signal(|| "Guest".to_string());
    let mut password = use_signal(String::new);
    let mut visible = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let title = match screen {
        Screen::Register => "A seat of your own.",
        Screen::Login => "Welcome back.",
        Screen::UpgradeAccount => "Keep your good games.",
        _ => "The classics bring us together.",
    };
    rsx! { section { class: "auth-layout",
        div { class: "auth-story",  h1 { {t("A familiar game.")} br {} span { class: "gold", {t("A fresh table.")} } } p { {t("Play a relaxed round of dominoes with friends, or sharpen your instincts against the bots.")} }
            div { class: "club-promise", span { {t("2–4 players")} } span { {t("101 scoring")} } span { {t("Friends & bots")} } }
        }
        section { class: "auth-card glass",  h2 { {t(title)} } super::ui::LanguagePicker {}
            if screen == Screen::Entry {
                p { class: "muted", {t("Pick a name and pull up a chair. No account needed.")} }
                form { onsubmit: move |e| { e.prevent_default(); if busy() { return; } busy.set(true); spawn(async move { if let Some(v) = result(net::auth_guest(&guest_name()).await).await { apply_auth(&v); let guest=SESSION.read().is_guest; if guest {if let Ok(profile)=net::auth_me().await {apply_auth(&profile);}} net::ensure_connected(); nav(Screen::MainMenu); refresh_menu().await; refresh_friends().await; } busy.set(false); }); },
                    label { r#for: "guest-name", {t("What should we call you?")} }
                    input { id: "guest-name", name: "guest-name", maxlength: 16, required: true, autocomplete: "nickname", value: "{guest_name}", oninput: move |e| guest_name.set(e.value()) }
                    button { r#type: "submit", disabled: busy() || guest_name().trim().is_empty(), if busy() { {t("Finding your seat…")} } else { {t("Play as guest")} } Icon { name: "arrow" } }
                }
                div { class: "auth-divider", span { {t("Already a regular?")} } }
                button { class: "secondary glass-capsule", disabled: busy(), onclick: move |_| nav(Screen::Login), {t("Sign in")} }
                button { class: "text-button", disabled: busy(), onclick: move |_| nav(Screen::Register), {t("Create an account")} }
            } else {
                p { class: "muted", if screen == Screen::UpgradeAccount { {t("Your matches, friends, and scores stay with you. Add a password to make this account permanent.")} } else if screen == Screen::Register { {t("Save your progress and bring your game to any device.")} } else { {t("Your table, your friends, your next round.")} } }
                form { onsubmit: move |e| { e.prevent_default(); if busy() { return; } busy.set(true); spawn(async move { let r = match screen { Screen::Register => net::auth_register(username().trim(), &password()).await, Screen::UpgradeAccount => net::auth_upgrade(username().trim(), &password()).await, _ => net::auth_login(username().trim(), &password()).await }; if let Some(v) = result(r).await { apply_auth(&v); let guest=SESSION.read().is_guest; if guest {if let Ok(profile)=net::auth_me().await {apply_auth(&profile);}} net::ensure_connected(); nav(Screen::MainMenu); refresh_menu().await; refresh_friends().await; } busy.set(false); }); },
                    label { r#for: "username", {t("Username")} }
                    input { id: "username", name: "username", autocomplete: "username", required: true, maxlength: 32, value: "{username}", oninput: move |e| username.set(e.value()) }
                    label { r#for: "password", {t("Password")} }
                    div { class: "password-field", input { id: "password", name: "password", r#type: if visible() { "text" } else { "password" }, autocomplete: if screen == Screen::Login { "current-password" } else { "new-password" }, minlength: if screen == Screen::Login { 1 } else { 8 }, maxlength: 1024, required: true, value: "{password}", oninput: move |e| password.set(e.value()) } button { r#type: "button", class: "password-toggle", aria_label: if visible() { t("Hide password") } else { t("Show password") }, onclick: move |_| visible.set(!visible()), if visible() { {t("Hide")} } else { {t("Show")} } } }
                    if screen != Screen::Login { small { class: "muted", {t("At least 8 characters. Make it something only you know.")} } }
                    button { r#type: "submit", disabled: busy() || username().trim().is_empty() || password().is_empty(), if busy() { {t("Please wait…")} } else if screen == Screen::UpgradeAccount { {t("Save my account")} } else if screen == Screen::Register { {t("Create account")} } else { {t("Sign in")} } Icon { name: "arrow" } }
                }
                button { class: "text-button", disabled: busy(), onclick: move |_| nav(if screen == Screen::UpgradeAccount { Screen::MainMenu } else { Screen::Entry }), Icon { name: "back" } if screen == Screen::UpgradeAccount { {t("Back home")} } else { {t("Back")} } }
            }
        }
    } }
}
