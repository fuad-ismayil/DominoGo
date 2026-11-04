use super::ui::Icon;
use crate::i18n::t;
use crate::{app::*, net, store::*};
use dioxus::prelude::*;
use serde_json::json;
use wasm_bindgen::JsCast;

async fn copy_text(text: String) -> bool {
    let nav = crate::browser::window().navigator();
    let Ok(clipboard) = js_sys::Reflect::get(&nav, &"clipboard".into()) else {
        return false;
    };
    let Ok(method) = js_sys::Reflect::get(&clipboard, &"writeText".into())
        .and_then(|v| v.dyn_into::<js_sys::Function>())
    else {
        return false;
    };
    let Ok(promise) = method
        .call1(&clipboard, &text.into())
        .and_then(|v| v.dyn_into::<js_sys::Promise>())
    else {
        return false;
    };
    wasm_bindgen_futures::JsFuture::from(promise).await.is_ok()
}

#[component]
pub fn Lobby() -> Element {
    let lobby = LOBBY.read().clone();
    let friends = FRIENDS.read().friends.clone();
    let mut busy = use_signal(|| false);
    let mut copied = use_signal(|| false);
    let mut link_copied = use_signal(|| false);
    let mut link_failed = use_signal(|| false);
    let invite_link = crate::browser::invite_link(&lobby.lobby_id);
    let inviting = use_signal(|| None::<String>);
    let sent = use_signal(Vec::<String>::new);
    let seats = lobby.players.clone();
    rsx! { section { class: "lobby-page menu-page",
           div { class: "lobby-intro",  h1 { {t("Good games start here.")} } p { class: "muted", if lobby.is_host { {t("You're the host. Fill the seats with friends or bots, then deal the first round.")} } else { {t("You're in. Settle in while your host gets the table ready.")} } } }
           div { class: "lobby-columns",
               section { class: "lobby-seats glass", div { class: "section-heading", h2 { {t("At the table")} } span { class: "member-pill glass-capsule", "{seats.len()} / {lobby.max_players}" } }
                   div { class: "lobby-draw-rule", strong { {t("Draw rule")} } span { {t(lobby.draw_rule.description())} } small { {t("Fixed for this table")} } }
                   for (i,p) in seats.iter().enumerate() { div { class: "seat-row", span { class: "seat-avatar glass-circle", if p.id.starts_with("ai_") { Icon { name: "bot" } } else { "{p.username.chars().next().unwrap_or('D').to_uppercase()}" } } div { strong { {player_label(&p.id,&p.username)} } small { if p.id==SESSION.read().user_id { if lobby.is_host { {t("You · Host")} } else { {t("You")} } } else if p.id.starts_with("ai_") { {t("Bot player")} } else { {t("Ready to play")} } } }
                       if lobby.is_host && p.id != SESSION.read().user_id { button { class: "round-control", aria_label: t("Remove player from table"), title: t("Remove player"), disabled: busy(), onclick: { let id=lobby.lobby_id.clone(); let player=p.id.clone(); move |_| { busy.set(true); let id=id.clone(); let player=player.clone(); spawn(async move { if let Some(v)=result(net::kick_rest(&id,&player).await).await { LOBBY.signal().write().players=parse_players(&v["players"]); } busy.set(false); }); } }, Icon { name: "close" } } } else { span { class: "seat-number", "0{i+1}" } }
                   } }
                   for i in seats.len()..lobby.max_players { div { class: "seat-row empty-seat", span { class: "seat-avatar", Icon { name: "plus" } } div { strong { {t("Open seat")} } small { {t("Invite a friend or add a bot")} } } span { class: "seat-number", "0{i+1}" } } }
                   if lobby.is_host { div { class: "lobby-controls", button { class: "secondary glass-capsule", disabled: busy() || seats.len()>=lobby.max_players, onclick: move |_| { busy.set(true); let id=LOBBY.read().lobby_id.clone(); spawn(async move { if let Some(v)=result(net::add_bot_rest(&id).await).await { LOBBY.signal().write().players=parse_players(&v["players"]); } busy.set(false); }); }, Icon { name: "bot" } {t("Add a bot")} } button { disabled: busy() || seats.len()<2, onclick: move |_| { busy.set(true); let id=LOBBY.read().lobby_id.clone(); spawn(async move { result(net::start_game_rest(&id).await).await; busy.set(false); }); }, if busy() { {t("Please wait…")} } else { {t("Start match")} } Icon { name: "arrow" } } } if seats.len()<2 { small { class: "muted", {t("Add at least one more player to start.")} } } } else { p { class: "waiting-message", span { class: "waiting-dot" } {t("Waiting for your host to start the match…")} } }
               }
               section { class: "lobby-sharing glass",  h2 { {t("Share your code")} } p { class: "muted", {t("Your friends can enter this code from Join table.")} }
                   div { class: "share-code", code { "{lobby.lobby_id}" } button { class: "round-control glass-circle", aria_label: t("Copy table code"), onclick: { let id=lobby.lobby_id.clone(); move |_| { let id=id.clone(); spawn(async move { if copy_text(id).await { copied.set(true); } else { error("Copy isn't available here. Select the table code above to copy it."); } }); } }, Icon { name: "copy" } } }
                   if copied() { small { class: "account-success", role: "status", {t("Copied. Send it to your friends.")} } }
                   section { class: "share-invite-link", aria_label: t("Share an invite link"),
                       h3 { {t("A seat in one tap.")} }
                       p { {t("Send a link. Your friends can join the table and settle in before the first round.")} }
                       label { r#for: "table-invite-link", {t("Invite link")} }
                       input { id: "table-invite-link", r#type: "text", readonly: true, value: "{invite_link}", onfocus: move |_| { if let Some(input)=crate::browser::document().get_element_by_id("table-invite-link").and_then(|el|el.dyn_into::<web_sys::HtmlInputElement>().ok()) { input.select(); } } }
                       button { class: "secondary glass-capsule", onclick: { let link=invite_link.clone(); move |_| { let link=link.clone(); link_copied.set(false); link_failed.set(false); spawn(async move { if copy_text(link).await { link_copied.set(true); } else { link_failed.set(true); } }); } }, Icon { name: "link" } if link_copied() { {t("Link copied")} } else { {t("Copy invite link")} } }
                       if link_copied() { small { class: "account-success", role: "status", {t("Copied. Send a seat to a friend.")} } }
                       if link_failed() { small { class: "muted", role: "status", {t("Select the link above to copy it, then send it to a friend.")} } }
                   }
                   button { class: "secondary glass-capsule", onclick: move |_| { let open=!*SHOW_INVITE_MODAL.read(); SHOW_INVITE_MODAL.signal().set(open); if open { spawn(refresh_friends()); } }, Icon { name: "people" } if *SHOW_INVITE_MODAL.read() { {t("Close friends")} } else { {t("Invite a friend")} } }
                   if *SHOW_INVITE_MODAL.read() { div { class: "invite-list", if friends.is_empty() { p { class: "muted", {t("No friends to invite yet. Share the code above, or add friends from Home.")} } } for f in friends { div { class: "friend-invite-row", span { strong { "{f.username}" } small { if f.is_online { {t("Online")} } else { {t("Offline")} } } } button { class: "compact-button secondary", disabled: inviting().is_some() || sent().contains(&f.id), onclick: { let friend=f.id.clone(); move |_| { let mut inviting=inviting; inviting.set(Some(friend.clone())); let friend=friend.clone(); let id=LOBBY.read().lobby_id.clone(); net::emit("invite_to_lobby", json!({"lobbyId":id,"friendUserId":friend}), Some(Box::new(move |v| { let mut sent=sent; let mut inviting=inviting; if let Some(e)=v["error"].as_str() { error(e); } else { if let Ok(mut list)=sent.try_write(){list.push(friend.clone());} }
    if let Ok(mut pending)=inviting.try_write(){*pending=None;}; }))); } }, if sent().contains(&f.id) { {t("Invited")} } else { {t("Invite")} } } } } } }
               }
           }
           button { class: "leave-table text-button", disabled: busy(), onclick: move |_| { busy.set(true); let id=LOBBY.read().lobby_id.clone(); spawn(async move { if result(net::leave_lobby_rest(&id).await).await.is_some() { net::emit("leave_lobby",json!(id),None); reset_to_menu(); refresh_menu().await; } busy.set(false); }); }, Icon { name: "back" } {t("Leave table")} }
       } }
}
