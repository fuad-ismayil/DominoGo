//! One score presentation for seats, results and history. Pending points
//! remain visible even when they have not qualified for banking yet.
use crate::i18n::{t, tf};
use crate::types::SScore;
use dioxus::prelude::*;
#[component]
pub fn ScoreReadout(score: SScore, #[props(default = true)] target: bool) -> Element {
    rsx! {span {class:"score-readout",aria_live:"polite",aria_atomic:"true",aria_label:tf("{score.permanent} banked points, {score.temporary} pending points", &[("score.permanent", format!("{}", (score.permanent))), ("score.temporary", format!("{}", (score.temporary)))]),
        strong {class:"score-bank","{score.permanent}"}
        if target {small {class:"score-target","/ 101"}}
        if score.temporary>0 {span {class:"score-pending",title:t("Pending points bank when consecutive wins total more than 12"), "+{score.temporary}" small {class:"score-pending-word",{t("pending")}}}}
    }}
}
