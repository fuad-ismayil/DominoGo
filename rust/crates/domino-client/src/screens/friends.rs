use super::ui::{BackButton, Icon};
use crate::i18n::{t, tf};
use crate::{app::*, net, store::*};
use dioxus::prelude::*;

#[component]
pub fn Friends() -> Element {
    let mut name = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut loading = use_signal(|| true);
    let mut success = use_signal(String::new);
    let mut removing = use_signal(|| None::<String>);
    use_future(move || async move {
        refresh_friends().await;
        loading.set(false);
    });
    if SESSION.read().is_guest {
        return rsx! { section {class:"menu-page",BackButton {} section {class:"panel narrow glass",h1 {{t("Keep your people close.")}}p {class:"muted",{t("Save your guest account to add friends, receive invitations and keep your game history across devices.")}}button {onclick:move |_|nav(Screen::UpgradeAccount),{t("Save my account")}}p {class:"muted",{t("You can still play together right now: share a table code with your friends.")}}}}};
    }
    let view = FRIENDS.read().clone();
    let online = view.friends.iter().filter(|f| f.is_online).count();
    rsx! { section { class: "friends-page menu-page", BackButton {}
        div { class: "page-heading",  h1 { {t("Good company.")} } p { class: "muted", {t("Keep your people close. There's always room for one more round.")} } }
        div { class: "social-columns",
            section { class: "friends-panel glass", div { class: "section-heading", h2 { {t("Your friends")} } span { class: "member-pill glass-capsule", {tf("{online} online", &[("online",online.to_string())])} } }
                if loading() { p { class: "muted", role: "status", {t("Finding your people…")} } } else if view.friends.is_empty() { div { class: "social-empty", div { class: "empty-orbit glass-circle", Icon { name: "people" } } h3 { {t("A good game starts with an invitation.")} } p { {t("Add someone by their exact username. They'll be here once they accept.")} } } }
                for f in view.friends { div { class: "friend-card", div { class: "friend-main", span { class: "seat-avatar glass-circle", "{f.username.chars().next().unwrap_or('D').to_uppercase()}" } div { strong { "{f.username}" } small { class: if f.is_online { "online-status" } else { "muted" }, if f.is_online { {t("● Online")} } else { {t("Offline")} } } } }
                    if removing().as_ref()==Some(&f.id) { div { class: "inline-confirm", span { {t("Remove friend?")} } button { class: "compact-button secondary", disabled: busy(), onclick: { let id=f.id.clone(); move |_| { busy.set(true); let id=id.clone(); spawn(async move { if result(net::friend_remove(&id).await).await.is_some() { removing.set(None); refresh_friends().await; } busy.set(false); }); } }, {t("Remove")} } button { class: "compact-button text-button", onclick: move |_| removing.set(None), {t("Cancel")} } } } else { button { class: "round-control", aria_label: t("Remove friend"), title: t("Remove friend"), onclick: { let id=f.id.clone(); move |_| removing.set(Some(id.clone())) }, Icon { name: "close" } } }
                } }
            }
            div { class: "social-sidebar",
                section { class: "add-friend-panel glass", Icon { name: "plus" } h2 { {t("Bring someone along")} } p { class: "muted", {t("Enter their username exactly as they use it.")} }
                    form { onsubmit: move |e| { e.prevent_default(); if busy() {return;} busy.set(true); success.set(String::new()); spawn(async move { if result(net::friend_request(name().trim()).await).await.is_some() { success.set("Friend request sent.".into()); name.set(String::new()); refresh_friends().await; } busy.set(false); }); }, label { r#for: "friend", {t("Friend's username")} } input { id: "friend", required: true, maxlength: 32, autocomplete: "off", placeholder: t("Username"), value: "{name}", oninput: move |e| name.set(e.value()) } button { disabled: busy() || name().trim().is_empty(), if busy() { {t("Please wait…")} } else { {t("Send request")} } Icon { name: "arrow" } } }
                    if !success().is_empty() { p { class: "account-success", role: "status", {t(&success())} } }
                }
                section { class: "requests-panel glass", h2 { {t("Invitations to connect")} } h3 { {tf("Received · {view.incoming.len()}", &[("view.incoming.len()", format!("{}", (view.incoming.len())))])} }
                    if view.incoming.is_empty() { p { class: "muted", {t("You're all caught up.")} } }
                    for r in view.incoming { div { class: "request-card", strong { "{r.username}" } div { class: "actions", button { class: "compact-button", disabled: busy(), onclick: move |_| { busy.set(true); spawn(async move { result(net::friend_accept(r.id).await).await; refresh_friends().await; busy.set(false); }); }, {t("Accept")} } button { class: "compact-button text-button", disabled: busy(), onclick: move |_| { busy.set(true); spawn(async move { result(net::friend_reject(r.id).await).await; refresh_friends().await; busy.set(false); }); }, {t("Decline")} } } } }
                    h3 { {tf("Sent · {view.outgoing.len()}", &[("view.outgoing.len()", format!("{}", (view.outgoing.len())))])} }
                    if view.outgoing.is_empty() { p { class: "muted", {t("No pending requests.")} } }
                    for r in view.outgoing { div { class: "request-card sent-request", span { "{r.username}" } button { class: "compact-button text-button", disabled: busy(), onclick: move |_| { busy.set(true); spawn(async move { result(net::friend_cancel(r.id).await).await; refresh_friends().await; busy.set(false); }); }, {t("Cancel")} } } }
                }
            }
        }
    } }
}
