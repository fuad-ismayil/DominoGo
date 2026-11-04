use crate::i18n::t;
use dioxus::prelude::*;
#[component]
pub fn Rules() -> Element {
    rsx! { details { class: "rules", summary { {t("How to play · 101 dominoes")} }
        p { {t("Two to four players share a double-six set. Each player receives seven tiles. Play a tile matching either open end of the chain. Empty your hand to win the round.")} }
        p { {t("Opening: in round one, the player holding the lowest-priority double starts. The order is 1–1, 2–2, 3–3, 4–4, 5–5, 6–6, then 0–0. Play your lowest double first. Later rounds start with the previous winner; any tile can open.")} }
        p { {t("Your host chooses the draw rule when creating the table. Free drawing allows drawing even with a matching tile; otherwise you must play when you can. The rule stays fixed throughout the match. Draw from the market: two tiles per draw for two players, one for three, none for four. Drawing keeps your turn. You can pass only when you have no matching tile and the market is empty (or there is no market). If everyone passes, the lowest hand pip total wins; a tie goes to the earliest player in the table order.")} }
        p { {t("Scoring: the winner collects the pips remaining in every other hand. A lone 0–0 counts as 10. Your first points remain pending until consecutive round wins take the total above 12; those points become permanent. Losing a round clears pending points. Once you have permanent points, new wins bank directly. First to 101 permanent points wins the match.")} }
        p { {t("On touchscreens or with a keyboard, select a highlighted tile and choose Left or Right. The server checks every move. Use the table code to invite friends, add bots to practice, and request a pause to save a match for later.")} }
    } }
}
