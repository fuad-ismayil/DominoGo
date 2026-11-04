use super::ui::Icon;
use crate::i18n::{t, tf};
use crate::{app::*, net, store::*};
use dioxus::prelude::*;
use wasm_bindgen::JsCast;

#[component]
pub fn AccountSheet() -> Element {
    use_effect(move || {
        if let Some(close) = crate::browser::document()
            .get_element_by_id("account-close")
            .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = close.focus();
        }
    });
    use_drop(move || {
        if let Ok(Some(trigger)) = crate::browser::document().query_selector(".profile-button") {
            if let Ok(trigger) = trigger.dyn_into::<web_sys::HtmlElement>() {
                let _ = trigger.focus();
            }
        }
    });
    let mut volume = use_signal(crate::game::audio::volume);
    let session = SESSION.read().clone();
    let stats = PLAYER_STATS.read().clone();
    let mut confirm = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut success = use_signal(String::new);
    let mut resetting = use_signal(|| false);
    rsx! { div { class: "sheet-backdrop", onclick: move |_| ACCOUNT_OPEN.signal().set(false),
        section { id:"account-sheet", class: "account-sheet glass", role: "dialog", aria_modal: "true", aria_label: t("Your account"), onkeydown: move |e| { if e.key() == Key::Escape { ACCOUNT_OPEN.signal().set(false); }
            if e.key() == Key::Tab {
                let doc=crate::browser::document();
                if let Ok(nodes)=doc.query_selector_all("#account-sheet button:not(:disabled),#account-sheet input:not(:disabled)") {
                    let length=nodes.length();
                    if length>0 { let first=nodes.item(0).and_then(|n|n.dyn_into::<web_sys::HtmlElement>().ok()); let last=nodes.item(length-1).and_then(|n|n.dyn_into::<web_sys::HtmlElement>().ok()); let active=doc.active_element();
                        if e.modifiers().contains(Modifiers::SHIFT) { if active.as_ref().is_some_and(|el|first.as_ref().is_some_and(|first| el.is_same_node(Some(first)))) {e.prevent_default();if let Some(last)=last{let _=last.focus();}} }
                        else if active.as_ref().is_some_and(|el|last.as_ref().is_some_and(|last|el.is_same_node(Some(last)))) {e.prevent_default();if let Some(first)=first{let _=first.focus();}}
                    }
                }
            } }, onclick: move |e| e.stop_propagation(),
            div { class: "section-heading", div {  h2 { {t("Your account")} } } button { class: "round-control glass-circle", id:"account-close", aria_label: t("Close account"), onclick: move |_| ACCOUNT_OPEN.signal().set(false), Icon { name: "close" } } }
            div { class: "account-identity", span { class: "profile-orbit glass-circle", "{session.username.chars().next().unwrap_or('D').to_uppercase()}" } div { h3 { "{session.username}" } span { class: "muted", if session.is_guest { {t("Guest player")} } else { {t("Registered player")} } } } }
            super::ui::LanguagePicker {}
            if let Some(s) = stats { div { class: "club-stats", div { strong { "{s.games_played}" } span { {t("Games")} } } div { strong { "{s.wins}" } span { {t("Wins")} } } div { strong { "{s.win_rate}%" } span { {t("Win rate")} } } } }
            if session.is_guest { div { class: "account-save", Icon { name: "shield" } p { {t("Keep your game history and sign in on any device. Add a password to save this guest account.")} } } button { onclick: move |_| { ACCOUNT_OPEN.signal().set(false); nav(Screen::UpgradeAccount); }, {t("Save my account")} } }
            div { class: "setting-row", div { strong { {t("Game sounds")} } small { {t("Tiles, turns, and celebrations")} } } button { class: "setting-switch glass-capsule", role: "switch", aria_checked: "{SOUND_ENABLED.read()}", onclick: move |_| { let enabled=!*SOUND_ENABLED.read(); SOUND_ENABLED.signal().set(enabled); crate::game::audio::set_enabled(enabled); }, if *SOUND_ENABLED.read() { {t("On")} } else { {t("Off")} } } }
            label { class:"volume-label", r#for:"sound-volume", {tf("Sound volume · {(volume()*100.).round()}%", &[("(volume()*100.).round()", format!("{}", ((volume()*100.).round())))])} }
            input { id:"sound-volume", r#type:"range", min:"0", max:"100", step:"1", value:"{(volume()*100.).round()}", oninput:move |e| { if let Ok(value)=e.value().parse::<f64>() {volume.set(value/100.);crate::game::audio::set_volume(value/100.);} } }
            if !session.is_guest {
                if resetting() { PasswordReset { oncancel: move |_| resetting.set(false) } }
                else { button { class: "account-action glass-capsule", onclick: move |_| { success.set(String::new()); resetting.set(true); }, Icon { name: "key" } {t("Reset password")} } }
            }
            if !session.is_guest { button { class: "account-action glass-capsule", disabled: busy(), onclick: move |_| { if busy() { return; } busy.set(true); spawn(async move { if result(net::auth_logout_others().await).await.is_some() { success.set("You are now signed out on other devices.".into()); } busy.set(false); }); }, Icon { name: "shield" } {t("Sign out other devices")} } }
            if !success().is_empty() { p { class: "account-success", role: "status", {t(&success())} } }
            if confirm() { div { class: "signout-confirm", p { {t("Sign out of this device?")} } if session.is_guest { small { {t("Save your account first to return to your progress.")} } } div { class: "actions", button { class: "danger-action", disabled: busy(), onclick: move |_| { busy.set(true); spawn(async move { let _ = net::auth_logout().await; { net::disconnect(); net::clear_token(); SESSION.signal().set(Session::default()); LOBBY.signal().set(LobbyView::default()); GAME_STATE.signal().set(None); ACCOUNT_OPEN.signal().set(false); MATCH_HISTORY.signal().set(Vec::new()); PLAYER_STATS.signal().set(None); FRIENDS.signal().set(FriendsView::default()); GAME_INVITE_TOASTS.signal().set(Vec::new()); nav(Screen::Entry); } busy.set(false); }); }, {t("Sign out")} } button { class: "secondary", onclick: move |_| confirm.set(false), {t("Stay here")} } } } } else { button { class: "account-action text-button", onclick: move |_| confirm.set(true), Icon { name: "logout" } {t("Sign out")} } }
        }
    } }
}

#[component]
fn PasswordReset(oncancel: EventHandler) -> Element {
    let mut password = use_signal(String::new);
    let mut confirmation = use_signal(String::new);
    let mut visible = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    let mut saved = use_signal(|| false);
    rsx! { section { class: "password-reset", aria_label: t("Reset password"),
        if saved() {
            p { class: "account-success", role: "status", {t("Password updated. You’re still signed in here; other devices have been signed out.")} }
            button { class: "secondary", onclick: move |_| oncancel.call(()), {t("Done")} }
        } else {
            h3 { {t("Reset password")} }
            p { {t("Choose a new password. Your current login is all you need.")} }
            form { onsubmit: move |e| {
                e.prevent_default();
                if busy() { return; }
                error.set(String::new());
                if password().chars().count() < 8 { error.set("Use at least 8 characters.".into()); return; }
                if password() != confirmation() { error.set("The passwords don’t match.".into()); return; }
                busy.set(true);
                spawn(async move {
                    match net::auth_reset_password(&password()).await {
                        Ok(_) => { password.set(String::new()); confirmation.set(String::new()); saved.set(true); }
                        Err(e) => error.set(e.message),
                    }
                    busy.set(false);
                });
            },
                label { r#for: "new-account-password", {t("New password")} }
                div { class: "password-field",
                    input { id: "new-account-password", name: "new-password", r#type: if visible() { "text" } else { "password" }, autocomplete: "new-password", required: true, minlength: 8, maxlength: 1024, disabled: busy(), value: "{password}", oninput: move |e| password.set(e.value()), onmounted: move |_| { if let Some(el)=crate::browser::document().get_element_by_id("new-account-password").and_then(|el|el.dyn_into::<web_sys::HtmlElement>().ok()) {let _=el.focus();} } }
                    button { r#type: "button", class: "password-toggle", aria_label: if visible() { t("Hide new password") } else { t("Show new password") }, onclick: move |_| visible.set(!visible()), if visible() { {t("Hide")} } else { {t("Show")} } }
                }
                label { r#for: "confirm-account-password", {t("Confirm new password")} }
                input { id: "confirm-account-password", name: "confirm-password", r#type: if visible() { "text" } else { "password" }, autocomplete: "new-password", required: true, minlength: 8, maxlength: 1024, disabled: busy(), value: "{confirmation}", oninput: move |e| confirmation.set(e.value()) }
                small { {t("At least 8 characters. Other devices will be signed out.")} }
                if !error().is_empty() { p { class: "password-error", role: "alert", {t(&error())} } }
                div { class: "actions",
                    button { r#type: "submit", disabled: busy(), if busy() { {t("Updating…")} } else { {t("Update password")} } }
                    button { r#type: "button", class: "secondary", disabled: busy(), onclick: move |_| oncancel.call(()), {t("Cancel")} }
                }
            }
        }
    } }
}
