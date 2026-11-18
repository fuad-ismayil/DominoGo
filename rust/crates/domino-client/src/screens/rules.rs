use crate::i18n::t;
use dioxus::prelude::*;
#[component]
pub fn Rules() -> Element {
    if crate::store::GAME_STATE
        .read()
        .as_ref()
        .is_some_and(|s| s.mode == "telefon")
    {
        return rsx! {details {class:"rules",summary {{t("How to play · 5-lik (Telefon)")}}
            p {{t("Each player receives seven tiles. Round one opens with 2–3 and scores five. Redeal if 2–3 is in the market. Later rounds open with any tile, played by the previous round winner.")}}
            p {{t("After each placement, add all exposed ends. A positive multiple of five scores that total immediately. An exposed double counts both halves. Unused side branches count zero.")}}
            p {{t("Any double connected on both normal sides may take a sideways tile, but opening that branch must score a multiple of five. Match its number; later tiles can extend the branch normally. Each double has two side branches.")}}
            p {{t("Scores are individual. Empty your hand to win the round and collect the other hands, rounded to the nearest five. A lone 0–0 counts ten. When everyone passes, the lowest hand wins; table order breaks ties. First to at least 365 wins immediately.")}}
            p {{t("The host fixes the draw rule. Draw two tiles for two players, one for three, none for four; drawing keeps your turn. Pass only without a legal move and without drawable market tiles.")}}
        }};
    }
    rsx! { details { class: "rules", summary { {t("How to play · 101 dominoes")} }
        p { {t("Two to four players share a double-six set. Each player receives seven tiles. Play a tile matching either open end of the chain. Empty your hand to win the round.")} }
        p { {t("Opening: in round one, the player holding the lowest-priority double starts. The order is 1–1, 2–2, 3–3, 4–4, 5–5, 6–6, then 0–0. Play your lowest double first. Later rounds start with the previous winner; any tile can open.")} }
        p { {t("Your host chooses the draw rule when creating the table. Free drawing allows drawing even with a matching tile; otherwise you must play when you can. The rule stays fixed throughout the match. Draw from the market: two tiles per draw for two players, one for three, none for four. Drawing keeps your turn. You can pass only when you have no matching tile and the market is empty (or there is no market). If everyone passes, the lowest hand pip total wins; a tie goes to the earliest player in the table order.")} }
        p { {t("Scoring: the winner collects the pips remaining in every other hand. A lone 0–0 counts as 10. Your first points remain pending until consecutive round wins take the total above 12; those points become permanent. Losing a round clears pending points. Once you have permanent points, new wins bank directly. First to 101 permanent points wins the match.")} }
        p { {t("On touchscreens or with a keyboard, select a highlighted tile and choose Left or Right. The server checks every move. Use the table code to invite friends, add bots to practice, and request a pause to save a match for later.")} }
    } }
}
