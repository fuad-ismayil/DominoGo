use crate::game::{
    drag::DragController,
    tile::{BoardTile, TileFace},
};
use crate::i18n::{t, tf};
use crate::placement::{preview_tile, PendingPlacement};
use crate::screens::score::ScoreReadout;
use crate::{app::*, net, store::*, types::SGameState};
use dioxus::prelude::*;
use serde_json::{json, Value};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast};

fn request(event: &str, mut payload: Value, mut pending: Signal<bool>) {
    if pending() {
        return;
    }
    pending.set(true);
    payload["lobbyId"] = json!(LOBBY.read().lobby_id);
    net::emit(
        event,
        payload,
        Some(Box::new(move |response| {
            let mut pending = pending;
            if let Ok(mut busy) = pending.try_write() {
                *busy = false;
            }
            if let Some(e) = response["error"].as_str() {
                error(e);
                crate::game::audio::play(crate::game::audio::Sound::Error);
            }
        })),
    );
}
fn place(
    tile: [u8; 2],
    end: &str,
    mut pending: Signal<bool>,
    mut placement: Signal<Option<PendingPlacement>>,
) -> bool {
    let Some(state) = GAME_STATE.read().clone() else {
        return false;
    };
    let Some(end) = domino_core::End::from_token(end) else {
        return false;
    };
    let legal = crate::board::placements(&state, &SESSION.read().user_id)
        .into_iter()
        .any(|(t, e)| e == end && (t == tile || t == [tile[1], tile[0]]));
    if pending() || placement().is_some() || !legal {
        return false;
    }
    let issued = PendingPlacement::new(&state, tile, end);
    placement.set(Some(issued));
    pending.set(true);
    net::emit(
        "play_tile",
        json!({"lobbyId":LOBBY.read().lobby_id,"tile":tile,"end":end}),
        Some(Box::new(move |response| {
            let mut pending = pending;
            let mut placement = placement;
            // A late acknowledgement must not clear a later action or report failure
            // after an authoritative snapshot has already confirmed this placement.
            if placement
                .try_read()
                .ok()
                .is_none_or(|active| *active != Some(issued))
            {
                return;
            }
            if let Some(e) = response["error"].as_str() {
                if let Ok(mut busy) = pending.try_write() {
                    *busy = false;
                }
                if let Ok(mut visual) = placement.try_write() {
                    *visual = None;
                }
                error(e);
                crate::game::audio::play(crate::game::audio::Sound::Error);
            }
        })),
    );
    true
}

#[component]
pub fn Game() -> Element {
    use_effect(move || {
        let _ = EMOTE_USAGE.read().len();
        // A reaction sent from the far end can move to the front. Reveal that
        // updated order instead of leaving the strip scrolled past favorites.
        if let Some(row) = crate::browser::document().get_element_by_id("emote-list") {
            if let Some(row) = row.dyn_ref::<web_sys::HtmlElement>() {
                row.set_scroll_left(0);
            }
        }
    });
    let mut selected = use_signal(|| None::<[u8; 2]>);
    let mut pending = use_signal(|| false);
    let mut placement = use_signal(|| None::<PendingPlacement>);
    let mut connected = use_signal(net::is_connected);
    use_future(move || async move {
        loop {
            crate::game::timers::sleep_ms(500).await;
            let next = net::is_connected();
            if connected() != next {
                connected.set(next);
            }
        }
    });
    let mut previous_state = use_signal(|| None::<SGameState>);
    let mut round_before = use_signal(|| None::<SGameState>);
    use_effect(move || {
        let current = GAME_STATE.read().clone();
        if let Some(state) = current.as_ref() {
            if matches!(state.status.as_str(), "round_end" | "finished") {
                if let Some(old) = previous_state
                    .peek()
                    .as_ref()
                    .filter(|old| old.status == "playing" && old.round_number == state.round_number)
                {
                    round_before.set(Some(old.clone()));
                }
            } else if state.status == "playing" {
                round_before.set(None);
            }
        }
        previous_state.set(current);
    });
    let drag = use_hook(move || {
        Rc::new(DragController::new(
            move |tile| {
                let mut selected = selected;
                selected.set(Some(tile));
            },
            move |tile, end| {
                let mut selected = selected;
                if place(tile, end, pending, placement) {
                    selected.set(None);
                }
            },
        ))
    });
    let cancel = drag.clone();
    use_effect(move || {
        let state = GAME_STATE.read().clone();
        if placement.peek().as_ref().is_some_and(|p| {
            state
                .as_ref()
                .is_none_or(|s| p.resolved(s, &SESSION.read().user_id))
        }) {
            placement.set(None);
            pending.set(false);
        }
        if GAME_STATE
            .read()
            .as_ref()
            .is_some_and(|state| state.status != "playing")
        {
            if let Ok(Some(menu)) = crate::browser::document().query_selector("details.table-menu")
            {
                let _ = menu.remove_attribute("open");
            }
        }
        cancel.cancel();
        selected.set(None);
    });
    let cancel = drag.clone();
    use_drop(move || cancel.cancel());
    let Some(state) = GAME_STATE.read().clone() else {
        return rsx! {section {class:"panel narrow",{t("Dealing your tiles…")}}};
    };
    let revealed = matches!(state.status.as_str(), "round_end" | "finished");
    let me = SESSION.read().user_id.clone();
    let my_turn = state.status == "playing" && state.players.get(state.turn_index) == Some(&me);
    let hand = state.hands.get(&me).cloned().unwrap_or_default();
    let turn = state
        .players
        .get(state.turn_index)
        .map(|p| player_name(p))
        .unwrap_or_default();
    let me_index = state.players.iter().position(|p| p == &me).unwrap_or(0);
    let opponents: Vec<_> = (1..state.players.len())
        .map(|offset| state.players[(me_index + offset) % state.players.len()].clone())
        .collect();
    let review_columns = if state.players.len() == 2 { 5 } else { 3 };
    let review_needed_rows = opponents
        .iter()
        .map(|id| {
            state
                .hands
                .get(id)
                .map_or(0, |h| h.len().div_ceil(review_columns))
        })
        .max()
        .unwrap_or(0);
    let review_rows = review_needed_rows.min(2);
    let review_tall_rows = review_needed_rows.min(3);
    let busy = pending() || placement().is_some() || !connected();
    rsx! {
        style {{include_str!("../../assets/game.css")}}
        section {style:"--review-hand-rows:{review_rows};--review-hand-rows-tall:{review_tall_rows};",class:if state.status=="playing" {"club-game"} else if revealed && !hand.is_empty() {"club-game review-mode has-own-remainder"} else {"club-game review-mode"},
            if !ERROR_MSG.read().is_empty(){div {class:"game-error-toast",role:"alert",{t(&ERROR_MSG.read())} button {class:"text-button",onclick:move |_|ERROR_MSG.signal().set(String::new()),"×"}}}
            div {class:"match-bar glass-capsule",
                div {class:"match-title",span {class:"round-kicker",{tf("Round {round} · {mode}", &[("round",state.round_number.to_string()),("mode",if state.mode=="telefon"{"5-lik · Telefon".into()}else{"101".into()})])}} strong {if state.status=="playing" {if my_turn {{t("Your move")}} else {{tf("{turn} is thinking", &[("turn", (turn).to_string())])}}} else if state.status=="round_end" {{t("Round complete")}} else if state.status=="stopped" {{t("Match paused")}} else {{t("Match complete")}}}}
                ConnectionStatus {connected:connected()}
                Countdown {deadline:state.turn_deadline, active:state.status=="playing"}
                details {class:"table-menu",summary {aria_label:t("Table settings"),"•••"} div {class:"table-menu-panel glass",
                    small {{tf("TABLE {LOBBY.read().lobby_id}", &[("LOBBY.read().lobby_id", (LOBBY.read().lobby_id).to_string())])}}
                    p {class:"locked-draw-rule", {tf("{state.draw_rule.description()} · Locked", &[("state.draw_rule.description()", t(state.draw_rule.description()))])}}
                    button {class:"text-button",onclick:move |_|{let doc=crate::browser::document();if doc.fullscreen_element().is_some(){doc.exit_fullscreen();}else if let Some(el)=doc.document_element(){let _=el.request_fullscreen();}},{t("Fullscreen")}}
                    button {class:"text-button",aria_pressed:*SOUND_ENABLED.read(),onclick:move |_|{let next=!*SOUND_ENABLED.read();SOUND_ENABLED.signal().set(next);crate::game::audio::set_enabled(next);},if *SOUND_ENABLED.read(){{t("Sound on")}}else{{t("Sound off")}}}
                    if state.status=="playing" {button {class:"text-button",onclick:move |_|action("stop_game_request",json!({})),{t("Pause match")}}}
                    crate::screens::ui::LanguagePicker {}
                    crate::screens::rules::Rules {}
                }}
            }
            div {class:format!("domino-arena seats-{}",state.players.len()),
                for (index,id) in opponents.iter().enumerate() {Seat {key:"{id}",id:id.clone(),state:state.clone(),position:if opponents.len()==1 || index==1 {"north"}else if index==0 {"west"}else{"east"}}}
                Board {state:state.clone(),selected:selected(),busy,placement:placement(),onplace:move |end:String|{if let Some(tile)=selected(){if place(tile,&end,pending,placement){selected.set(None);}}}}
                div {class:if my_turn {"local-seat glass-capsule seat-active"}else{"local-seat glass-capsule"},
                    ReactionBubble {user_id:me.clone()}
                    span {class:"seat-avatar","{SESSION.read().username.chars().next().unwrap_or('•')}"}
                    strong {"{SESSION.read().username}"} ScoreReadout {telefon:state.mode=="telefon",score:state.scores.get(&me).copied().unwrap_or_default()}
                }
                if revealed && !hand.is_empty() {div {class:"own-revealed-hand", RevealedHand {hand:hand.clone(),owner:player_name(&me),points:0} }}
            }
            if state.status!="playing" {section {class:"round-review",aria_label:t("Round results"),
                div {class:"round-review-summary",
                    div {span {class:"round-kicker",if state.status=="round_end"{{t("ROUND COMPLETE")}}else if state.status=="stopped"{{t("TABLE PAUSED")}}else{{t("MATCH COMPLETE")}}}
                        if let Some(winner)=state.last_winner.as_ref().filter(|_|state.status!="stopped") {h2 {"{player_name(winner)}"} p {if state.status=="finished"{{t("Wins the match. Well played.")}}else{{t("Takes the round.")}}} p {class:"round-collected",role:"status", {crate::feedback::score_transaction(&state,round_before.read().as_ref(),&me)}}}
                    }
                    div {class:"round-review-actions",
                        if LOBBY.read().is_host {
                            button {disabled:pending(),onclick:move |_|request(if GAME_STATE.read().as_ref().is_some_and(|s|s.status=="round_end"){"next_round"}else if GAME_STATE.read().as_ref().is_some_and(|s|s.status=="stopped"){"continue_game"}else{"rematch"},json!({}),pending),if state.status=="round_end"{{t("Next round →")}}else if state.status=="stopped"{{t("Continue match")}}else{{t("Play again")}}}
                        }else{small {class:"muted",{t("Waiting for the host…")}}}
                        button {class:"text-button",onclick:move |_|{net::emit("leave_lobby",json!(LOBBY.read().lobby_id),None);reset_to_menu();spawn(refresh_menu());},{t("Return home")}}
                    }
                }
                div {class:"final-scores",for id in state.players.iter(){div {span {"{player_name(id)}"} ScoreReadout {telefon:state.mode=="telefon",score:state.scores.get(id).copied().unwrap_or_default()}}}}
                details {class:"round-score-details",summary {{t("Score details")}} p {{t("The finished board stays visible until the next round.")}} if state.scores.values().any(|score|score.temporary>0) {p {{t("Pending points bank once consecutive wins take them above 12. Losing a round clears pending points.")}}}}
            }}
            if state.status=="playing" {
                div {class:"rack-area",
                    div {class:"rack-caption",role:"status",aria_live:"polite",aria_atomic:"true",span {{crate::feedback::guidance(&state,&me,busy,connected())}} if my_turn && !busy {small {{t("or tap a tile, then an end")}}}}
                    div {class:"tile-rack",for (index,tile) in hand.iter().copied().enumerate(){
                        {let sides=crate::board::can_play_sides(&state,&me,tile);let legal=if state.mode=="telefon"{crate::board::placements(&state,&me).iter().any(|(t,_)|*t==tile)}else{sides.0||sides.1};let id=format!("rack-tile-{index}");let source=id.clone();let start=drag.clone();let click=drag.clone();rsx! {
                            button {id, class:format!("rack-tile {} {} {}",if legal{"legal-tile"}else{"quiet-tile"},if selected()==Some(tile){"chosen-tile"}else{""},if placement().as_ref().is_some_and(|p|p.tile==tile){"tile-submitting"}else{""}),disabled:!my_turn || !legal || busy,aria_label:format!("{} · {} · {}",tile[0],tile[1],t(if legal {"playable"} else {"cannot play"})),aria_pressed:selected()==Some(tile),
                                onpointerdown:move |e|{if e.is_primary() && e.trigger_button()==Some(dioxus::html::input_data::MouseButton::Primary){e.prevent_default();let p=e.client_coordinates();start.begin(tile,e.pointer_id(),p.x,p.y,&source);}},
                                onclick:move |_|{if click.allow_click(){selected.set(Some(tile));}},
                                TileFace {value:tile}
                            }
                        }}
                    }}
                    div {class:"rack-actions",button {class:"market-button glass-capsule",disabled:busy||!crate::board::can_draw(&state,&me),onclick:move |_|request("draw_market",json!({}),pending),span {class:"market-icon","▤"} strong {{t("Draw")}} span {"{state.market_count}"}}
                        div {id:"emote-list",class:"reactions",role:"group",aria_label:t("Quick reactions, swipe for more"),tabindex:"0",for emoji in crate::reactions::ordered(&EMOTE_USAGE.read()) {button {key:"{emoji}",class:"reaction",aria_label:tf("Send reaction {emoji}", &[("emoji", (emoji).to_string())]),onclick:move |_|action("send_reaction",json!({"emoji":emoji})),"{emoji}"}}}
                        button {class:"pass-button glass-capsule",disabled:busy||!crate::board::can_pass(&state,&me),onclick:move |_|request("pass_turn",json!({}),pending),{t("Pass →")}}
                    }
                }
            }
            if let Some(vote)=STOP_VOTE.read().as_ref(){if vote.votes>0 && !vote.stopped {div {class:"pause-toast glass-capsule",{tf("Pause requested · {vote.votes}/{vote.required}", &[("vote.votes", format!("{}", (vote.votes))), ("vote.required", format!("{}", (vote.required)))])} button {onclick:move |_|action("stop_game_request",json!({})),{t("Agree")}} button {class:"text-button",onclick:move |_|action("cancel_stop_vote",json!({})),{t("Cancel")}}}}}
        }
    }
}

#[component]
fn Countdown(deadline: Option<f64>, active: bool) -> Element {
    let mut now = use_signal(crate::browser::now_ms);
    use_future(move || async move {
        loop {
            crate::game::timers::sleep_ms(250).await;
            now.set(crate::browser::now_ms());
            net::keepalive_tick(crate::browser::now_ms());
        }
    });
    let remaining = deadline.map(|d| ((d - now()) / 1000.).max(0.).ceil() as i64);
    rsx! {if active {if let Some(seconds)=remaining {span {class:if seconds<=5{"turn-clock clock-urgent"}else{"turn-clock"},{tf("{seconds}s", &[("seconds",seconds.to_string())])}}}}}
}

#[component]
fn Seat(id: String, state: SGameState, position: String) -> Element {
    let active = state.players.get(state.turn_index) == Some(&id) && state.status == "playing";
    let name = player_name(&id);
    let count = state.opponent_hand_counts.get(&id).copied().unwrap_or(0);
    rsx! {div {class:format!("opponent-seat seat-{position} {}",if active{"seat-active"}else{""}),
        div {class:"seat-badge glass-capsule",span {class:"seat-avatar","{name.chars().next().unwrap_or('•')}"} div {strong {"{name}"}ScoreReadout {telefon:state.mode=="telefon",score:state.scores.get(&id).copied().unwrap_or_default()}} span {class:"tile-count","{count}"}}
        ReactionBubble {user_id:id.clone()}
        if matches!(state.status.as_str(),"round_end"|"finished") {
            RevealedHand {hand:state.hands.get(&id).cloned().unwrap_or_default(),owner:name.clone(),points:if state.last_winner.as_ref()==Some(&id){0}else{crate::board::hand_points(state.hands.get(&id).map(Vec::as_slice).unwrap_or_default())}}
        }else{div {class:"opponent-rack",aria_label:tf("{count} hidden tiles", &[("count", format!("{}", (count)))]),for i in 0..count.min(10) {span {key:"{i}",class:"tile-back",style:"--fan:{i};"}}}}
    }}
}

#[component]
fn Board(
    state: SGameState,
    selected: Option<[u8; 2]>,
    busy: bool,
    placement: Option<PendingPlacement>,
    onplace: EventHandler<String>,
) -> Element {
    if state.mode == "telefon" {
        return rsx! { super::telefon::TelefonBoard {state,selected,busy,placement,onplace} };
    }
    let dimensions = use_signal(|| (700., 300.));
    let observer = use_hook(|| Rc::new(RefCell::new(None::<BoardResize>)));
    let cleanup = observer.clone();
    use_drop(move || {
        cleanup.borrow_mut().take();
    });
    let attach = move |_| {
        let Some(el) = crate::browser::document().get_element_by_id("snake-board") else {
            return;
        };
        let r = el.get_bounding_client_rect();
        let mut initial = dimensions;
        initial.set((r.width().max(1.), r.height().max(1.)));
        let runtime = dioxus::dioxus_core::Runtime::current();
        let measured = el.clone();
        let callback =
            Closure::<dyn FnMut(js_sys::Array, web_sys::ResizeObserver)>::new(move |_, _| {
                let r = measured.get_bounding_client_rect();
                let next = (r.width().max(1.), r.height().max(1.));
                runtime.in_scope(dioxus::prelude::ScopeId::ROOT, || {
                    let mut dimensions = dimensions;
                    if dimensions() != next {
                        dimensions.set(next);
                    }
                });
            });
        if let Ok(resize) = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
            resize.observe(&el);
            *observer.borrow_mut() = Some(BoardResize {
                observer: resize,
                _callback: callback,
            });
        }
    };
    let me = SESSION.read().user_id.clone();
    let my_turn = state.status == "playing" && state.players.get(state.turn_index) == Some(&me);
    let sides = selected
        .map(|tile| crate::board::can_play_sides(&state, &me, tile))
        .unwrap_or((false, false));
    let viewport = crate::browser::window();
    let portrait = viewport
        .inner_width()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(1000.)
        < 600.
        && viewport
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(600.)
            > viewport
                .inner_width()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(1000.);
    let unit = if portrait { 32. } else { 34. };
    let layout = crate::layout::fit_at_scale(&state.board, dimensions().0, dimensions().1, unit);
    rsx! {div {id:"snake-board",class:"snake-board",onmounted:attach,aria_label:t("Domino chain with left and right open ends"),
        if state.board.is_empty(){div {class:"table-watermark",span {"DOMINO"}strong {"GO"}small {if my_turn{{t("Your opening move")}}else{{t("The table is yours to discover")}}}}}
        svg {class:"snake-svg",style:"position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);width:min(100%,{layout.width*unit}px);height:min(100%,{layout.height*unit}px)",view_box:"0 0 {layout.width} {layout.height}",preserve_aspect_ratio:"xMidYMid meet",
            defs {
                linearGradient {id:"ivory-face",x1:"0",y1:"0",x2:"1",y2:"1",stop {offset:"0%",stop_color:"#fffef6"}stop {offset:"55%",stop_color:"#f3ecd9"}stop {offset:"100%",stop_color:"#ded0ae"}}
                radialGradient {id:"pip-ink",cx:"40%",cy:"35%",r:"65%",stop {offset:"0%",stop_color:"#233a2c"}stop {offset:"75%",stop_color:"#102319"}stop {offset:"100%",stop_color:"#756f54"}}
            }
            for (index,slot) in layout.slots.iter().enumerate(){
                if index==0 || index==layout.slots.len()-1 {
                    {let (mx,my,mw,mh)=slot.drop_bounds();let end=if index==0{"left"}else{"right"};let enabled=my_turn&&!busy&&if index==0{sides.0}else{sides.1};let empty=state.board.is_empty();let label=if empty {if index==0{{t("START")}}else{String::new()}}else if index==0{{t("LEFT")}}else{{t("RIGHT")}};let pip=if index==0{state.open_ends[0]}else{state.open_ends[1]};
                    let target_end=if index==0{domino_core::End::Left}else{domino_core::End::Right};
                    let submitting=placement.as_ref().filter(|p|p.end==target_end);
                    let preview=submitting.map(|p|p.tile).or(if enabled {selected}else{None});
                    rsx! {
                        if state.status=="playing" && (!empty || index==0) {g {id:"drop-{end}",class:format!("snake-drop {} {}",if enabled{"drop-enabled"}else{""},if submitting.is_some(){"drop-submitting"}else{""}),"data-drop-end":end,"data-enabled":enabled.to_string(),role:"button",tabindex:if enabled{"0"}else{"-1"},"aria-label":{tf("Play on {end} end", &[("end", t(if end=="left" {"Left"}else{"Right"}).to_lowercase())])},"aria-disabled":(!enabled).to_string(),transform:"translate({mx} {my})",
                            onclick:move |_|{if enabled{onplace.call(end.into());}},onkeydown:move |e|{if enabled&&(e.key()==Key::Enter||e.key()==Key::Character(" ".into())){e.prevent_default();onplace.call(end.into());}},
                            rect {x:"0",y:"0",width:"{mw}",height:"{mh}",rx:".17",fill:"#112f25",fill_opacity:".5",stroke:"currentColor",stroke_width:".035",stroke_dasharray:if enabled{".12 .08"}else{".08 .13"}}
                            if let Some(tile)=preview {
                                {let value=preview_tile(tile,target_end,state.open_ends);let vertical=value[0]==value[1];let (pw,ph)=if vertical{(1.,2.)}else{(2.,1.)};rsx! {
                                    g {class:if submitting.is_some(){"placement-preview preview-submitting"}else{"placement-preview"},transform:"translate({(mw-pw*0.86)/2.} {(mh-ph*0.86)/2.}) scale(0.86)",pointer_events:"none","aria-hidden":"true",BoardTile {value,vertical,reversed:slot.reversed}}
                                }}
                            } else {
                            text {x:"{mw/2.}",y:".30",text_anchor:"middle",font_size:".20",font_family:"sans-serif",font_weight:"700",fill:"currentColor","{label}"}
                            text {x:"{mw/2.}",y:".76",text_anchor:"middle",font_size:".36",font_family:"sans-serif",fill:"currentColor","{pip.map(|n|n.to_string()).unwrap_or_else(||\"+\".into())}"}
                            }
                        }}
                    }}
                }else {
                    {let tile=state.board[index-1];let last=state.last_played_tile.is_some_and(|t|t==tile||t==[tile[1],tile[0]]);let finish=state.round_finishing_tile.is_some_and(|t|t==tile||t==[tile[1],tile[0]]);let vertical=slot.vertical||tile[0]==tile[1];let (w,h)=if vertical{(1.,2.)}else{(2.,1.)};rsx! {g {key:"{tile[0].min(tile[1])}-{tile[0].max(tile[1])}",class:if finish {"placed-domino round-finisher"}else if last {"placed-domino last-played"}else{"placed-domino"},"data-last-played":last.to_string(),"data-round-finisher":finish.to_string(),transform:"translate({slot.x-w/2.} {slot.y-h/2.})",BoardTile {value:tile,vertical,reversed:slot.reversed&&!slot.vertical}
                        if last || finish {rect {class:"last-tile-rim",x:".035",y:".035",width:"{w-0.07}",height:"{h-0.07}",rx:".095",fill:if finish {"#35baad38"}else{"#ecc45a24"},stroke:if finish {"#168a83"}else{"#c38a22"},stroke_width:if finish {"2"}else{"1.25"},"vector-effect":"non-scaling-stroke",pointer_events:"none"} title {{t(if finish {"Round-ending tile"}else{"Last played tile"})}}}
                    }}}
                }
            }
        }
    }}
}

#[component]
fn ReactionBubble(user_id: String) -> Element {
    let reaction = TABLE_REACTIONS.read().get(&user_id).cloned();
    rsx! {if let Some(reaction)=reaction {span {key:"{reaction.stamp}",class:"reaction-bubble",role:"status",aria_label:tf("{player_name(&user_id)} reacts {reaction.emoji}", &[("player_name(&user_id)",player_name(&user_id)),("reaction.emoji",reaction.emoji.clone())]),"{reaction.emoji}"}}}
}

#[component]
fn ConnectionStatus(connected: bool) -> Element {
    rsx! {if !connected {span {class:"connection-indicator",role:"status",{t("Reconnecting…")}}}}
}

struct BoardResize {
    observer: web_sys::ResizeObserver,
    _callback: Closure<dyn FnMut(js_sys::Array, web_sys::ResizeObserver)>,
}
impl Drop for BoardResize {
    fn drop(&mut self) {
        self.observer.disconnect();
    }
}

#[component]
fn RevealedHand(hand: Vec<[u8; 2]>, owner: String, points: i32) -> Element {
    rsx! {div {class:"revealed-hand",role:"group",aria_label:format!("{} — {}",owner,t("Remaining tiles")),
        div {class:"revealed-tiles",tabindex:"0", if hand.is_empty() {span {class:"empty-hand","—"}} for tile in hand {div {class:"revealed-tile",role:"img",aria_label:format!("{} · {}",tile[0],tile[1]),TileFace {value:tile}}}}
        if points>0 {small {class:"revealed-points",{tf("Round points: {points}",&[("points",points.to_string())])}}}
    }}
}
