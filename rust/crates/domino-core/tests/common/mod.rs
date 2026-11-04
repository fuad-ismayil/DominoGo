//! Shared helpers for the integration tests.

#![allow(dead_code)]

use domino_core::{GameMode, GameState, GameStatus, PlayerScore};
use std::collections::BTreeMap;

/// Build a clean `playing` state with named players and zeroed scores.
#[must_use]
pub fn base_state(players: &[&str]) -> GameState {
    let mut hands = BTreeMap::new();
    let mut scores = BTreeMap::new();
    for p in players {
        hands.insert((*p).to_string(), Vec::new());
        scores.insert((*p).to_string(), PlayerScore::default());
    }
    GameState {
        mode: GameMode::OneOhOne,
        draw_rule: Default::default(),
        players: players.iter().map(|p| (*p).to_string()).collect(),
        hands,
        market: Vec::new(),
        board: Vec::new(),
        open_ends: [None, None],
        scores,
        turn_index: 0,
        status: GameStatus::Playing,
        round_number: 1,
        last_winner: None,
        consecutive_passes: 0,
    }
}

/// Assert the structural invariants that must hold after every engine step.
pub fn assert_invariants(state: &GameState) {
    // Tile conservation: the 28-tile set is never created or lost.
    let in_hands: usize = state.hands.values().map(|h| h.len()).sum();
    assert_eq!(
        in_hands + state.market.len() + state.board.len(),
        28,
        "28 tiles must always be accounted for"
    );

    let mut tiles: Vec<_> = state
        .hands
        .values()
        .flatten()
        .chain(state.market.iter())
        .chain(state.board.iter())
        .map(|&t| {
            assert!(t[0] <= 6 && t[1] <= 6, "tile pips must be double-six");
            if t[0] <= t[1] {
                t
            } else {
                [t[1], t[0]]
            }
        })
        .collect();
    tiles.sort();
    assert!(
        tiles.windows(2).all(|w| w[0] != w[1]),
        "no duplicated tiles"
    );
    assert!(
        state.board.windows(2).all(|w| w[0][1] == w[1][0]),
        "chain joins must match"
    );
    assert!(
        state.turn_index < state.players.len(),
        "turn must name a player"
    );

    // Orientation bookkeeping: open ends agree with the board's outer pips.
    if let Some(head) = state.board.first() {
        assert_eq!(Some(head[0]), state.open_ends[0], "left end vs board head");
        let tail = state.board.last().expect("board non-empty implies a tail");
        assert_eq!(
            tail[1],
            state.open_ends[1].expect("right end"),
            "right end vs board tail"
        );
    } else {
        assert_eq!(
            state.open_ends,
            [None, None],
            "empty board has no open ends"
        );
    }

    // Every score component is non-negative; ≥101 permanent is impossible in a
    // *playing* state (round_end/finished states pass through checkGameOver).
    assert_eq!(
        state.scores.len(),
        state.players.len(),
        "one score per player"
    );
    for score in state.scores.values() {
        assert!(
            score.temporary >= 0 && score.permanent >= 0,
            "scores non-negative"
        );
        assert!(
            score.permanent < 101 || state.status != GameStatus::Playing,
            "permanent ≥ 101 while still playing"
        );
    }
}
