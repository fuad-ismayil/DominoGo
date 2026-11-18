use crate::{
    i18n::{self, t},
    model::{card_value, color_name},
    store::*,
};
use dioxus::prelude::*;
use uno_core::{Card, Color};
#[component]
pub fn Icon(name: &'static str) -> Element {
    let path = match name {
        "smile" => "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0ZM8 14s1 3 4 3 4-3 4-3M8 9h.01M16 9h.01",
        "plus" => "M12 5v14M5 12h14",
        "arrow" => "M5 12h14m-6-6 6 6-6 6",
        "back" => "M19 12H5m6-6-6 6 6 6",
        "close" => "m6 6 12 12M6 18 18 6",
        "home" => "m3 10 9-7 9 7v10H4V10ZM9 20v-7h6v7",
        "people" => {
            "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2M16 3a4 4 0 0 1 0 8M22 21v-2a4 4 0 0 0-3-3.87M13 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z"
        }
        "user" => "M20 21v-2a7 7 0 0 0-14 0v2M17 7a5 5 0 1 1-10 0 5 5 0 0 1 10 0Z",
        "trophy" => {
            "M8 3h8v6a4 4 0 0 1-8 0V3ZM8 5H4v2a4 4 0 0 0 4 4M16 5h4v2a4 4 0 0 1-4 4M12 13v6M8 21h8"
        }
        "refresh" => "M20 7v5h-5M4 17v-5h5M6 6a8 8 0 0 1 14 6M4 12a8 8 0 0 0 14 6",
        "copy" => "M9 9h12v12H9V9ZM15 5V3H3v12h2",
        "history" => "M3 11a9 9 0 1 1 2.5 7M3 4v7h7M12 7v5l3 2",
        "bot" => "M5 7h14v13H5V7ZM12 3v4M2 11v5M22 11v5M8 11h1M15 11h1M9 16h6",
        "cards" => "m8 3 12 3-4 16-12-3L8 3ZM5 4 2 5l3 15",
        _ => "M12 3v18M3 12h18",
    };
    rsx! {
        svg {
            width: "20",
            height: "20",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.7",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            path { d: path }
        }
    }
}
#[component]
pub fn Brand() -> Element {
    rsx! {
        span { class: "uno-brand-icon", aria_hidden: "true",
            img {
                src: "/brand/unogo-mark-v2.svg",
                width: "60",
                height: "60",
                alt: "",
                draggable: "false",
            }
        }
        span { class: "brand-wordmark",
            "UNO"
            span { class: "gold", "GO" }
        }
    }
}
#[component]
pub fn LanguagePicker() -> Element {
    let current = i18n::language();
    rsx! {
        label { class: "language-picker",
            span { {t("Language")} }
            select {
                aria_label: t("Language"),
                value: current.code(),
                onchange: move | e |
                        i18n::set_language(i18n::Language::parse(& e.value())),
                for lang in i18n::Language::ALL {
                    option { value: lang.code(), selected: lang==current, "{lang.name()}" }
                }
            }
        }
    }
}
#[component]
pub fn Back() -> Element {
    rsx! {
        button {
            class: "back-link glass-capsule",
            disabled: *BUSY.read(),
            onclick: move |_| nav(Screen::MainMenu),
            Icon { name: "back" }
            {t("Home")}
        }
    }
}
#[component]
pub fn CardFace(card: Card) -> Element {
    let value = match card.value {
        uno_core::Value::Skip => "S".into(),
        uno_core::Value::Reverse => "R".into(),
        uno_core::Value::Wild => "W".into(),
        _ => card_value(card),
    };
    rsx! {
        div { class: "card-art {color_name(card.color)}",
            span { class: "card-corner", "{value}" }
            span { class: "card-center",
                match card.value {
                    uno_core::Value::Skip => rsx! {
                        svg {
                            view_box: "0 0 40 40",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "4",
                            circle { cx: "20", cy: "20", r: "13" }
                            path { d: "M10 30 30 10" }
                        }
                    },
                    uno_core::Value::Reverse => rsx! {
                        svg {
                            view_box: "0 0 40 40",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "4",
                            stroke_linejoin: "round",
                            path { d: "M8 16h24l-8-8M32 24H8l8 8" }
                        }
                    },
                    _ => rsx! { "{value}" },
                }
            }
            span { class: "card-corner bottom", "{value}" }
        }
    }
}
#[component]
pub fn ColorPicker(
    onchoose: EventHandler<Color>,
    oncancel: EventHandler<()>,
    #[props(default = false)] warning: bool,
) -> Element {
    use_effect(move || crate::browser::focus(".color-dialog button"));
    use_drop(move || crate::browser::focus(".hand-card.selected"));
    rsx! {
        div {
            class: "modal-backdrop",
            onkeydown: move |e| {
                crate::browser::trap_focus(&e); if e.key() == Key::Escape {
                    oncancel.call(());
                }
            },
            section {
                class: "color-dialog glass",
                role: "dialog",
                aria_modal: "true",
                aria_label: t("Choose a color"),
                h2 { {t("Choose a color")} }
                p { class: "muted", {t("Your card stays in your hand until you choose.")} }
                if warning {
                    p { class: "uno-warning",
                        {t("You have the active color. If your +4 is challenged, you draw four cards.")}
                    }
                }
                div { class: "color-choices",
                    for color in Color::ALL {
                        button {
                            class: "color-choice {color_name(Some(color))}",
                            autofocus: color == Color::Red,
                            onclick: move |_| onchoose.call(color),
                            "{t(color_name(Some(color)))}"
                        }
                    }
                }
                button { class: "text-button", onclick: move |_| oncancel.call(()), {t("Cancel")} }
            }
        }
    }
}
#[component]
pub fn Rules() -> Element {
    rsx! {
        details { class: "club-rules uno-rules",
            summary { {t("How to play · UnoGo")} }
            div { class: "rules-copy",
                p{{t("New rounds use Friends rules. Existing saved rounds keep their original Classic rules.")}}
                p{{t("Tap same-number cards to select them together. At least one must match the pile by color or number; the last sets the next color. Tap a different number to replace the selection. Grouped 0s or 5s resolve once.")}}
                p{{t("After a 0, choose Swap hands or Keep my cards. A swap exchanges your remaining hand with any opponent. No choice within 15 seconds keeps your cards. Playing your last 0 wins without a swap.")}}
                p{{t("After a 5, everyone taps the center card after the shared countdown. The last tap draws one card. Taps are ordered by server arrival; duplicate taps do not count.")}}
                p{{t("The tapping window lasts 6 seconds. Missing players count as last; if several do not tap, one is chosen randomly for the single penalty. The game then continues.")}}

                p {
                    {
                        t(
                            "Match the color, number or action. On a red 7, play any red card or any 7. A Wild chooses the next color.",
                        )
                    }
                }
                p {
                    {
                        t(
                            "You may draw one instead. Only that new card can be played; otherwise keep it and end your turn. Penalties cannot be stacked.",
                        )
                    }
                }
                p {
                    {
                        t(
                            "With two cards, arm UNO before playing. If you forget, call UNO during the two-second window before someone catches you for two cards.",
                        )
                    }
                }
                p {
                    {
                        t(
                            "Play +4 only without a card matching the active color. A successful challenge gives the sender four cards; a failed challenge gives you six.",
                        )
                    }
                }
                p {
                    {
                        t(
                            "Turns last 30 seconds. A +4 response lasts 8 seconds. A timeout draws and keeps, or accepts +4. The first empty hand wins after final penalties resolve.",
                        )
                    }
                }
            }
        }
    }
}

#[component]
pub fn ChallengeReceipt() -> Element {
    use_effect(move || crate::browser::focus(".challenge-dialog button"));
    let Some(receipt) = RECEIPT.read().clone() else {
        return rsx! {};
    };
    rsx! {
                    div { class: "modal-backdrop",onkeydown:move|e|crate::browser::trap_focus(&e),
                        section {
                            class: "challenge-dialog glass",
                            role: "dialog",
                            aria_modal: "true",
                            aria_label: t("Challenge result"),
                            h2 {
                                if receipt["challenge_succeeded"] == true {
                                    {t("Challenge successful")}
                                } else {
                                    {t("Challenge unsuccessful")}
                                }
                            }
                            p { {t("Only you can see the sender's hand before the +4.")} }
                            div { class: "evidence-hand",
                                if let Some(cards) = receipt["challenge_evidence"].as_array() {
                                    for card in cards {
                                        if let Ok(card) = serde_json::from_value(card.clone()) {
                                            CardFace { card }
                                        }
                                    }
                                }
                            }
                            button {
                                autofocus: true,
                                onclick: move | _ | RECEIPT
                                        .signal().set(None),
                                {t("Continue")}
                            }
                        }
                    }

    }
}
