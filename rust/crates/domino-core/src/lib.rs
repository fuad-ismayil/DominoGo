//! Authoritative DominoGo rules, reproducible shuffles and fair-information AI.
//!
//! Game states retain the original persisted JSON schema. Rule enforcement and
//! bot strategy intentionally improve on the original implementation: legal
//! passing prevents fraudulent blocked rounds, and bots preserve
//! follow-up moves without reading hidden information.

pub mod ai;
pub mod engine;
pub mod rng;
pub mod telefon;
pub mod types;

pub use ai::{get_ai_move, AIAction};
pub use engine::{
    can_play_tile, check_game_over, create_game, create_game_with, draw_count_for,
    draw_from_market, generate_tiles, generate_tiles_with, legal_moves, next_round,
    next_round_with, pass_turn, play_tile, start_round, start_round_with, try_create_game_with,
    DominoError, End, FIRST_ROUND_DOUBLE_PRIORITY,
};
pub use rng::GameRng;
pub use types::{
    DrawRule, GameMode, GameState, GameStatus, PlayerScore, TelefonNode, Tile, REACTIONS,
};
