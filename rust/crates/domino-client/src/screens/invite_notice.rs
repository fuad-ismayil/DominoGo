use crate::i18n::t;
use crate::{invite::InviteProblem, store::INVITE_NOTICE};
use dioxus::prelude::*;
use wasm_bindgen::JsCast;

#[component]
pub fn InviteNotice(problem: InviteProblem) -> Element {
    use_effect(move || {
        if let Some(button) = crate::browser::document()
            .get_element_by_id("invite-notice-dismiss")
            .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = button.focus();
        }
    });
    use_drop(move || {
        if let Ok(Some(button)) = crate::browser::document().query_selector(".brand-button") {
            if let Ok(button) = button.dyn_into::<web_sys::HtmlElement>() {
                let _ = button.focus();
            }
        }
    });
    rsx! { div { class: "invite-notice-backdrop", onclick: move |_| INVITE_NOTICE.signal().set(None),
        section { class: "invite-notice-card glass", role: "dialog", aria_modal: "true", aria_labelledby: "invite-notice-title", aria_describedby: "invite-notice-copy",
            onclick: move |e| e.stop_propagation(),
            onkeydown: move |e| {
                if e.key()==Key::Escape { INVITE_NOTICE.signal().set(None); }
                if e.key()==Key::Tab { e.prevent_default(); }
            },
            div { class: "invite-notice-icon glass-circle", super::ui::Icon { name: "people" } }

            h2 { id: "invite-notice-title", {t(problem.title())} }
            p { id: "invite-notice-copy", {t(problem.message())} }
            button { id: "invite-notice-dismiss", class: "glass-capsule", onclick: move |_| INVITE_NOTICE.signal().set(None), {t("Back home")} super::ui::Icon { name: "arrow" } }
        }
    } }
}
