use crate::i18n::{t, tf};
use crate::screens::{
    auth::Auth, friends::Friends, game::Game, leaders::Leaders, lobby::Lobby, menu::Menu,
    setup::Setup,
};
use crate::{net, store::*, types::*};
use dioxus::prelude::*;
use serde_json::{json, Value};

pub fn error(message: impl Into<String>) {
    ERROR_MSG.signal().set(message.into());
}
pub fn action(event: &str, extra: Value) {
    let mut payload = extra;
    payload["lobbyId"] = json!(LOBBY.read().lobby_id);
    net::emit(
        event,
        payload,
        Some(Box::new(|v| {
            if let Some(e) = v["error"].as_str() {
                error(e);
            }
        })),
    );
}
pub(crate) async fn result(r: Result<Value, net::RestError>) -> Option<Value> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            error(e.message);
            None
        }
    }
}
pub async fn enter_lobby(id: String) {
    if let Some(v) = result(net::join_lobby_rest(&id).await).await {
        activate_lobby(id, v);
    }
}
fn activate_lobby(id: String, v: Value) {
    let previous = LOBBY.read().lobby_id.clone();
    if !previous.is_empty() && previous != id {
        net::emit("leave_lobby", json!(previous), None);
    }
    GAME_STATE.signal().set(None);
    STOP_VOTE.signal().set(None);
    SHOW_INVITE_MODAL.signal().set(false);
    LOBBY.signal().set(LobbyView {
        mode: v["mode"].as_str().unwrap_or("101").into(),
        lobby_id: id.clone(),
        players: parse_players(&v["players"]),
        max_players: v["maxPlayers"].as_u64().unwrap_or(4) as usize,
        draw_rule: serde_json::from_value(v["drawRule"].clone()).unwrap_or_default(),
        turn_timer: v["turnTimer"].as_bool().unwrap_or(true),
        is_host: v["creatorId"].as_str() == Some(&SESSION.read().user_id),
    });
    nav(Screen::Lobby);
    net::emit(
        "join_lobby",
        json!(id),
        Some(Box::new(|v| {
            if let Some(e) = v["error"].as_str() {
                error(e);
            }
        })),
    );
}

async fn open_invite(invite: Result<String, crate::invite::InviteProblem>) {
    use crate::invite::InviteProblem;
    let problem = match invite {
        Ok(id) => match net::join_lobby_rest(&id).await {
            Ok(v) => {
                activate_lobby(id, v);
                return;
            }
            Err(e) => match e.code.as_deref() {
                Some("TABLE_FULL") => InviteProblem::Full,
                Some("TABLE_STARTED") => InviteProblem::Started,
                _ if e.status == 404 => InviteProblem::Missing,
                _ => InviteProblem::Unavailable,
            },
        },
        Err(problem) => problem,
    };
    reset_to_menu();
    INVITE_NOTICE.signal().set(Some(problem));
}

pub async fn refresh_menu() {
    let id = SESSION.read().user_id.clone();
    if id.is_empty() {
        return;
    }
    if let Ok(v) = net::matches_history().await {
        if SESSION.read().user_id != id {
            return;
        }
        if let Ok(matches) = serde_json::from_value(v["matches"].clone()) {
            if *MATCH_HISTORY.read() != matches {
                MATCH_HISTORY.signal().set(matches);
            }
        }
    }
    if let Ok(v) = net::player_stats(&id).await {
        if SESSION.read().user_id != id {
            return;
        }
        if let Ok(stats) = serde_json::from_value(v) {
            if PLAYER_STATS.read().as_ref() != Some(&stats) {
                PLAYER_STATS.signal().set(Some(stats));
            }
        }
    }
}
pub async fn refresh_friends() {
    if let Some(v) = result(net::friends_list().await).await {
        FRIENDS.signal().set(FriendsView {
            friends: serde_json::from_value(v["friends"].clone()).unwrap_or_default(),
            incoming: serde_json::from_value(v["incoming"].clone()).unwrap_or_default(),
            outgoing: serde_json::from_value(v["outgoing"].clone()).unwrap_or_default(),
        });
        GAME_INVITE_TOASTS
            .signal()
            .set(serde_json::from_value(v["pendingGameInvites"].clone()).unwrap_or_default());
    }
}
fn event_is_current(value: &Value) -> bool {
    let current = LOBBY.read().lobby_id.clone();
    !current.is_empty() && value["lobbyId"].as_str().is_none_or(|id| id == current)
}

fn install_handlers() {
    net::on(
        "emote_usage",
        Box::new(|v| {
            if v["userId"].as_str() != Some(SESSION.read().user_id.as_str()) {
                return;
            }
            if let Ok(counts) = serde_json::from_value::<std::collections::BTreeMap<String, u64>>(
                v["counts"].clone(),
            ) {
                let mut usage = EMOTE_USAGE.signal();
                let mut usage = usage.write();
                // Concurrent sockets may deliver snapshots out of order. Counts
                // only grow, so an older snapshot must not undo a newer ordering.
                for (emoji, count) in counts {
                    if REACTIONS.contains(&emoji.as_str()) {
                        let previous = usage.entry(emoji).or_default();
                        *previous = (*previous).max(count);
                    }
                }
            }
        }),
    );
    net::on(
        "connect",
        Box::new(|_| {
            let id = LOBBY.read().lobby_id.clone();
            if !id.is_empty() {
                net::emit("join_lobby", json!(id), None);
            }
        }),
    );
    net::on(
        "connect_error",
        Box::new(|v| {
            error(
                v["message"]
                    .as_str()
                    .unwrap_or("Connection failed. Please sign in again."),
            )
        }),
    );
    net::on(
        "lobby_updated",
        Box::new(|v| {
            if !event_is_current(v) {
                return;
            }
            let players = parse_players(&v["players"]);
            let me = SESSION.read().user_id.clone();
            if !players.iter().any(|p| p.id == me) {
                reset_to_menu();
                error("You left the table.");
            } else {
                let mut lobby = LOBBY.signal();
                let mut lobby = lobby.write();
                lobby.players = players;
                if let Some(creator) = v["creatorId"].as_str() {
                    lobby.is_host = creator == me;
                }
            }
        }),
    );
    net::on(
        "game_started",
        Box::new(|v| {
            if !event_is_current(v) {
                return;
            }
            nav(Screen::Game);
        }),
    );
    net::on(
        "game_state_update",
        Box::new(|v| {
            if !event_is_current(v) {
                return;
            }
            match serde_json::from_value::<SGameState>(v.clone()) {
                Ok(s) => {
                    let previous = GAME_STATE.read().clone();
                    let me = SESSION.read().user_id.clone();
                    if let Some(old) = previous {
                        if s.board.len() > old.board.len() {
                            // Contact lands near the end of the board tile's short settle.
                            crate::game::timers::set_timeout(120, || {
                                if !crate::browser::document().hidden() {
                                    crate::game::audio::play(crate::game::audio::Sound::Place);
                                }
                            });
                        } else if s.market_count < old.market_count {
                            crate::game::audio::play(crate::game::audio::Sound::Draw);
                        }
                        if s.status != old.status
                            && (s.status == "round_end" || s.status == "finished")
                        {
                            if s.last_winner.as_ref() == Some(&me) {
                                crate::game::audio::play(crate::game::audio::Sound::Win);
                                crate::game::confetti::round_win();
                            } else {
                                crate::game::audio::play(crate::game::audio::Sound::Lose);
                            }
                        // A placement already announces the handover. Only play
                        // a separate turn cue when no tile was placed (e.g. a pass).
                        } else if s.board.len() == old.board.len()
                            && s.players.get(s.turn_index) == Some(&me)
                            && old.players.get(old.turn_index) != Some(&me)
                        {
                            crate::game::audio::play(crate::game::audio::Sound::Turn);
                        }
                    }
                    GAME_STATE.signal().set(Some(s));
                    if *SCREEN.read() != Screen::Game {
                        nav(Screen::Game);
                    }
                }
                Err(_) => error("Unable to read game update. Rejoin the table."),
            }
        }),
    );
    net::on(
        "stop_vote_update",
        Box::new(|v| {
            if !event_is_current(v) {
                return;
            }
            STOP_VOTE
                .signal()
                .set(serde_json::from_value(v.clone()).ok())
        }),
    );
    net::on(
        "reaction",
        Box::new(|v| {
            if !event_is_current(v) {
                return;
            }
            let Some(user) = v["userId"].as_str().map(str::to_owned) else {
                return;
            };
            let emoji = v["emoji"].as_str().unwrap_or("").to_string();
            if !REACTIONS.contains(&emoji.as_str()) {
                return;
            }
            let stamp = crate::browser::now_ms();
            TABLE_REACTIONS
                .signal()
                .write()
                .insert(user.clone(), TableReaction { emoji, stamp });
            crate::game::timers::set_timeout(2400, move || {
                let remove = TABLE_REACTIONS
                    .read()
                    .get(&user)
                    .is_some_and(|r| r.stamp == stamp);
                if remove {
                    TABLE_REACTIONS.signal().write().remove(&user);
                }
            });
        }),
    );
    net::on(
        "game_invitation",
        Box::new(|v| {
            if let Ok(i) = serde_json::from_value::<GameInvite>(v.clone()) {
                if !GAME_INVITE_TOASTS
                    .read()
                    .iter()
                    .any(|existing| existing.id == i.id)
                {
                    GAME_INVITE_TOASTS.signal().write().push(i);
                }
            }
        }),
    );
    for event in [
        "friend_online",
        "friend_offline",
        "friend_request_received",
        "friend_request_accepted",
        "friend_removed",
    ] {
        net::on(
            event,
            Box::new(|_| {
                spawn(refresh_friends());
            }),
        );
    }
}

#[component]
pub fn App() -> Element {
    use_hook(|| {
        crate::i18n::init();
        install_handlers();
        SOUND_ENABLED.signal().set(crate::game::audio::enabled());
        spawn(async {
            let invite = crate::browser::take_invite();
            if net::get_token().is_some() {
                if let Ok(v) = net::auth_me().await {
                    apply_auth(&v);
                } else {
                    net::clear_token();
                }
            }
            // Opening an invite also works for a first-time visitor: reserve
            // their seat as a guest without adding a sign-in detour.
            if invite.is_some() && SESSION.read().user_id.is_empty() {
                match net::auth_guest("Guest").await {
                    Ok(v) => {
                        apply_auth(&v);
                        if let Ok(v) = net::auth_me().await {
                            apply_auth(&v);
                        }
                    }
                    Err(_) => {
                        error("We couldn’t open your invite. Check your connection and try the link again.");
                    }
                }
            }
            if !SESSION.read().user_id.is_empty() {
                nav(Screen::MainMenu);
                net::ensure_connected();
                if let Some(invite) = invite {
                    open_invite(invite).await;
                }
            }
            LOADING.signal().set(false);
            if !SESSION.read().user_id.is_empty() {
                refresh_menu().await;
                refresh_friends().await;
            }
        });
    });
    use_future(move || async move {
        loop {
            net::keepalive_tick(crate::browser::now_ms());
            crate::game::timers::sleep_ms(1000).await;
        }
    });
    let invite_pending = use_signal(|| None::<i64>);
    let invites = GAME_INVITE_TOASTS.read().clone();
    let screen = *SCREEN.read();
    let signed_in = !SESSION.read().user_id.is_empty();
    let show_navigation = signed_in
        && LOBBY.read().lobby_id.is_empty()
        && !matches!(
            screen,
            Screen::Entry | Screen::Login | Screen::Register | Screen::Game
        );
    let avatar = SESSION
        .read()
        .username
        .chars()
        .next()
        .unwrap_or('D')
        .to_uppercase()
        .to_string();
    rsx! {
        style { {include_str!("../assets/index.css")} }
        style { {include_str!("../assets/menus.css")} }
        style { {include_str!("../assets/club-system.css")} }
        canvas { id: "confetti-canvas", style: "position:fixed;inset:0;pointer-events:none;z-index:20;" }
        main { aria_hidden: if INVITE_NOTICE.read().is_some() { "true" } else { "false" }, class: if screen == Screen::Game { "app-shell app-shell--game game-shell" } else { "app-shell menu-shell" },
            if screen != Screen::Game { header { class: "topbar club-topbar",
                button { class: "brand-button", aria_label: t("DominoGo home"), onclick: move |_| { if LOBBY.read().lobby_id.is_empty() && !SESSION.read().user_id.is_empty() { nav(Screen::MainMenu); } }, crate::screens::ui::BrandLogo {} }
                if signed_in && screen != Screen::Game {
                    div { class: "topbar-account", span { class: "muted", "{SESSION.read().username}" } button { class: "profile-button glass-circle", aria_label: t("Open your account"), onclick: move |_| ACCOUNT_OPEN.signal().set(true), "{avatar}" } }
                } else if !signed_in { span { class: "header-tagline", {t("A classic. Good company.")} } }
            }
            }
            if screen != Screen::Game && !ERROR_MSG.read().is_empty() { div { class: "notice", role: "alert", {t(&ERROR_MSG.read())} button { class: "icon-button", onclick: move |_| ERROR_MSG.signal().set(String::new()), "×" } } }
            div { class: if screen == Screen::Game { "game-invite-stack" } else { "invite-stack" },
            for invite in invites {
                div { class: "notice invite-notice", role: "status", div { strong { {tf("A seat with {invite.sender_username}", &[("invite.sender_username", (invite.sender_username).to_string())])} } p { {t("You've been invited to a private table.")} } }
                    button { disabled: invite_pending().is_some(), onclick: move |_| { let id = invite.id; let mut pending=invite_pending; pending.set(Some(id)); net::emit("accept_game_invite", json!({"inviteId": id}), Some(Box::new(move |v| { let mut pending=invite_pending; pending.set(None); if let Some(lobby_id) = v["lobbyId"].as_str() { GAME_INVITE_TOASTS.signal().write().retain(|i| i.id != id); spawn(enter_lobby(lobby_id.to_string())); } else if let Some(e) = v["error"].as_str() { error(e); } }))); }, if invite_pending()==Some(invite.id) { {t("Please wait…")} } else { {t("Join table")} } }
                    button { class: "secondary", disabled: invite_pending().is_some(), onclick: move |_| { let id=invite.id; let mut pending=invite_pending; pending.set(Some(id)); net::emit("reject_game_invite", json!({"inviteId": id}), Some(Box::new(move |v| { let mut pending=invite_pending; pending.set(None); if let Some(e)=v["error"].as_str() { error(e); } else { GAME_INVITE_TOASTS.signal().write().retain(|i| i.id != id); } }))); }, {t("Decline")} }
                }
            }
            }
            if *LOADING.read() { section { class: "panel narrow", {t("Opening your table…")} } } else {
                match *SCREEN.read() {
                    Screen::Entry | Screen::Login | Screen::Register | Screen::UpgradeAccount => rsx! { Auth { key: "{screen:?}" } },
                    Screen::MainMenu => rsx! { Menu {} },
                    Screen::CreateLobby | Screen::JoinLobby => rsx! { Setup {} },
                    Screen::Lobby => rsx! { Lobby {} },
                    Screen::Game => rsx! { Game {} },
                    Screen::Friends => rsx! { Friends {} },
                    Screen::Leaderboard => rsx! { Leaders {} },
                }
            }
            if show_navigation { nav { class: "club-navigation glass-capsule", aria_label: t("Main navigation"),
                button { class: if screen == Screen::MainMenu { "nav-item active" } else { "nav-item" }, aria_current: if screen == Screen::MainMenu { "page" } else { "false" }, onclick: move |_| { nav(Screen::MainMenu); spawn(refresh_menu()); }, crate::screens::ui::Icon { name: "home" } span { {t("Home")} } }
                button { class: if screen == Screen::Friends { "nav-item active" } else { "nav-item" }, aria_current: if screen == Screen::Friends { "page" } else { "false" }, onclick: move |_| nav(Screen::Friends), crate::screens::ui::Icon { name: "people" } span { {t("Friends")} } }
                button { class: if screen == Screen::Leaderboard { "nav-item active" } else { "nav-item" }, aria_current: if screen == Screen::Leaderboard { "page" } else { "false" }, onclick: move |_| nav(Screen::Leaderboard), crate::screens::ui::Icon { name: "trophy" } span { {t("Ranking")} } }
                button { class: "nav-item", onclick: move |_| ACCOUNT_OPEN.signal().set(true), crate::screens::ui::Icon { name: "user" } span { {t("Account")} } }
            } }
            if *ACCOUNT_OPEN.read() { crate::screens::account::AccountSheet {} }
            if screen != Screen::Game { footer { class: "club-footer", {t("DOUBLE SIX. GOOD COMPANY. ONE MORE ROUND.")} } }
        }
        if let Some(problem) = *INVITE_NOTICE.read() { crate::screens::invite_notice::InviteNotice { problem } }
    }
}
