//! Server response types. Hands stay private during play and are public once a round ends.

use serde::Deserialize;
use std::collections::BTreeMap;

/// A domino (`[number, number]` on the wire).
pub type Tile = [u8; 2];

/// Sanitized per-user game state received via `game_state_update`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SGameState {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub draw_rule: domino_core::DrawRule,
    #[serde(default)]
    pub players: Vec<String>,
    #[serde(default)]
    pub hands: BTreeMap<String, Vec<Tile>>,
    #[serde(default)]
    pub market: Vec<Tile>,
    #[serde(default)]
    pub board: Vec<Tile>,
    #[serde(default)]
    pub last_played_tile: Option<Tile>,
    #[serde(default)]
    pub round_finishing_tile: Option<Tile>,
    #[serde(default)]
    pub open_ends: [Option<u8>; 2],
    #[serde(default)]
    pub scores: BTreeMap<String, SScore>,
    #[serde(default)]
    pub turn_index: usize,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub round_number: u32,
    #[serde(default)]
    pub last_winner: Option<String>,
    #[serde(default)]
    pub opponent_hand_counts: BTreeMap<String, i64>,
    #[serde(default)]
    pub market_count: i64,
    #[serde(default)]
    pub turn_deadline: Option<f64>,
}

/// `{ temporary, permanent }` 101-mode score.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct SScore {
    #[serde(default)]
    pub temporary: i32,
    #[serde(default)]
    pub permanent: i32,
}

/// Auth response user (`register`/`login`/`guest`/`me`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SUser {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub is_guest: bool,
    #[serde(default)]
    pub guest_expires_at: Option<String>,
}

/// `GET /api/friends` row.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub is_online: bool,
}

/// Incoming/outgoing friend request row.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendRequest {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub username: String,
}

/// Pending game invitation (socket `game_invitation` + `/api/friends`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInvite {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub lobby_id: String,
    #[serde(default)]
    pub sender_username: String,
}

/// Socket `stop_vote_update` payload.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StopVote {
    #[serde(default)]
    pub votes: i64,
    #[serde(default)]
    pub required: i64,
    #[serde(default)]
    pub voters: Vec<String>,
    #[serde(default)]
    pub stopped: bool,
}

/// `GET /api/matches/history` row.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct MatchRow {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub players: Vec<MatchPlayer>,
    #[serde(default)]
    pub scores: Option<BTreeMap<String, SScore>>,
    #[serde(default)]
    pub winner: Option<String>,
}

/// Player inside a match-history row.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct MatchPlayer {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub username: String,
}

/// `GET /api/stats/:id`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStats {
    #[serde(default)]
    pub wins: i64,
    #[serde(default)]
    pub losses: i64,
    #[serde(default)]
    pub total_score: i64,
    #[serde(default)]
    pub games_played: i64,
    #[serde(default)]
    pub win_rate: i64,
    #[serde(default)]
    pub avg_score: i64,
}

/// `GET /api/leaderboard` row.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderEntry {
    #[serde(default)]
    pub rank: i64,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub wins: i64,
    #[serde(default)]
    pub losses: i64,
    #[serde(default)]
    pub games_played: i64,
    #[serde(default)]
    pub win_rate: i64,
    #[serde(default)]
    pub total_score: i64,
    #[serde(default)]
    pub avg_score: i64,
}

/// A player row in a lobby (`{id, username}`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LobbyPlayer {
    pub id: String,
    pub username: String,
}
