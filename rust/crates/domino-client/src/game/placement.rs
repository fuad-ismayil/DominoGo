//! Pure placement-preview orientation and authoritative reconciliation.
use crate::types::{SGameState, Tile};
use domino_core::End;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingPlacement {
    pub tile: Tile,
    pub end: End,
    pub round_number: u32,
    pub board_len: usize,
}

impl PendingPlacement {
    pub fn new(state: &SGameState, tile: Tile, end: End) -> Self {
        Self {
            tile,
            end,
            round_number: state.round_number,
            board_len: state.board.len(),
        }
    }

    /// A repeated or unrelated snapshot must not erase the local preview.
    pub fn resolved(&self, state: &SGameState, user: &str) -> bool {
        state.round_number != self.round_number
            || state.status != "playing"
            || state.players.get(state.turn_index).map(String::as_str) != Some(user)
            || !state.hands.get(user).is_some_and(|hand| {
                hand.iter()
                    .any(|tile| *tile == self.tile || *tile == [self.tile[1], self.tile[0]])
            })
    }
}

/// Board order is left to right: the touching pip faces inward.
/// An opening tile keeps the orientation selected by the player.
pub fn preview_tile(tile: Tile, end: End, open_ends: [Option<u8>; 2]) -> Tile {
    match end {
        End::Left if open_ends[0].is_some_and(|pip| tile[0] == pip && tile[1] != pip) => {
            [tile[1], tile[0]]
        }
        End::Right if open_ends[1].is_some_and(|pip| tile[1] == pip && tile[0] != pip) => {
            [tile[1], tile[0]]
        }
        _ => tile,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playing() -> SGameState {
        SGameState {
            status: "playing".into(),
            round_number: 2,
            players: vec!["me".into(), "other".into()],
            hands: [("me".into(), vec![[2, 5], [0, 0]])].into(),
            board: vec![[2, 3]],
            open_ends: [Some(2), Some(3)],
            ..Default::default()
        }
    }

    #[test]
    fn preview_orients_both_tile_orders_at_each_end() {
        for tile in [[2, 5], [5, 2]] {
            assert_eq!(preview_tile(tile, End::Left, [Some(2), Some(2)]), [5, 2]);
            assert_eq!(preview_tile(tile, End::Right, [Some(2), Some(2)]), [2, 5]);
        }
    }

    #[test]
    fn preview_preserves_opening_doubles_and_handles_blank() {
        for end in [End::Left, End::Right] {
            assert_eq!(preview_tile([5, 2], end, [None, None]), [5, 2]);
            assert_eq!(preview_tile([0, 0], end, [Some(0), Some(0)]), [0, 0]);
            assert_eq!(preview_tile([4, 4], end, [Some(4), Some(4)]), [4, 4]);
        }
        assert_eq!(preview_tile([0, 6], End::Left, [Some(0), Some(2)]), [6, 0]);
        assert_eq!(preview_tile([6, 0], End::Right, [Some(2), Some(0)]), [0, 6]);
    }

    #[test]
    fn unrelated_snapshot_keeps_preview_including_reversed_hand_tile() {
        let mut state = playing();
        let pending = PendingPlacement::new(&state, [2, 5], End::Left);
        assert!(!pending.resolved(&state, "me"));
        state.market_count = 9;
        state.turn_deadline = Some(123.0);
        state.hands.get_mut("me").unwrap()[0] = [5, 2];
        assert!(!pending.resolved(&state, "me"));
        assert_eq!(pending.board_len, 1);
    }

    #[test]
    fn authoritative_placement_or_round_change_resolves_preview() {
        let state = playing();
        let pending = PendingPlacement::new(&state, [2, 5], End::Left);
        let mut placed = state.clone();
        placed.board.insert(0, [5, 2]);
        placed.hands.get_mut("me").unwrap().remove(0);
        assert!(pending.resolved(&placed, "me"));
        let mut next_round = state.clone();
        next_round.round_number += 1;
        assert!(pending.resolved(&next_round, "me"));
        let mut next_turn = state.clone();
        next_turn.turn_index = 1;
        assert!(pending.resolved(&next_turn, "me"));
        let mut ended = state;
        ended.status = "round_end".into();
        assert!(pending.resolved(&ended, "me"));
    }
}
