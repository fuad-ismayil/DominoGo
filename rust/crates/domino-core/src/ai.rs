//! Deterministic, information-fair bot policy.

use crate::engine::{draw_count_for, End};
use crate::types::{GameState, Tile};

/// The action an AI bot wants to perform. Serialized with a `"type"` tag to
/// mirror the TS union shapes: `{"type":"play","tile":[..],"end":"left"}`,
/// `{"type":"draw"}`, `{"type":"pass"}`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AIAction {
    Play { tile: Tile, end: End },
    Draw,
    Pass,
}

/// Choose a move using only the bot's hand and public board. The deterministic
/// policy sheds expensive tiles while preserving follow-up moves and avoiding
/// stranded doubles. It never inspects opponents' hands or the market contents.
#[must_use]
pub fn get_ai_move(state: &GameState, ai_id: &str) -> AIAction {
    let Some(hand) = state.hands.get(ai_id) else {
        return AIAction::Pass;
    };
    let moves = crate::engine::legal_moves(state, ai_id);
    let mut best = None;
    let mut best_score = i32::MIN;
    for (tile, end) in moves {
        let mut ends = state.open_ends;
        if state.board.is_empty() {
            ends = [Some(tile[0]), Some(tile[1])];
        } else {
            let index = if end == End::Left { 0 } else { 1 };
            ends[index] = Some(if ends[index] == Some(tile[0]) {
                tile[1]
            } else {
                tile[0]
            });
        }
        let mut removed = false;
        let remaining: Vec<_> = hand
            .iter()
            .filter(|&&t| {
                if !removed && t == tile {
                    removed = true;
                    false
                } else {
                    true
                }
            })
            .copied()
            .collect();
        let mobility = remaining
            .iter()
            .filter(|&&t| crate::engine::can_play_tile(t, ends))
            .count() as i32;
        let stranded_doubles = remaining
            .iter()
            .filter(|&&t| t[0] == t[1] && !crate::engine::can_play_tile(t, ends))
            .count() as i32;
        let score = if remaining.is_empty() {
            10000
        } else {
            4 * (i32::from(tile[0]) + i32::from(tile[1])) + 3 * mobility - 5 * stranded_doubles
                + if tile[0] == tile[1] { 4 } else { 0 }
                - if mobility == 0 { 20 } else { 0 }
        };
        if score > best_score {
            best_score = score;
            best = Some(AIAction::Play { tile, end });
        }
    }
    if let Some(action) = best {
        return action;
    }
    if draw_count_for(state.players.len()) > 0 && !state.market.is_empty() {
        AIAction::Draw
    } else {
        AIAction::Pass
    }
}
