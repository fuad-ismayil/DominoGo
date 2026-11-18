//! Reactions use one unclipped table layer, including the local player's emote.
use crate::{
    browser,
    store::{CLOCK_OFFSET, TABLE},
};
use dioxus::prelude::*;
use wasm_bindgen::JsCast;

#[component]
pub fn TableReactions() -> Element {
    let Some(table) = TABLE.read().clone() else {
        return rsx! {};
    };
    rsx! { div { class:"table-reaction-layer",
        for (seat,player) in table.players.iter().enumerate() {
            if let Some(reaction)=table.reactions.iter().find(|r|r["userId"].as_str()==player.user_id.as_deref()) {
                ReactionBubble {
                    key:"{reaction[\"id\"].as_str().unwrap_or_default()}",
                    id:reaction["id"].as_str().unwrap_or_default().to_string(),
                    emoji:reaction["emoji"].as_str().unwrap_or_default().to_string(),
                    name:player.name.clone(),seat,local:seat==table.local_seat,
                    created_at:reaction["createdAt"].as_u64().unwrap_or_default()
                }
            }
        }
    }}
}

fn place(id: &str, seat: usize, local: bool) {
    let doc = browser::document();
    let Some(bubble) = doc
        .get_element_by_id(id)
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
    else {
        return;
    };
    let (Ok(Some(board)), Ok(Some(anchor))) = (
        doc.query_selector(".game-board"),
        doc.query_selector(&if local {
            ".table-emotes".into()
        } else {
            format!(".opponent[data-seat='{seat}']")
        }),
    ) else {
        return;
    };
    let b = board.get_bounding_client_rect();
    let a = anchor.get_bounding_client_rect();
    let x = a.x() + a.width() / 2.;
    let mut visible = true;
    if !local {
        if let Ok(Some(rail)) = doc.query_selector(".opponents") {
            let r = rail.get_bounding_client_rect();
            visible = x >= r.left() + 12. && x <= r.right() - 12.;
        }
    }
    let style = bubble.style();
    let _ = style.set_property(
        "left",
        &format!("{}px", (x - b.x()).clamp(22., (b.width() - 22.).max(22.))),
    );
    let _ = style.set_property(
        "top",
        &format!(
            "{}px",
            if local {
                a.top() - b.top() - 22.
            } else {
                a.top() - b.top() + 12.
            }
        ),
    );
    let _ = style.set_property("visibility", if visible { "visible" } else { "hidden" });
}

#[component]
fn ReactionBubble(
    id: String,
    emoji: String,
    name: String,
    seat: usize,
    local: bool,
    created_at: u64,
) -> Element {
    let node_id = format!("table-reaction-{id}");
    // Negative delay preserves the event's age across polling/reconnect. Repeated
    // snapshots retain this keyed node instead of restarting its animation.
    let age = use_hook(|| {
        let server_now = (browser::now() as i64 - *CLOCK_OFFSET.read()).max(0) as u64;
        server_now.saturating_sub(created_at).min(3_000)
    });
    let position_id = node_id.clone();
    use_future(move || {
        let id = position_id.clone();
        async move {
            for _ in 0..38 {
                browser::sleep(80).await;
                place(&id, seat, local);
            }
        }
    });
    rsx! {span{id:node_id,class:if local{"reaction-bubble own-reaction"}else{"reaction-bubble"},
        "data-reaction-id":id,"data-seat":"{seat}","data-own":"{local}",
        style:"visibility:hidden;animation-delay:-{age}ms",role:"status",aria_label:format!("{name}: {emoji}"),"{emoji}"
    }}
}
