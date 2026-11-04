//! Authoritative double-six domino rules and 101 scoring.
//!
//! Persisted game states remain compatible with the original game. The rules
//! preserve its regional scoring conventions while enforcing legal play:
//!
//! * the round-1 opening double uses the priority order
//!   `[1,1] → [6,6] → [0,0]` (i.e. `[0,0]` is checked **last**);
//! * a lone `[0,0]` left in a loser's hand counts 10 points;
//! * blocked games give the round to the player whose hand pip-sum is lowest
//!   (first player in turn order wins ties);
//! * 101 scoring: a round win with `permanent > 0` banks points directly,
//!   otherwise points accumulate in `temporary` until they exceed 12, whereupon
//!   they are promoted to `permanent`.

use crate::rng::GameRng;
use crate::types::{GameMode, GameState, GameStatus, PlayerScore, Tile};
use std::collections::BTreeMap;

/// Which end of the chain a tile attaches to. Serialized as `"left"`/`"right"`
/// to match the client's `end` strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum End {
    Left,
    Right,
}

/// Errors thrown by the TypeScript engine as `Error(message)`; `Display` strings
/// are identical so existing clients can surface them unchanged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DominoError {
    #[error("Game is not in playing state")]
    NotPlaying,
    #[error("Not your turn")]
    NotYourTurn,
    #[error("Tile not in hand")]
    TileNotInHand,
    #[error("First round must start with a double")]
    MustStartWithADouble,
    #[error("First round must start with your lowest double ({0}-{1})")]
    MustStartWithLowestDouble(u8, u8),
    #[error("Invalid move on left end")]
    InvalidMoveOnLeftEnd,
    #[error("Invalid move on right end")]
    InvalidMoveOnRightEnd,
    #[error("Market is empty")]
    MarketEmpty,
    #[error("No market draws allowed in 4-player game")]
    NoMarketDrawsAllowed,
    #[error("Play a matching tile first")]
    PlayableTileAvailable,
    #[error("Draw from the market before passing")]
    MustDrawBeforePassing,
    #[error("A game requires two to four distinct, nonempty player IDs")]
    InvalidPlayers,
    #[error("Round has not ended yet")]
    RoundNotEnded,
}

/// Doubles in the exact priority order hard-coded in the TS engine — note that
/// `[0,0]` is deliberately checked last.
pub const FIRST_ROUND_DOUBLE_PRIORITY: [Tile; 7] =
    [[1, 1], [2, 2], [3, 3], [4, 4], [5, 5], [6, 6], [0, 0]];

/// The first double in [`FIRST_ROUND_DOUBLE_PRIORITY`] present in `hand`.
fn lowest_double(hand: &[Tile]) -> Option<Tile> {
    FIRST_ROUND_DOUBLE_PRIORITY
        .iter()
        .copied()
        .find(|d| hand.iter().any(|t| t[0] == d[0] && t[1] == d[1]))
}

/// Draws allowed per market draw, by player count (`2 → 2`, `3 → 1`, else `0`).
#[must_use]
pub fn draw_count_for(player_count: usize) -> usize {
    match player_count {
        2 => 2,
        3 => 1,
        _ => 0,
    }
}

/// Total pip sum of a tile collection.
fn pip_sum(tiles: &[Tile]) -> i32 {
    tiles.iter().map(|t| t[0] as i32 + t[1] as i32).sum()
}

/// Create the full 28-tile double-six set in `i ≤ j` order, shuffled with
/// Fisher–Yates over OS entropy. Same construction as `generateTiles()`.
pub fn generate_tiles() -> Vec<Tile> {
    generate_tiles_with(&mut GameRng::from_entropy())
}

/// Seeded variant of [`generate_tiles`] for tests/replays.
pub fn generate_tiles_with(rng: &mut GameRng) -> Vec<Tile> {
    let mut tiles: Vec<Tile> = Vec::with_capacity(28);
    for i in 0u8..=6 {
        for j in i..=6 {
            tiles.push([i, j]);
        }
    }
    rng.shuffle(&mut tiles);
    tiles
}

/// Shuffle/redistribute hands for the next round (`startRound`).
///
/// Round 1 keeps re-shuffling until at least one player holds a double, then
/// sets the starting player to the **first player in turn order** holding the
/// highest-priority double. Later rounds start with `lastWinner`.
pub fn start_round_with(state: &GameState, rng: &mut GameRng) -> GameState {
    let players = &state.players;
    let ids: Vec<&str> = players.iter().map(String::as_str).collect();
    assert!(
        valid_players(&ids),
        "a round requires two to four distinct players"
    );
    let mut hands: BTreeMap<String, Vec<Tile>>;
    let mut market: Vec<Tile>;
    let mut starting_player_index: usize = 0;

    loop {
        let tiles = generate_tiles_with(rng);
        hands = players.iter().map(|p| (p.clone(), Vec::new())).collect();

        let mut next = tiles.iter().copied();
        for _ in 0..7 {
            for p in players {
                hands
                    .get_mut(p)
                    .expect("every player has a hand")
                    .push(next.next().expect("28 tiles for ≤4 players × 7 draws"));
            }
        }
        market = tiles[(players.len() * 7)..].to_vec();

        if state.round_number == 1 {
            let mut found = false;
            'outer: for d in FIRST_ROUND_DOUBLE_PRIORITY {
                for (i, p) in players.iter().enumerate() {
                    if hands[p].iter().any(|t| t[0] == d[0] && t[1] == d[1]) {
                        starting_player_index = i;
                        found = true;
                        break 'outer;
                    }
                }
            }
            if found {
                break;
            }
        } else {
            // TS: players.indexOf(state.lastWinner || players[0]); -1 → 0.
            starting_player_index = players
                .iter()
                .position(|p| state.last_winner.as_ref() == Some(p))
                .unwrap_or(0);
            break;
        }
    }

    GameState {
        hands,
        market,
        board: Vec::new(),
        open_ends: [None, None],
        turn_index: starting_player_index,
        status: GameStatus::Playing,
        consecutive_passes: 0,
        ..state.clone()
    }
}

/// Entropy-seeded alias of [`start_round_with`].
pub fn start_round(state: &GameState) -> GameState {
    start_round_with(state, &mut GameRng::from_entropy())
}

fn valid_players(players: &[&str]) -> bool {
    (2..=4).contains(&players.len())
        && players.iter().all(|p| !p.trim().is_empty())
        && players
            .iter()
            .enumerate()
            .all(|(i, p)| !players[..i].contains(p))
}

/// Validated constructor for untrusted lobby membership.
pub fn try_create_game_with(
    players: &[&str],
    mode: GameMode,
    rng: &mut GameRng,
) -> Result<GameState, DominoError> {
    if !valid_players(players) {
        return Err(DominoError::InvalidPlayers);
    }
    Ok(create_game_with(players, mode, rng))
}

/// Build a fresh match (`createGame`) — all scores zeroed, then deal round 1.
pub fn create_game_with(players: &[&str], mode: GameMode, rng: &mut GameRng) -> GameState {
    assert!(
        valid_players(players),
        "a game requires two to four distinct, nonempty player IDs"
    );
    let players: Vec<String> = players.iter().map(|s| (*s).to_string()).collect();
    let mut scores: BTreeMap<String, PlayerScore> = BTreeMap::new();
    for p in &players {
        scores.insert(p.clone(), PlayerScore::default());
    }
    let initial = GameState {
        mode,
        draw_rule: Default::default(),
        players,
        hands: BTreeMap::new(),
        market: Vec::new(),
        board: Vec::new(),
        open_ends: [None, None],
        scores,
        turn_index: 0,
        status: GameStatus::Playing,
        round_number: 1,
        last_winner: None,
        consecutive_passes: 0,
    };
    start_round_with(&initial, rng)
}

/// Entropy-seeded alias of [`create_game_with`].
pub fn create_game(players: &[&str], mode: GameMode) -> GameState {
    create_game_with(players, mode, &mut GameRng::from_entropy())
}

/// 101-scoring transition shared by domino wins and blocked games.
fn apply_101_scoring(
    mut scores: BTreeMap<String, PlayerScore>,
    winner_id: &str,
    round_points: i32,
    players: &[String],
) -> BTreeMap<String, PlayerScore> {
    for p in players {
        if p.as_str() != winner_id {
            if let Some(s) = scores.get_mut(p) {
                s.temporary = 0;
            }
        }
    }
    if let Some(winner_score) = scores.get_mut(winner_id) {
        if winner_score.permanent > 0 {
            winner_score.temporary = 0;
            winner_score.permanent += round_points;
        } else {
            let new_total = winner_score.temporary + round_points;
            if new_total > 12 {
                *winner_score = PlayerScore {
                    temporary: 0,
                    permanent: new_total,
                };
            } else {
                winner_score.temporary = new_total;
            }
        }
    }
    scores
}

/// Does `tile` match an open end? Use [`legal_moves`] for opening constraints.
pub fn can_play_tile(tile: Tile, open_ends: [Option<u8>; 2]) -> bool {
    if open_ends[0].is_none() && open_ends[1].is_none() {
        return true;
    }
    let touches = |end: Option<u8>| end.is_some_and(|e| tile[0] == e || tile[1] == e);
    touches(open_ends[0]) || touches(open_ends[1])
}

/// Legal placements for a hand, including the first-round opening constraint.
/// This only reads the named player's hand and public board information.
#[must_use]
pub fn legal_moves(state: &GameState, player_id: &str) -> Vec<(Tile, End)> {
    if state.status != GameStatus::Playing {
        return Vec::new();
    }
    let Some(hand) = state.hands.get(player_id) else {
        return Vec::new();
    };
    if state.board.is_empty() && state.round_number == 1 {
        return lowest_double(hand)
            .map(|t| vec![(t, End::Left)])
            .unwrap_or_default();
    }
    let mut moves = Vec::new();
    for &tile in hand {
        if state.board.is_empty() {
            moves.push((tile, End::Left));
        } else {
            for (index, end) in [(0, End::Left), (1, End::Right)] {
                if state.open_ends[index].is_some_and(|pip| tile.contains(&pip)) {
                    moves.push((tile, end));
                }
            }
        }
    }
    moves
}

/// Play a tile from the player's hand onto one end (`playTile`).
///
/// This function never mutates: on success the caller gets the new state; on
/// failure the exact TS error message is returned as a [`DominoError`].
pub fn play_tile(
    state: &GameState,
    player_id: &str,
    tile: Tile,
    end: End,
) -> Result<GameState, DominoError> {
    if state.status != GameStatus::Playing {
        return Err(DominoError::NotPlaying);
    }
    if state.players.get(state.turn_index).map(String::as_str) != Some(player_id) {
        return Err(DominoError::NotYourTurn);
    }

    let hand = state.hands.get(player_id).cloned().unwrap_or_default();
    // Membership check is orientation-insensitive, exactly like the TS
    // `findIndex` with both-ordering comparison.
    let tile_index = hand
        .iter()
        .position(|t| (t[0] == tile[0] && t[1] == tile[1]) || (t[0] == tile[1] && t[1] == tile[0]))
        .ok_or(DominoError::TileNotInHand)?;

    let actual_tile = hand[tile_index];
    let mut new_open_ends = state.open_ends;
    let mut oriented_tile: Tile = actual_tile;

    // First move of round 1 must be the player's lowest-priority double.
    if state.board.is_empty() && state.round_number == 1 {
        match lowest_double(&hand) {
            Some(required) => {
                if actual_tile != required {
                    return Err(DominoError::MustStartWithLowestDouble(
                        required[0],
                        required[1],
                    ));
                }
            }
            None => {
                if actual_tile[0] != actual_tile[1] {
                    return Err(DominoError::MustStartWithADouble);
                }
            }
        }
    }

    if state.board.is_empty() {
        new_open_ends = [Some(actual_tile[0]), Some(actual_tile[1])];
    } else {
        match end {
            End::Left => {
                let left = state.open_ends[0];
                if left == Some(actual_tile[0]) {
                    new_open_ends[0] = Some(actual_tile[1]);
                    oriented_tile = [actual_tile[1], actual_tile[0]];
                } else if left == Some(actual_tile[1]) {
                    new_open_ends[0] = Some(actual_tile[0]);
                    oriented_tile = actual_tile;
                } else {
                    return Err(DominoError::InvalidMoveOnLeftEnd);
                }
            }
            End::Right => {
                let right = state.open_ends[1];
                if right == Some(actual_tile[0]) {
                    new_open_ends[1] = Some(actual_tile[1]);
                    oriented_tile = actual_tile;
                } else if right == Some(actual_tile[1]) {
                    new_open_ends[1] = Some(actual_tile[0]);
                    oriented_tile = [actual_tile[1], actual_tile[0]];
                } else {
                    return Err(DominoError::InvalidMoveOnRightEnd);
                }
            }
        }
    }

    let mut new_hand = hand;
    new_hand.remove(tile_index);
    let mut new_hands = state.hands.clone();
    new_hands.insert(player_id.to_string(), new_hand.clone());

    let new_board: Vec<Tile> = if state.board.is_empty() {
        vec![oriented_tile]
    } else {
        match end {
            End::Left => {
                let mut b = Vec::with_capacity(state.board.len() + 1);
                b.push(oriented_tile);
                b.extend_from_slice(&state.board);
                b
            }
            End::Right => {
                let mut b = state.board.clone();
                b.push(oriented_tile);
                b
            }
        }
    };

    let mut new_scores = state.scores.clone();
    let mut new_status = state.status;
    let mut last_winner = state.last_winner.clone();

    // Round win — "Domino!"
    if new_hand.is_empty() {
        new_status = GameStatus::RoundEnd;
        last_winner = Some(player_id.to_string());

        let mut round_points: i32 = 0;
        for p in &state.players {
            if p.as_str() == player_id {
                continue;
            }
            let loser_hand = &new_hands[p];
            round_points += if loser_hand.len() == 1 && loser_hand[0] == [0, 0] {
                10
            } else {
                pip_sum(loser_hand)
            };
        }
        new_scores = apply_101_scoring(new_scores, player_id, round_points, &state.players);
    }

    Ok(GameState {
        hands: new_hands,
        board: new_board,
        open_ends: new_open_ends,
        scores: new_scores,
        turn_index: if new_status == GameStatus::RoundEnd {
            state.turn_index
        } else {
            (state.turn_index + 1) % state.players.len()
        },
        status: new_status,
        last_winner,
        consecutive_passes: 0,
        ..state.clone()
    })
}

/// Draw tiles from the market (`drawFromMarket`). The turn does **not** advance
/// — the same player acts again after drawing, matching the TS behavior.
pub fn draw_from_market(state: &GameState, player_id: &str) -> Result<GameState, DominoError> {
    if state.status != GameStatus::Playing {
        return Err(DominoError::NotPlaying);
    }
    if state.players.get(state.turn_index).map(String::as_str) != Some(player_id) {
        return Err(DominoError::NotYourTurn);
    }
    if state.draw_rule == crate::DrawRule::WhenBlocked && !legal_moves(state, player_id).is_empty()
    {
        return Err(DominoError::PlayableTileAvailable);
    }
    if state.market.is_empty() {
        return Err(DominoError::MarketEmpty);
    }

    let draw_count = draw_count_for(state.players.len());
    if draw_count == 0 {
        return Err(DominoError::NoMarketDrawsAllowed);
    }
    let actual = draw_count.min(state.market.len());

    let mut new_hands = state.hands.clone();
    new_hands
        .entry(player_id.to_string())
        .or_default()
        .extend(state.market.iter().take(actual).copied());

    let new_market = state.market[actual..].to_vec();
    Ok(GameState {
        hands: new_hands,
        market: new_market,
        ..state.clone()
    })
}

/// Pass the turn (`passTurn`); a full lap of consecutive passes blocks the game
/// and awards the round to the player with the lowest hand pip-sum.
pub fn pass_turn(state: &GameState, player_id: &str) -> Result<GameState, DominoError> {
    if state.status != GameStatus::Playing {
        return Err(DominoError::NotPlaying);
    }
    if state.players.get(state.turn_index).map(String::as_str) != Some(player_id) {
        return Err(DominoError::NotYourTurn);
    }

    if !legal_moves(state, player_id).is_empty() {
        return Err(DominoError::PlayableTileAvailable);
    }
    if draw_count_for(state.players.len()) > 0 && !state.market.is_empty() {
        return Err(DominoError::MustDrawBeforePassing);
    }

    let new_consecutive_passes = state.consecutive_passes + 1;
    let mut new_status = state.status;
    let mut last_winner = state.last_winner.clone();
    let mut new_scores = state.scores.clone();

    if new_consecutive_passes >= state.players.len() as u32 {
        new_status = GameStatus::RoundEnd;

        // Lowest hand sum wins; ties go to the first player in turn order.
        let mut min_sum = i32::MAX;
        let mut winner = state.players[0].clone();
        let mut turn_sums = std::collections::HashMap::<String, i32>::new();
        for p in &state.players {
            let hand = &state.hands[p];
            let sum = if hand.len() == 1 && hand[0] == [0, 0] {
                10
            } else {
                pip_sum(hand)
            };
            turn_sums.insert(p.clone(), sum);
            if sum < min_sum {
                min_sum = sum;
                winner = p.clone();
            }
        }

        last_winner = Some(winner.clone());
        let round_points: i32 = state
            .players
            .iter()
            .filter(|p| **p != winner)
            .map(|p| turn_sums[p])
            .sum();

        new_scores = apply_101_scoring(new_scores, &winner, round_points, &state.players);
    }

    Ok(GameState {
        turn_index: if new_status == GameStatus::RoundEnd {
            state.turn_index
        } else {
            (state.turn_index + 1) % state.players.len()
        },
        consecutive_passes: new_consecutive_passes,
        status: new_status,
        last_winner,
        scores: new_scores,
        ..state.clone()
    })
}

/// Match over once anyone's permanent score reaches 101 (`checkGameOver`).
#[must_use]
pub fn check_game_over(state: &GameState) -> GameState {
    if state
        .players
        .iter()
        .any(|p| state.scores[p].permanent >= 101)
    {
        GameState {
            status: GameStatus::Finished,
            ..state.clone()
        }
    } else {
        state.clone()
    }
}

/// End the round and deal the next one (`nextRound`).
pub fn next_round_with(state: &GameState, rng: &mut GameRng) -> Result<GameState, DominoError> {
    if state.status != GameStatus::RoundEnd {
        return Err(DominoError::RoundNotEnded);
    }
    let checked = check_game_over(state);
    if checked.status == GameStatus::Finished {
        return Ok(checked);
    }
    let mut next = checked;
    next.round_number += 1;
    Ok(start_round_with(&next, rng))
}

/// Entropy-seeded alias of [`next_round_with`].
pub fn next_round(state: &GameState) -> Result<GameState, DominoError> {
    next_round_with(state, &mut GameRng::from_entropy())
}
