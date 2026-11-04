//! Ivory tile rendering shared by the hand, board and pointer ghost.
use dioxus::prelude::*;

#[component]
pub fn TileFace(value: [u8; 2], #[props(default = false)] horizontal: bool) -> Element {
    rsx! { div { class: if horizontal { "ivory-tile ivory-horizontal" } else { "ivory-tile" },
        for value in value { div { class: "ivory-half", for i in 0..9 { span { class: if pip(value,i) { "ivory-pip ink" } else { "ivory-pip" } } } } }
    } }
}

#[component]
pub fn BoardTile(value: [u8; 2], vertical: bool, reversed: bool) -> Element {
    let (w, h) = if vertical { (1., 2.) } else { (2., 1.) };
    let transform = if reversed {
        format!("rotate(180 {} {})", w / 2., h / 2.)
    } else {
        String::new()
    };
    rsx! { g { transform,
        rect { x: "0", y: "0.07", width: "{w}", height: "{h}", rx: "0.12", fill: "#a69877", stroke: "#0d2118", stroke_width: "0.025" }
        rect { x: "0", y: "0", width: "{w}", height: "{h}", rx: "0.12", fill: "url(#ivory-face)", stroke: "#d3c5a7", stroke_width: "0.025" }
        rect { x: "0.045", y: "0.045", width: "{w-0.09}", height: "{h-0.09}", rx: "0.09", fill: "none", stroke: "#fffdf5", stroke_width: "0.025", opacity: "0.7" }
        if vertical { path { d: "M.12 1 H.88", stroke: "#af9e7e", stroke_width: ".025" } } else { path { d: "M1 .12 V.88", stroke: "#af9e7e", stroke_width: ".025" } }
        for (half,count) in value.iter().enumerate() { for i in 0..9 { if pip(*count,i) {
            circle { cx: (0.23 + (i%3) as f64 * 0.27 + if vertical {0.0} else {half as f64}).to_string(), cy: (0.23 + (i/3) as f64 * 0.27 + if vertical {half as f64} else {0.0}).to_string(), r: ".082", fill: "url(#pip-ink)" }
        } } }
        circle { cx: "{w/2.}", cy: "{h/2.}", r: ".035", fill: "#c3b28e" }
    } }
}
fn pip(value: u8, index: usize) -> bool {
    match value {
        1 => index == 4,
        2 => [0, 8].contains(&index),
        3 => [0, 4, 8].contains(&index),
        4 => [0, 2, 6, 8].contains(&index),
        5 => [0, 2, 4, 6, 8].contains(&index),
        6 => [0, 2, 3, 5, 6, 8].contains(&index),
        _ => false,
    }
}
