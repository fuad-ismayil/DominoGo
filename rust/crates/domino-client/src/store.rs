//! Shared reactive session, lobby and screen state. Screen inputs remain local to each component.

use crate::types::*;
use dioxus::prelude::*;

/// Application destinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Entry,
    Login,
    Register,
    MainMenu,
    CreateLobby,
    JoinLobby,
    Lobby,
    Game,
    UpgradeAccount,
    Friends,
    Leaderboard,
}

/// Session info (username / user id / guest flags).
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub username: String,
    pub user_id: String,
    pub is_guest: bool,
    pub guest_expires_at: Option<String>,
}

/// Lobby view (`lobbyId`, players, max players, host flag).
#[derive(Debug, Clone, Default)]
pub struct LobbyView {
    pub mode: String,
    pub draw_rule: domino_core::DrawRule,
    pub lobby_id: String,
    pub players: Vec<LobbyPlayer>,
    pub max_players: usize,
    pub is_host: bool,
}

/// Friend data bundle returned by `GET /api/friends` (+ socket refetches).
#[derive(Debug, Clone, Default)]
pub struct FriendsView {
    pub friends: Vec<FriendInfo>,
    pub incoming: Vec<FriendRequest>,
    pub outgoing: Vec<FriendRequest>,
}

// --------------------------------------------------------------- globals --

pub static SCREEN: GlobalSignal<Screen> = Signal::global(|| Screen::Entry);
pub static ACCOUNT_OPEN: GlobalSignal<bool> = Signal::global(|| false);
pub static SOUND_ENABLED: GlobalSignal<bool> = Signal::global(|| true);
pub static LOADING: GlobalSignal<bool> = Signal::global(|| true);
#[derive(Debug, Clone)]
pub struct TableReaction {
    pub emoji: String,
    pub stamp: f64,
}
pub static TABLE_REACTIONS: GlobalSignal<std::collections::BTreeMap<String, TableReaction>> =
    Signal::global(std::collections::BTreeMap::new);
pub static ERROR_MSG: GlobalSignal<String> = Signal::global(String::new);
pub static INVITE_NOTICE: GlobalSignal<Option<crate::invite::InviteProblem>> =
    Signal::global(|| None);

pub static SESSION: GlobalSignal<Session> = Signal::global(Session::default);
pub static EMOTE_USAGE: GlobalSignal<std::collections::BTreeMap<String, u64>> =
    Signal::global(std::collections::BTreeMap::new);

pub static LOBBY: GlobalSignal<LobbyView> = Signal::global(LobbyView::default);
pub static GAME_STATE: GlobalSignal<Option<SGameState>> = Signal::global(|| None);
pub static STOP_VOTE: GlobalSignal<Option<StopVote>> = Signal::global(|| None);

pub static FRIENDS: GlobalSignal<FriendsView> = Signal::global(FriendsView::default);
pub static GAME_INVITE_TOASTS: GlobalSignal<Vec<GameInvite>> = Signal::global(Vec::new);
pub static SHOW_INVITE_MODAL: GlobalSignal<bool> = Signal::global(|| false);

pub static MATCH_HISTORY: GlobalSignal<Vec<MatchRow>> = Signal::global(Vec::new);
pub static PLAYER_STATS: GlobalSignal<Option<PlayerStats>> = Signal::global(|| None);
pub static LEADERBOARD: GlobalSignal<Vec<LeaderEntry>> = Signal::global(Vec::new);

pub fn nav(screen: Screen) {
    ERROR_MSG.signal().set(String::new());
    SCREEN.signal().set(screen);
    crate::browser::window().scroll_to_with_x_and_y(0., 0.);
}

/// Navigate to a screen and reset per-lobby state (used by exit/return flows).
pub fn reset_to_menu() {
    LOBBY.signal().set(LobbyView::default());
    GAME_STATE.signal().set(None);
    STOP_VOTE.signal().set(None);
    TABLE_REACTIONS.signal().write().clear();
    nav(Screen::MainMenu);
}

/// Parse lobby player rows from a REST/socket payload value.
pub fn parse_players(value: &serde_json::Value) -> Vec<LobbyPlayer> {
    match value.as_array() {
        Some(rows) => rows
            .iter()
            .filter_map(|p| {
                let id = p.get("id")?.as_str()?;
                let username = p.get("username")?.as_str()?;
                Some(LobbyPlayer {
                    id: id.to_string(),
                    username: username.to_string(),
                })
            })
            .collect(),
        None => Vec::new(),
    }
}

/// Apply `{ token, user }` responses (login/register/guest).
pub fn apply_auth(data: &serde_json::Value) {
    let user = &data["user"];
    if user["id"]
        .as_str()
        .is_some_and(|id| id != SESSION.read().user_id)
    {
        EMOTE_USAGE.signal().write().clear();
    }
    let mut s = SESSION.read().clone();
    if let Ok(u) = serde_json::from_value::<SUser>(user.clone()) {
        s.username = u.username;
        s.user_id = u.id;
        s.is_guest = u.is_guest;
        s.guest_expires_at = u.guest_expires_at;
        SESSION.signal().set(s);
    } else {
        // Defensive fallback for slightly different payload shapes.
        if let Some(u) = user["username"].as_str() {
            s.username = u.to_string();
        }
        if let Some(u) = user["id"].as_str() {
            s.user_id = u.to_string();
        }
        s.is_guest = user["isGuest"].as_bool().unwrap_or(s.is_guest);
        s.guest_expires_at = user["guestExpiresAt"].as_str().map(String::from);
        SESSION.signal().set(s);
    }
    if let Some(t) = data["token"].as_str() {
        crate::net::set_token(t);
    }
}

/// Look up a display name for a player id from the current lobby roster.
pub fn player_label(id: &str, username: &str) -> String {
    if let Some(number) = id.strip_prefix("ai_") {
        crate::i18n::tf("Bot {number}", &[("number", number.to_owned())])
    } else {
        username.to_owned()
    }
}

pub fn player_name(id: &str) -> String {
    let lobby = LOBBY.read();
    for p in lobby.players.iter() {
        if p.id == id {
            return player_label(id, &p.username);
        }
    }
    id.to_string()
}

/// Supported table reactions.
pub const REACTIONS: [&str; 16] = domino_core::REACTIONS;
