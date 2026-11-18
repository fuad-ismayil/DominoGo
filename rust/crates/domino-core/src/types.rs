//! Core game types — serde-compatible with the JSON produced by the TypeScript
//! engine (`src/game/engine.ts`) and persisted in `game_states.state_json`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A domino. Index 0 == left pip, index 1 == right pip (values 0–6).
///
/// Serialized as a 2-element JSON array (e.g. `[[6,6],[5,2]]`), matching the TS
/// `Tile = [number, number]` tuple exactly.
pub type Tile = [u8; 2];

/// The only game mode implemented by the TypeScript engine today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMode {
    /// Serialized as `"101"` to match the TS string literal `'101'`.
    #[serde(rename = "101")]
    OneOhOne,
    #[serde(rename = "telefon")]
    Telefon,
}

impl GameMode {
    pub fn target(self) -> i32 {
        match self {
            Self::OneOhOne => 101,
            Self::Telefon => 365,
        }
    }
}

/// Stable, chronological tile IDs. A child enters through port 0; port 1 is
/// its continuation. Doubles also have two scoring-only side ports, 2 and 3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelefonNode {
    pub tile: Tile,
    pub parent: Option<usize>,
    pub port: u8,
}

pub const REACTIONS: [&str; 16] = [
    "👍", "👎", "😤", "🎉", "😂", "🔥", "💀", "🤝", "😭", "🥲", "😎", "🤯", "😅", "😈", "👏", "❤️",
];

/// A player's 101-mode score. Serialized with the same keys as TS.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerScore {
    /// Carries round points until they exceed 12 (then promoted to `permanent`).
    pub temporary: i32,
    /// Banked points; reaching 101 ends the match.
    pub permanent: i32,
}

/// Overall match status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameStatus {
    Playing,
    RoundEnd,
    Finished,
}

/// Fixed table rule; older saved matches retain voluntary drawing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawRule {
    #[default]
    Anytime,
    WhenBlocked,
}

impl DrawRule {
    pub fn description(self) -> &'static str {
        match self {
            Self::Anytime => "Draw even with a matching tile",
            Self::WhenBlocked => "Draw only when you cannot play",
        }
    }
}

/// Full authoritative state of a match.
///
/// Field names are `camelCase` on the wire (`openEnds`, `turnIndex`, …) so that
/// states written by the Node.js server remain deserializable and vice versa.
/// `hands`/`scores` use `BTreeMap` instead of `HashMap` for deterministic JSON
/// output across runs (insertion order is not part of game semantics).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    pub mode: GameMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telefon: Vec<TelefonNode>,
    #[serde(default)]
    pub draw_rule: DrawRule,
    /// Turn order; player IDs are opaque strings (`"ai_1"`, user UUIDs, …).
    pub players: Vec<String>,
    /// Each player's hand. Deal order is deterministic, but membership is all
    /// that matters to the rules.
    pub hands: BTreeMap<String, Vec<Tile>>,
    /// Undealt tiles; draws take from the **front** (index 0).
    pub market: Vec<Tile>,
    /// Played tiles in left-to-right order, each stored in **board orientation**
    /// (`[0]` points left, `[1]` points right).
    pub board: Vec<Tile>,
    /// `[left_end, right_end]`. `None` on an empty board. Once both are set they
    /// stay `Some` for the rest of the round.
    pub open_ends: [Option<u8>; 2],
    pub scores: BTreeMap<String, PlayerScore>,
    /// Index into `players` of whose turn it is.
    pub turn_index: usize,
    pub status: GameStatus,
    pub round_number: u32,
    /// Winner of the most recent round (also the round-start player afterwards).
    pub last_winner: Option<String>,
    /// Consecutive passes; a full lap of passes blocks the game.
    pub consecutive_passes: u32,
}
