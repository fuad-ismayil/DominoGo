use super::ui::{BackButton, Icon};
use crate::i18n::t;
use crate::{app::*, net, store::*};
use dioxus::prelude::*;

#[component]
pub fn Setup() -> Element {
    let mut code = use_signal(String::new);
    let mut count = use_signal(|| 2i64);
    let mut busy = use_signal(|| false);
    let mut free_draw = use_signal(|| true);
    let create = *SCREEN.read() == Screen::CreateLobby;
    rsx! { section { class: "setup-page menu-page",
        BackButton {}
        div { class: "setup-card glass",
            h1 { if create { {t("Set the table.")} } else { {t("You're invited.")} } }
            p { class: "muted", if create { {t("Your table, your people. Pick the number of seats, then invite friends or add a bot.")} } else { {t("Ask your host for the seven-character table code and join the next round.")} } }
            form { onsubmit: move |e| { e.prevent_default(); if busy() { return; } busy.set(true); spawn(async move { if create { if let Some(v) = result(net::create_lobby(count(), if free_draw() { domino_core::DrawRule::Anytime } else { domino_core::DrawRule::WhenBlocked }).await).await { if let Some(id) = v["id"].as_str() { enter_lobby(id.to_string()).await; } } } else { enter_lobby(code().trim().to_uppercase()).await; } busy.set(false); }); },
                if create {
                    fieldset { class: "seat-picker", legend { {t("How many players?")} }
                        for n in [2i64,3,4] { button { r#type: "button", class: if count()==n { "seat-option selected glass" } else { "seat-option glass" }, aria_pressed: "{count()==n}", onclick: move |_| count.set(n), strong { "{n}" } span { {t("players")} } } }
                    }
                    div { class: "draw-rule-setting",
                        div { class: "draw-rule-heading", label { r#for: "free-draw", {t("Free drawing")} }
                            button { id: "free-draw", r#type: "button", class: "draw-rule-switch", role: "switch", aria_checked: "{free_draw()}", aria_describedby: "draw-rule-help", disabled: busy(), onclick: move |_| free_draw.set(!free_draw()), span {} }
                        }
                        p { id: "draw-rule-help", if free_draw() { {t("Draw on your turn, even with a matching tile.")} } else { {t("Draw only when you have no matching tile.")} } }
                        small { if count()==4 { {t("Four players use all tiles, so there is no market to draw from. ")} } {t("Fixed for this table. Cannot be changed during a match.")} }
                    }
                    div { class: "setup-note", Icon { name: "bot" } span { {t("Play solo with bots, with friends, or a little of both.")} } }
                } else { label { r#for: "code", {t("Table code")} } input { class: "code-input", id: "code", name: "table-code", required: true, maxlength: 7, minlength: 7, autocomplete: "off", spellcheck: "false", autocapitalize: "characters", placeholder: "ABC1234", value: "{code}", oninput: move |e| code.set(e.value().trim().to_uppercase()) } }
                button { class: "setup-submit", disabled: busy(), if busy() { {t("Opening your table…")} } else if create { {t("Create table")} } else { {t("Join table")} } Icon { name: "arrow" } }
            }
            small { class: "setup-footnote", {t("Private tables · 101 scoring · Double-six dominoes")} }
        }
    } }
}
