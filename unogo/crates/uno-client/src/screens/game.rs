use crate::{
    browser,
    i18n::t,
    model::{card_value, color_name},
    store::*,
    ui::*,
};
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use serde_json::json;
use uno_core::{Action, Phase, Value};
fn play_group(cards: Vec<u8>) {
    if *BUSY.read() || !*CONNECTED.read() {
        return;
    }
    let Some(table) = TABLE.read().clone() else {
        return;
    };
    let Some(game) = table.game else { return };
    if !uno_core::number_group_playable(&game, &cards) {
        return;
    }
    if cards.len() == 1 {
        command(Action::Play {
            card: cards[0],
            color: None,
            uno: *ARMED.read(),
        });
    } else {
        command(Action::PlayNumbers {
            cards,
            uno: *ARMED.read(),
        });
    }
}
fn play_card(
    card: uno_core::Card,
    mut choosing: Signal<Option<u8>>,
    mut selected: Signal<Option<u8>>,
) {
    if !crate::motion::playable(card.id) {
        return;
    }
    if card.color.is_none() {
        choosing.set(Some(card.id));
    } else {
        command(Action::Play {
            card: card.id,
            color: None,
            uno: *ARMED.read(),
        });
        selected.set(None);
    }
}
#[component]
fn Clock() -> Element {
    let mut time = use_signal(browser::now);
    use_future(move || async move {
        loop {
            browser::sleep(200).await;
            time.set(browser::now());
        }
    });
    let deadline = TABLE
        .read()
        .as_ref()
        .and_then(|t| t.game.as_ref())
        .and_then(|g| g.deadline);
    let now = (time() as i64 - *CLOCK_OFFSET.read()).max(0) as u64;
    let remaining = deadline.map(|d| d.saturating_sub(now).div_ceil(1000));
    rsx! {
        span {
            class: if remaining.is_some_and(|n| n <= 5) { "turn-clock urgent" } else { "turn-clock" },
            aria_label: t("Time remaining"),
            if let Some(seconds) = remaining {
                "{seconds}s"
            } else {
                {t("Paused")}
            }
        }
    }
}
#[component]
pub fn GameBoard() -> Element {
    use_drop(crate::motion::cancel);
    use_future(move || async move {
        refresh_reactions().await;
    });
    let mut selected = use_signal(|| None::<u8>);
    let mut choosing = use_signal(|| None::<u8>);
    let mut opening = use_signal(|| false);
    let mut group = use_signal(Vec::<u8>::new);
    let mut swap_picker = use_signal(|| false);
    let mut previous_sequence = use_signal(|| 0u64);
    let mut previous_match = use_signal(|| None::<String>);
    use_effect(move || {
        if let Some(table) = TABLE.read().clone() {
            if previous_match() != table.match_id || previous_sequence() != table.accepted_sequence
            {
                group.set(vec![]);
                previous_sequence.set(table.accepted_sequence);
                selected.set(None);
                choosing.set(None);
                opening.set(false);
                previous_match.set(table.match_id.clone());
            }
            if table.game.as_ref().is_some_and(|g| {
                g.turn != table.local_seat
                    || g.paused
                    || !matches!(g.phase, Phase::Turn | Phase::Drawn { .. })
            }) {
                selected.set(None);
                choosing.set(None);
                opening.set(false);
                group.set(vec![]);
            }
            if !table
                .game
                .as_ref()
                .is_some_and(|g| matches!(g.phase, Phase::Swap { .. }))
            {
                swap_picker.set(false);
            }
        }
    });
    let Some(table) = TABLE.read().clone() else {
        return rsx! {};
    };
    let Some(g) = table.game.clone() else {
        return rsx! {};
    };
    let usage = REACTION_USAGE.read().clone();
    let mut reactions = usage["order"].as_array().cloned().unwrap_or_default();
    reactions.sort_by_key(|emoji| {
        std::cmp::Reverse(
            usage["counts"][emoji.as_str().unwrap_or_default()]
                .as_u64()
                .unwrap_or(0),
        )
    });
    let local = table.local_seat;
    let host = USER.read().as_ref().is_some_and(|u| u.id == table.host_id);
    let actionable = g.turn == local && !g.paused && !*BUSY.read() && *CONNECTED.read();
    let can_play = actionable && matches!(g.phase, Phase::Turn | Phase::Drawn { .. });
    let drawn = matches!(g.phase, Phase::Drawn { .. });
    let group_ids = group();
    let group_cards: Vec<_> = group_ids
        .iter()
        .filter_map(|id| g.hand.iter().find(|c| c.id == *id).copied())
        .collect();
    let group_valid = can_play && uno_core::number_group_playable(&g, &group_ids);
    let selection_ids = if group_ids.is_empty() {
        selected().into_iter().collect()
    } else {
        group_ids.clone()
    };
    let votes = table.pause_votes.clone().unwrap_or_default();
    let voted = USER.read().as_ref().is_some_and(|u| votes.contains(&u.id));
    let phase_label = match g.phase {
        Phase::Turn if g.turn == local => t("Your turn"),
        Phase::Drawn { .. } if g.turn == local => t("Play the new card or keep it"),
        Phase::Uno { player } => {
            format!("{} · {}", table.players[player].name, t("UNO window"))
        }
        Phase::Challenge { .. } if g.turn == local => t("Accept +4 or challenge?"),
        Phase::OpeningColor if g.turn == local => t("Choose the opening color"),
        Phase::Finished { .. } => t("Round complete"),
        Phase::Swap { player } => {
            if player == local {
                t("Swap hands or keep your cards?")
            } else {
                format!(
                    "{} · {}",
                    table.players[player].name,
                    t("choosing a hand swap")
                )
            }
        }
        Phase::TouchFive { .. } => t("Everyone, touch the 5!"),
        _ => format!("{} · {}", table.players[g.turn].name, t("their turn")),
    };
    if let Phase::Finished { winner } = g.phase {
        let summary = json!(
            { "players" : table.players, "counts" : g.counts, "winner" : winner }
        );
        return rsx! {
            section { class: "results-plane glass",
                h1 {
                    if let Some(winner) = winner {
                        "{table.players[winner].name} · {t(\"wins!\")}"
                    } else {
                        {t("No contest")}
                    }
                }
                p { class: "muted", {t("All final-card penalties have resolved.")} }
                super::ResultRows { summary }
                div { class: "finishing-card",
                    CardFace { key: "{g.top.id}", card: g.top }
                }
                div { class: "result-actions",
                    if host {
                        button {
                            disabled: *
                                        BUSY.read(),
                            onclick: move |_| room_control("rematch"),
                            { t("Play again")
                                        }
                        }
                    } else {
                        p { class: "muted", {t("Waiting for your host to start another round.")} }
                    }
                    button {
                        class: "secondary",
                        disabled: *BUSY.read(),
                        onclick: move |_| {
                            browser::remove("unogo.table");
                            nav(Screen::MainMenu);
                        },
                        {t("Back home")}
                    }
                }
            }
        };
    }
    rsx! {
        section { class: "game-board",
            onkeydown: move |e| { if e.key()==Key::Escape {crate::motion::cancel();selected.set(None);group.set(vec![]);} },
            div { class: "match-bar",
                span { class: if *CONNECTED.read() { "connection-status online" } else { "connection-status" },
                    if *CONNECTED.read() {
                        {t("Connected")}
                    } else {
                        {t("Reconnecting…")}
                    }
                }
                span { class: "table-code-label", "{table.id}" }
                Clock {}
                button {
                    class: "text-button",
                    disabled: *BUSY.read(),
                    onclick: move | _ |
                            nav(Screen::MainMenu),
                    {t("Home")}
                }
                button {
                    class: "secondary compact-button",
                    disabled: * BUSY.read() || g.paused ||!* CONNECTED
                            .read(),
                    onclick: move |_| room_control(if voted { "cancel-pause" } else { "pause" }),
                    if voted {
                        {t("Cancel pause")}
                    } else {
                        {t("Pause")}
                    }
                }
            }
            div {
                class: if table.players.len() > 6 { "opponents many-seats" } else { "opponents" },
                aria_label: t("Players"),
                for (seat, p) in table.players.iter().enumerate().filter(|(i, _)| *i != local) {
                    div {
                        class: if seat == g.turn { "opponent active" } else { "opponent" },
                        title: p.name.clone(),
                        "data-seat": "{seat}",
                        span {class:"opponent-stack",aria_hidden:"true",i{} i{} i{}}
                        span { class: "opponent-avatar glass-circle",
                            if p.user_id.is_none() {
                                Icon { name: "bot" }
                            } else {
                                "{p.name.chars().next().unwrap_or('U').to_uppercase()}"
                            }
                        }
                        div {
                            strong { "{p.name}" }
                            span { "{g.counts[seat]} {t(\"cards\")}" }
                        }
                        if let Phase::TouchFive{ref taps,..}=g.phase{
                            if let Some(rank)=taps.iter().position(|s|*s==seat){span{class:"tap-rank","{rank+1}"}}
                        }
                        if g.counts[seat] == 1 {
                            span { class: "uno-badge", "UNO" }
                        }
                    }
                }
            }
            div { class: "board-status",
                h1 { "{phase_label}" }
                p { class: "active-color",
                    span { class: "color-dot {color_name(Some(g.color))}" }
                    "{t(\"Active color\")}: {t(color_name(Some(g.color)))}"
                    span { class: "direction-label",
                        if g.direction == 1 {
                            {t("Clockwise")}
                        } else {
                            {t("Counterclockwise")}
                        }
                    }
                }
                if let Phase::TouchFive{ref taps,..}=g.phase{small{class:"tap-progress","{taps.len()} / {table.players.len()} " {t("tapped")}}}
                if let Some(result)=g.last_touch.as_ref().filter(|r|table.server_now.unwrap_or(0).saturating_sub(r.resolved_at)<4000){
                    small{class:"tap-outcome",role:"status",
                        if result.drawn==0{{t("No penalty card was available.")}}
                        else{"{table.players[result.penalty].name} · " {t("drew one card")}}
                    }
                }
                if g.pile_empty {
                    p { class: "pile-notice", role: "status",
                        {t("The draw pile is empty. Available cards have been drawn.")}
                    }
                }
            }
            div { class: "card-table",
                div { class: "pile-group draw-group",
                    button {
                        class: "draw-pile",
                        aria_label: t(if matches!(g.phase,Phase::Drawn{..}){"Keep card"}else{"Draw one card"}),
                        disabled: !actionable || !matches!(g.phase, Phase::Turn|Phase::Drawn{..}),
                        onclick: move | _ |
                                command(if drawn{Action::Keep}else{Action::Draw}),
                        span { class: "deck-emblem", img{src:"/brand/unogo-mark-v2.svg",alt:"",draggable:"false"} }
                        span { class: "deck-count", "{g.draw_count}" }
                    }
                    span { class: "pile-label", { t(if actionable && matches!(g.phase,Phase::Drawn{..}){"Keep card"}else if actionable && matches!(g.phase, Phase::Turn) {"Tap to draw"} else {"Draw pile"})
                            } }
                }
                div { class: "pile-group",
                    if let Phase::TouchFive{challenge,starts_at,ref taps,..}=g.phase{
                        super::house::TouchPile{card:g.top,challenge,starts_at,taps:taps.clone(),local,paused:g.paused}
                    }else{
                        div { class: "discard-pile",CardFace { key: "{g.top.id}", card: g.top } }
                        span { class: "pile-label", {t("Discard")} }
                    }
                }
            }

            div { class: if matches!(g.phase,Phase::Swap{..}){"decision-area swap-decision"}else{"decision-area"}, aria_live: "polite",
                match g.phase {
                    Phase::Uno { player } => rsx! {
                        p { {t("Two seconds to call UNO or catch the missed call.")} }
                        button {
                            class: "uno-call",
                            disabled: *BUSY.read() || !*CONNECTED.read(),
                            onclick: move
                                    | _ | command(if player == local { Action::CallUno } else { Action::CatchUno }),
                            if player == local {
                                {t("Call UNO!")}
                            } else {
                                {t("Catch UNO!")}
                            }
                        }
                    },
                    Phase::Challenge { .. } if actionable => rsx! {
                        p { {t("Challenge only if you think the sender had the active color.")} }
                        button { disabled: *BUSY.read(), onclick: move |_| command(Action::AcceptFour), {t("Accept +4")} }
                        button {
                            class: "secondary",
                            disabled: *BUSY.read(),
                            onclick: move |_| command(Action::ChallengeFour),
                            {t("Challenge")}
                        }
                    },
                    Phase::Swap{player} if player==local => rsx!{
                        button{id:"swap-hands",disabled:*BUSY.read()||g.paused,onclick:move |_|swap_picker.set(true),{t("Swap hands")}}
                        button{class:"secondary",disabled:*BUSY.read()||g.paused,onclick:move |_|command(Action::KeepHand),{t("Keep my cards")}}
                    },
                    Phase::OpeningColor if actionable => rsx! {
                        button { onclick: move | _ | opening
                                    .set(true), {t("Choose a color")} }
                    },
                    _ => rsx! {},
                }
                if group_ids.len()>1 && matches!(g.phase,Phase::Turn){
                    super::house::NumberOrder{cards:group_cards.clone(),valid:group_valid,onplay:move |_|play_group(group()),onearlier:move |index:usize|{let mut ids=group();if index>0&&index<ids.len(){ids.swap(index,index-1);group.set(ids);}}}
                }
                if !votes.is_empty() && !g.paused {
                    small { class: "pause-vote",
                        {t("Pause requested")}
                        " · {votes.len()} / {table.players.iter().filter(|p|p.user_id.is_some()).count()}"
                    }
                }
            }

            div { class: "hand-tray",
                div { class: "hand-heading",
                    h2 {
                        {t("Your hand")}
                        span { "{g.hand.len()} {t(\"cards\")}" }
                    }
                    div{class:"hand-tools",
                        if g.hand.len()==2 || (group_ids.len()>1 && group_ids.len()+1==g.hand.len()){
                            button{class:"uno-arm",aria_pressed:*ARMED.read(),title:t(if *ARMED.read(){"UNO is armed for your next play. Tap to cancel."}else{"Declare UNO before playing down to one card."}),disabled:!can_play,onclick:move |_|{let value=!*ARMED.read();ARMED.signal().set(value);},
                                if *ARMED.read(){svg{view_box:"0 0 16 16","aria-hidden":"true",path{d:"M3 8 6.5 11.5 13 5",fill:"none",stroke:"currentColor",stroke_width:"2",stroke_linecap:"round",stroke_linejoin:"round"}}}
                                "UNO!"
                            }
                        }

                    }
                }
                div {
                    class: "hand-track",
                    role: "group",
                    aria_label: t("Your hand"),
                    for (card_index,card) in g.hand.iter().copied().enumerate() {
                        {
                            let legal = can_play && g.playable.contains(&card.id);
                            let queue_index=group_ids.iter().position(|id|*id==card.id);
                            let is_selected = selected()==Some(card.id)||queue_index.is_some();
                            let next_selection=if can_play{crate::model::select_cards(&g,&selection_ids,card.id)}else{vec![card.id]};
                            let bundle_member=group_ids.len()>1&&queue_index.is_some();
                            let shown_legal=legal||(can_play&&next_selection.len()>selection_ids.len());
                            let spread=(g.hand.len().saturating_sub(1) as f64/2.).max(1.);
                            let fan=(card_index as f64-spread)/spread;
                            let card_style=format!("--card-angle:{}deg;--card-arc:{}px;--card-z:{}",fan*7.,fan.abs()*8.,card_index+1);
                            let label = format!(
                                "{} {}{}",
                                t(color_name(card.color)),
                                t(&card_value(card)),
                                if legal { format!(", {}", t("playable")) } else { String::new() },
                            );
                            rsx! {
                                button {
                            key: "{card.id}",
                            "data-card-id":"{card.id}",
                                    class: if is_selected { "hand-card selected" } else if shown_legal { "hand-card playable" } else { "hand-card" },
                                    aria_label: label,
                                    aria_pressed: is_selected,
                                    style: card_style,
                                    disabled: *BUSY.read() || !*CONNECTED.read(),
                                    onpointerdown: move |e| {
                                        if e.is_primary() && e.trigger_button()==Some(MouseButton::Primary) {
                                            let p=e.client_coordinates();
                                            let bundle=if bundle_member{group()}else{vec![]};
                                            let allowed=if bundle_member{group_valid}else{legal};
                                            crate::motion::start(card.id,e.pointer_id(),p.x,p.y,allowed,bundle);
                                        }
                                    },
                                    onpointermove: move |e| {let p=e.client_coordinates();crate::motion::move_pointer(e.pointer_id(),p.x,p.y);},
                                    onpointerup: move |e| {
                                        let p=e.client_coordinates();
                                        if crate::motion::finish(e.pointer_id(),p.x,p.y,false).is_some() {if bundle_member{play_group(group());}else{play_card(card,choosing,selected);}}
                                    },
                                    onpointercancel: move |e| {crate::motion::finish(e.pointer_id(),-1000.,-1000.,true);},
                                    onlostpointercapture: move |e| {crate::motion::finish(e.pointer_id(),-1000.,-1000.,true);},
                                    oncontextmenu: move |e| e.prevent_default(),
                                    ondragstart: move |e| e.prevent_default(),
                                    onclick: move |_| {
                                        if crate::motion::active()||crate::motion::consume_click(){return;}
                                        crate::motion::stop_deal(card.id);
                                        if group().is_empty()&&selected()==Some(card.id)&&legal{play_card(card,choosing,selected);}
                                        else if next_selection.len()>1{selected.set(None);group.set(next_selection.clone());}
                                        else{group.set(vec![]);selected.set(next_selection.first().copied());}
                                    },
                                    CardFace { card }
                                    if let Some(index)=queue_index{span{class:"card-order-badge","{index+1}"}}
                                }
                            }
                        }
                    }
                }
            }
            div{class:"table-emotes",
                div {class:"reaction-strip",aria_label:t("Reactions"),for emoji in reactions {
                    button {class:"reaction-button glass-circle",aria_label:format!("{} {}",t("React"),emoji.as_str().unwrap_or_default()),disabled:*REACTING.read()||!*CONNECTED.read(),onclick:{let emoji=emoji.as_str().unwrap_or_default().to_string();move |_|react(emoji.clone())},"{emoji.as_str().unwrap_or_default()}"}
                }}
            }
            super::reactions::TableReactions {}
            if swap_picker() && matches!(g.phase,Phase::Swap{player} if player==local){super::house::SwapPicker{onclose:move |_|swap_picker.set(false)}}

            if let Some(id) = choosing() {
                if can_play && g.playable.contains(&id) {
                    ColorPicker {
                        warning: g
                                .hand.iter().any(| c | c.id == id && c.value == Value::WildFour) && g
                                .four_warning,
                        onchoose: move |color| {
                            choosing.set(None);
                            command(Action::Play {
                                card: id,
                                color: Some(color),
                                uno: *ARMED.read(),
                            });
                            selected.set(None);
                        },
                        oncancel: move |_| choosing.set(None),
                    }
                }
            }
            if opening() && actionable && matches!(g.phase, Phase::OpeningColor) {
                ColorPicker {
                    onchoose: move |color| {
                        opening.set(false);
                        command(Action::ChooseColor(color));
                    },
                    oncancel: move |_| opening.set(false),
                }
            }
            if g.paused {
                div { class: "modal-backdrop",onkeydown:move|e|browser::trap_focus(&e),
                    section {
                        class: "color-dialog glass",
                        role: "dialog",
                        aria_modal: "true",
                        aria_label: t("Game paused"),
                        h2 { {t("Game paused")} }
                        p { {t("Your cards and remaining decision time are saved.")} }
                        if host {
                            button {
                                autofocus: true,
                                disabled: *BUSY.read() || !*CONNECTED.read(),
                                onclick: move |
                                        _ | room_control("resume"),
                                {t("Continue game")}
                            }
                        } else {
                            p { {t("Waiting for your host to continue.")} }
                        }
                        button {
                            class: "text-button",
                            disabled: *BUSY.read(),
                            onclick: move |_| nav(Screen::MainMenu),
                            {t("Back home")}
                        }
                    }
                }
            }
        }
    }
}
