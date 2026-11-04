use crate::i18n::t;
use dioxus::prelude::*;

#[component]
pub fn Icon(name: &'static str) -> Element {
    let path = match name {
        "plus" => "M12 5v14M5 12h14",
        "people" => "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2M16 3a4 4 0 0 1 0 8M22 21v-2a4 4 0 0 0-3-3.87M13 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z",
        "trophy" => "M8 3h8v6a4 4 0 0 1-8 0V3ZM8 5H4v2a4 4 0 0 0 4 4M16 5h4v2a4 4 0 0 1-4 4M12 13v6M8 21h8M9 19h6",
        "home" => "m3 10 9-7 9 7v10a1 1 0 0 1-1 1h-5v-7H9v7H4a1 1 0 0 1-1-1V10Z",
        "arrow" => "M5 12h14m-6-6 6 6-6 6",
        "back" => "M19 12H5m6-6-6 6 6 6",
        "history" => "M3 11a9 9 0 1 1 2.5 7M3 4v7h7M12 7v5l3 2",
        "user" => "M20 21v-2a7 7 0 0 0-14 0v2M17 7a5 5 0 1 1-10 0 5 5 0 0 1 10 0Z",
        "close" => "m6 6 12 12M6 18 18 6",
        "copy" => "M9 9h12v12H9V9ZM15 5V3H3v12h2",
        "refresh" => "M20 7v5h-5M4 17v-5h5M6 6a8 8 0 0 1 14 6M4 12a8 8 0 0 0 14 6",
        "shield" => "m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6l8-3ZM8 12l3 3 5-6",
        "bot" => "M5 7h14v13H5V7ZM12 3v4M2 11v5M22 11v5M8 11h1M15 11h1M9 16h6",
        "link" => "M10 13a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-2 2M14 11a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l2-2",
        "key" => "M15 3a6 6 0 1 1-4.7 9.7L3 20v-4l5.3-5.3A6 6 0 0 1 15 3ZM17 7h.01",
        "logout" => "M9 3H3v18h6M13 8l5 4-5 4M7 12h13",
        _ => "M12 5v14M5 12h14",
    };
    rsx! { svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.7", stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true", path { d: path } } }
}

#[component]
pub fn BackButton() -> Element {
    rsx! { button { class: "back-link glass-capsule", onclick: move |_| crate::store::nav(crate::store::Screen::MainMenu), Icon { name: "back" } {t("Home")} } }
}

/// Scalable ivory and gold domino emblem; no font-dependent symbols.
#[component]
pub fn BrandLogo() -> Element {
    rsx! {
        span { class: "brand-mark", aria_hidden: "true",
            svg { view_box: "0 0 58 58", fill: "none",
                g { transform: "translate(33 28) rotate(22)",
                    rect { x: "-11", y: "-22", width: "23", height: "45", rx: "6", fill: "#002e24", transform: "translate(1 3)" }
                    rect { x: "-12", y: "-23", width: "23", height: "45", rx: "6", fill: "#eabb65", stroke: "#ffe5a7", stroke_width: "1" }
                    path { d: "M-8 0H7", stroke: "#866325", stroke_opacity: ".45" }
                    circle { cx: "-5", cy: "-15", r: "2", fill: "#174a38" }
                    circle { cx: "4", cy: "-6", r: "2", fill: "#174a38" }
                    circle { cx: "-5", cy: "7", r: "2", fill: "#174a38" }
                    circle { cx: "4", cy: "16", r: "2", fill: "#174a38" }
                }
                g { transform: "translate(22 30) rotate(-15)",
                    rect { x: "-11", y: "-21", width: "24", height: "44", rx: "6", fill: "#001f19", fill_opacity: ".4" }
                    rect { x: "-12", y: "-23", width: "24", height: "44", rx: "6", fill: "#fff4d9", stroke: "#fffdf2", stroke_width: "1" }
                    path { d: "M-8-1H8", stroke: "#57785f", stroke_opacity: ".35" }
                    path { d: "M-8-17V-18Q-8-20-5-20H6", stroke: "white", stroke_width: "1.5", stroke_linecap: "round" }
                    for (x,y) in [(-5,-16),(0,-11),(5,-6),(-5,6),(5,6),(-5,15),(5,15)] {
                        circle { cx: "{x}", cy: "{y}", r: "2.2", fill: "#164c38" }
                    }
                }
            }
        }
        span { class: "brand-wordmark", "DOMINO" span { class: "gold", "GO" } }
    }
}

#[component]
pub fn LanguagePicker() -> Element {
    let selected = crate::i18n::language();
    rsx! {label {class:"language-picker",span {{t("Language")}}
        select {aria_label:t("Language"),value:selected.code(),onchange:move |e|crate::i18n::set_language(crate::i18n::Language::parse(&e.value())),
            for language in crate::i18n::Language::ALL {option {value:language.code(),selected:language==selected,"{language.name()}"}}
        }
    }}
}
