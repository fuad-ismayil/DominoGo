//! Legal control states derived from the shared rules using only visible data.
use crate::types::{SGameState, Tile};
use domino_core::{draw_count_for, legal_moves, End, GameMode, GameState, GameStatus};

/// Evaluate the local hand without inventing any hidden opponent information.
/// `legal_moves` reads only this hand, the board, round and match status.
pub fn placements(state: &SGameState, user_id: &str) -> Vec<(Tile, End)> {
    if state.status != "playing"
        || state.players.get(state.turn_index).map(String::as_str) != Some(user_id)
    {
        return Vec::new();
    }
    let view = GameState {
        mode: if state.mode == "telefon" {
            GameMode::Telefon
        } else {
            GameMode::OneOhOne
        },
        telefon: state.telefon.clone(),
        draw_rule: state.draw_rule,
        players: state.players.clone(),
        hands: state.hands.clone(),
        market: Vec::new(),
        board: state.board.clone(),
        open_ends: state.open_ends,
        scores: Default::default(),
        turn_index: state.turn_index,
        status: GameStatus::Playing,
        round_number: state.round_number,
        last_winner: state.last_winner.clone(),
        consecutive_passes: 0,
    };
    legal_moves(&view, user_id)
}

pub fn can_play_sides(state: &SGameState, user_id: &str, tile: Tile) -> (bool, bool) {
    let moves = placements(state, user_id);
    (
        moves.contains(&(tile, End::Left)),
        moves.contains(&(tile, End::Right)),
    )
}

pub fn can_draw(state: &SGameState, user_id: &str) -> bool {
    state.status == "playing"
        && state.players.get(state.turn_index).map(String::as_str) == Some(user_id)
        && (state.draw_rule == domino_core::DrawRule::Anytime
            || placements(state, user_id).is_empty())
        && draw_count_for(state.players.len()) > 0
        && state.market_count > 0
}

pub fn can_pass(state: &SGameState, user_id: &str) -> bool {
    state.status == "playing"
        && state.players.get(state.turn_index).map(String::as_str) == Some(user_id)
        && placements(state, user_id).is_empty()
        && (draw_count_for(state.players.len()) == 0 || state.market_count == 0)
}

/// Round review follows the scoring rule for a lone blank.
pub fn hand_points(hand: &[Tile]) -> i32 {
    if hand == [[0, 0]] {
        10
    } else {
        hand.iter().map(|t| i32::from(t[0]) + i32::from(t[1])).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> SGameState {
        SGameState {
            players: vec!["me".into(), "other".into()],
            hands: [("me".into(), vec![[1, 2], [3, 3], [0, 0]])].into(),
            status: "playing".into(),
            round_number: 1,
            ..Default::default()
        }
    }
    #[test]
    fn revealed_hand_values_match_round_scoring() {
        assert_eq!(hand_points(&[[0, 0]]), 10);
        assert_eq!(hand_points(&[[0, 0], [2, 3]]), 5);
        assert_eq!(hand_points(&[]), 0);
    }
    #[test]
    fn opening_uses_shared_double_priority() {
        let state = view();
        assert_eq!(can_play_sides(&state, "me", [3, 3]), (true, false));
        assert_eq!(can_play_sides(&state, "me", [0, 0]), (false, false));
        assert_eq!(can_play_sides(&state, "me", [1, 2]), (false, false));
        assert!(!can_draw(&state, "me"));
        assert!(!can_pass(&state, "me"));
    }
    #[test]
    fn drawing_is_optional_even_with_a_playable_tile() {
        let mut state = view();
        state.market_count = 10;
        assert!(can_draw(&state, "me"));
        assert!(!can_pass(&state, "me"));
        state.turn_index = 1;
        assert!(!can_draw(&state, "me"));
        state.turn_index = 0;
        state.players.extend(["third".into(), "fourth".into()]);
        assert!(!can_draw(&state, "me"));
    }
    #[test]
    fn restricted_table_disables_drawing_only_with_a_legal_move() {
        let mut state = view();
        state.draw_rule = domino_core::DrawRule::WhenBlocked;
        state.market_count = 14;
        assert!(!can_draw(&state, "me"));
        state.board = vec![[6, 6]];
        state.open_ends = [Some(6), Some(6)];
        assert!(can_draw(&state, "me"));
        assert!(!can_pass(&state, "me"));
    }
    #[test]
    fn final_market_tile_remains_drawable_in_two_player_game() {
        let mut state = view();
        state.board = vec![[6, 6]];
        state.open_ends = [Some(6), Some(6)];
        state.market_count = 1;
        assert!(can_draw(&state, "me"));
        assert!(!can_pass(&state, "me"));
        state.market_count = 0;
        assert!(!can_draw(&state, "me"));
        assert!(can_pass(&state, "me"));
    }
    #[test]
    fn controls_reject_foreign_turn_finished_game_and_unowned_tile() {
        let mut state = view();
        assert_eq!(can_play_sides(&state, "me", [1, 1]), (false, false));
        state.turn_index = 1;
        assert_eq!(can_play_sides(&state, "me", [3, 3]), (false, false));
        assert!(!can_pass(&state, "me"));
        state.turn_index = 0;
        state.status = "round_end".into();
        assert_eq!(can_play_sides(&state, "me", [3, 3]), (false, false));
        assert!(!can_draw(&state, "me"));
        assert!(!can_pass(&state, "me"));
    }
    #[test]
    fn two_ends_match_independently() {
        let mut state = view();
        state.board = vec![[1, 3]];
        state.open_ends = [Some(1), Some(3)];
        assert_eq!(can_play_sides(&state, "me", [1, 2]), (true, false));
        assert_eq!(can_play_sides(&state, "me", [3, 3]), (false, true));
    }
}
