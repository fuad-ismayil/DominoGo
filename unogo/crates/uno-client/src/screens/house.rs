use crate::{browser, i18n::t, store::*, ui::*};
use dioxus::prelude::*;
use uno_core::{Action, Card};

#[component]
pub(super) fn TouchPile(
    card: Card,
    challenge: u64,
    starts_at: u64,
    taps: Vec<usize>,
    local: usize,
    paused: bool,
) -> Element {
    let mut now = use_signal(browser::now);
    use_future(move || async move {
        loop {
            browser::sleep(60).await;
            now.set(browser::now());
        }
    });
    let server_now = (now() as i64 - *CLOCK_OFFSET.read()).max(0) as u64;
    let ready = server_now >= starts_at;
    let place = taps.iter().position(|s| *s == local);
    rsx! {
        button {
            class:if place.is_some(){"discard-pile touch-target tapped"}else if ready{"discard-pile touch-target ready"}else{"discard-pile touch-target counting"},
            aria_label:t("Tap the 5"),
            disabled:paused||!ready||place.is_some()||*BUSY.read()||!*CONNECTED.read(),
            onclick:move |_|command(Action::TouchFive{challenge}),
            CardFace{key:"{card.id}",card}
            if let Some(place)=place{span{class:"touch-stamp","✓ {place+1}"}}
        }
        span {class:"pile-label touch-hint",aria_live:"polite",
            if let Some(place)=place{"{t(\"Tapped\")} · {place+1}"}
            else if ready{{t("Tap now!")}}
            else{"{t(\"Get ready\")} {starts_at.saturating_sub(server_now).div_ceil(1000)}"}
        }
    }
}

#[component]
pub(super) fn SwapPicker(onclose: EventHandler<()>) -> Element {
    use_effect(|| browser::focus(".swap-dialog button"));
    use_drop(|| browser::focus("#swap-hands"));
    let Some(table) = TABLE.read().clone() else {
        return rsx! {};
    };
    let Some(game) = table.game.as_ref() else {
        return rsx! {};
    };
    rsx! {
        div {class:"modal-backdrop",onkeydown:move|e|{browser::trap_focus(&e);if e.key()==Key::Escape{onclose.call(());}},
            section{class:"swap-dialog glass",role:"dialog",aria_modal:"true",aria_label:t("Choose a player"),
                div{class:"section-heading",h2{{t("Choose a player")}}
                    button{class:"round-control glass-circle",aria_label:t("Close"),onclick:move |_|onclose.call(()),Icon{name:"close"}}
                }
                p{{t("Swap your remaining hand with one opponent. You can also keep your cards.")}}
                div{class:"swap-targets",
                    for (seat,player) in table.players.iter().enumerate().filter(|(seat,_)|*seat!=table.local_seat){
                        button{class:"swap-player secondary",disabled:*BUSY.read(),onclick:move |_|command(Action::SwapHands{target:seat}),
                            strong{"{player.name}"}
                            span{"{game.counts[seat]} {t(\"cards\")}"}
                        }
                    }
                }
                button{class:"text-button",disabled:*BUSY.read(),onclick:move |_|command(Action::KeepHand),{t("Keep my cards")}}
            }
        }
    }
}

#[component]
pub(super) fn NumberOrder(
    cards: Vec<Card>,
    valid: bool,
    onplay: EventHandler<()>,
    onearlier: EventHandler<usize>,
) -> Element {
    rsx! {
        div{class:"number-group",
            ol{class:"number-order",aria_label:t("Playing order"),
                for (index,card) in cards.iter().enumerate(){
                    li{class:"queue-chip {crate::model::color_name(card.color)}",
                        span{class:"queue-rank","{index+1}"}
                        span{"{t(crate::model::color_name(card.color))} {crate::model::card_value(*card)}"}
                        button{class:"queue-earlier",aria_label:format!("{} {}",t("Move earlier"),index+1),disabled:index==0||*BUSY.read(),onclick:move |_|onearlier.call(index),Icon{name:"back"}}
                    }
                }
            }
            button{class:"play-group",disabled:!valid||*BUSY.read(),onclick:move |_|onplay.call(()),"{t(\"Play\")} {cards.len()}"}
        }
        if !valid{small{class:"group-hint",{t("At least one selected card must match the pile.")}}}
    }
}
