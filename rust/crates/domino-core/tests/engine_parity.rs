//! Golden parity tests: the Rust engine must behave identically to
//! `src/game/engine.ts` and serialize with the same JSON shape.

mod common;
use common::base_state;
use domino_core::{
    can_play_tile, check_game_over, create_game_with, draw_count_for, draw_from_market,
    generate_tiles, get_ai_move, next_round_with, pass_turn, play_tile, AIAction, DominoError, End,
    GameMode, GameRng, GameState, GameStatus, PlayerScore, Tile,
};

#[test]
fn generate_the_full_double_six_set() {
    let tiles = generate_tiles();
    assert_eq!(tiles.len(), 28);
    let mut sorted = tiles.clone();
    sorted.sort();
    let mut expected: Vec<Tile> = Vec::new();
    for i in 0u8..=6 {
        for j in i..=6 {
            expected.push([i, j]);
        }
    }
    assert_eq!(sorted, expected, "same multiset of tiles, just shuffled");
}

#[test]
fn serde_shape_matches_ts_json() {
    let s = create_game_with(&["A", "B"], GameMode::OneOhOne, &mut GameRng::from_seed(7));
    let json = serde_json::to_string(&s).unwrap();
    for key in [
        "mode",
        "players",
        "hands",
        "market",
        "board",
        "openEnds",
        "scores",
        "turnIndex",
        "status",
        "roundNumber",
        "lastWinner",
        "consecutivePasses",
    ] {
        assert!(
            json.contains(&format!("\"{key}\"")),
            "missing key {key} in {json}"
        );
    }
    assert!(
        json.contains("\"lastWinner\":null"),
        "lastWinner serializes as null: {json}"
    );
    assert!(
        json.contains("\"status\":\"playing\""),
        "lowcase statuses: {json}"
    );

    // Round-trips must reproduce the state exactly.
    let parsed: GameState = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, s);
}

#[test]
fn old_server_state_json_still_parses() {
    // State written by the current Node.js server (from database.sqlite).
    const TS_STATE: &str = r#"{"mode":"101","players":["Human","ai_1"],"hands":{"Human":[[6,6],[5,2]],"ai_1":[[3,1]]},"market":[[4,4],[2,2]],"board":[[1,1]],"openEnds":[1,1],"scores":{"Human":{"temporary":0,"permanent":5},"ai_1":{"temporary":0,"permanent":0}},"turnIndex":1,"status":"playing","roundNumber":2,"lastWinner":"Human","consecutivePasses":0}"#;
    let state: GameState = serde_json::from_str(TS_STATE).unwrap();
    assert_eq!(state.status, GameStatus::Playing);
    assert_eq!(state.open_ends, [Some(1), Some(1)]);
    assert_eq!(
        state.scores["Human"],
        PlayerScore {
            temporary: 0,
            permanent: 5
        }
    );
    assert_eq!(state.turn_index, 1);
    assert!(
        can_play_tile([3, 1], state.open_ends),
        "pip 3 touches end 1"
    );
    assert!(
        !can_play_tile([3, 2], [Some(5), None]),
        "no pip matches end 5"
    );
    assert!(
        can_play_tile([3, 2], [None, None]),
        "open board takes anything"
    );
}

#[test]
fn first_play_of_round1_requires_lowest_double() {
    let mut s = base_state(&["A", "B"]);
    s.hands.insert("A".into(), vec![[3, 3], [2, 5]]);
    s.hands.insert("B".into(), vec![[0, 0], [6, 4], [5, 5]]);
    s.market = vec![[1, 2], [3, 4]];

    // Hand doubles use the priority [1,1]…[6,6],[0,0]; (3,3) outranks (5,5),
    // and B does not have the turn anyway.
    assert_eq!(
        play_tile(&s, "A", [2, 5], End::Left).unwrap_err(),
        DominoError::MustStartWithLowestDouble(3, 3)
    );

    // The required double plays fine even when a non-matching `end` is passed:
    // an empty board takes precedence over the end choice.
    let s = play_tile(&s, "A", [3, 3], End::Right).unwrap();
    assert_eq!(s.board, vec![[3, 3]]);
    assert_eq!(s.open_ends, [Some(3), Some(3)]);

    // A player without any double must open with one.
    let mut nb = base_state(&["A", "B"]);
    nb.hands.insert("A".into(), vec![[2, 5], [4, 1]]);
    assert_eq!(
        play_tile(&nb, "A", [2, 5], End::Left).unwrap_err(),
        DominoError::MustStartWithADouble
    );
    nb.hands.insert("A".into(), vec![[2, 5], [4, 4]]);
    let s = play_tile(&nb, "A", [4, 4], End::Left).unwrap();
    assert_eq!(s.open_ends, [Some(4), Some(4)]);
}

#[test]
fn two_player_domino_round_end_to_end() {
    let mut s = base_state(&["A", "B"]);
    s.hands
        .insert("A".into(), vec![[3, 3], [6, 2], [4, 2], [1, 5]]);
    s.hands
        .insert("B".into(), vec![[3, 6], [2, 4], [2, 1], [0, 0]]);
    s.market = vec![[5, 2], [5, 3]];

    s = play_tile(&s, "A", [3, 3], End::Left).unwrap();
    s = play_tile(&s, "B", [3, 6], End::Right).unwrap();
    s = play_tile(&s, "A", [6, 2], End::Right).unwrap();
    s = play_tile(&s, "B", [2, 4], End::Right).unwrap();
    s = play_tile(&s, "A", [4, 2], End::Right).unwrap();
    s = play_tile(&s, "B", [2, 1], End::Right).unwrap();
    s = play_tile(&s, "A", [1, 5], End::Right).unwrap(); // "Domino!"

    assert_eq!(s.status, GameStatus::RoundEnd);
    assert_eq!(s.last_winner.as_deref(), Some("A"));
    // B is left with the double blank → exactly 10 points for A.
    assert_eq!(
        s.scores["A"],
        PlayerScore {
            temporary: 10,
            permanent: 0
        }
    );
    assert_eq!(
        s.scores["B"],
        PlayerScore {
            temporary: 0,
            permanent: 0
        }
    );
    assert_eq!(s.turn_index, 0, "round end keeps the turn index");
    assert_eq!(s.consecutive_passes, 0);
    assert_eq!(
        s.board,
        vec![[3, 3], [3, 6], [6, 2], [2, 4], [4, 2], [2, 1], [1, 5]]
    );
    assert_eq!(s.open_ends, [Some(3), Some(5)]);
    assert!(s.hands["A"].is_empty());
    assert_eq!(s.hands["B"], vec![[0, 0]]);
}

#[test]
fn blocked_game_picks_lowest_sum_with_blank_rule() {
    let mut s = base_state(&["A", "B"]);
    s.hands.insert("A".into(), vec![[0, 0]]);
    s.hands.insert("B".into(), vec![[5, 5], [6, 6]]);
    s.board = vec![[3, 3]];
    s.open_ends = [Some(3), Some(3)];

    s = pass_turn(&s, "A").unwrap();
    assert_eq!(s.status, GameStatus::Playing);
    assert_eq!(s.consecutive_passes, 1);
    assert_eq!(s.turn_index, 1);

    s = pass_turn(&s, "B").unwrap();
    assert_eq!(s.status, GameStatus::RoundEnd);
    assert_eq!(s.consecutive_passes, 2);
    assert_eq!(s.last_winner.as_deref(), Some("A"));
    // A's (0,0) counts 10; B's 22 pip hands give A 22 round points → promoted.
    assert_eq!(
        s.scores["A"],
        PlayerScore {
            temporary: 0,
            permanent: 22
        }
    );
    assert_eq!(s.turn_index, 1, "round end keeps the turn index");
}

#[test]
fn scoring_promotes_temporary_over_12_and_resets_losers() {
    let mut s = base_state(&["A", "B"]);
    s.scores.insert(
        "A".into(),
        PlayerScore {
            temporary: 10,
            permanent: 0,
        },
    );
    s.scores.insert(
        "B".into(),
        PlayerScore {
            temporary: 7,
            permanent: 0,
        },
    );
    s.hands.insert("A".into(), vec![[0, 0]]);
    s.hands.insert("B".into(), vec![[5, 5], [6, 6]]);
    s.board = vec![[3, 3]];
    s.open_ends = [Some(3), Some(3)];

    s = pass_turn(&s, "A").unwrap();
    s = pass_turn(&s, "B").unwrap();
    assert_eq!(s.last_winner.as_deref(), Some("A"));
    // 10 (carried temporary) + 22 (B's hand) = 32 > 12 → promoted to permanent.
    assert_eq!(
        s.scores["A"],
        PlayerScore {
            temporary: 0,
            permanent: 32
        }
    );
    // Losers' temporary scores are wiped.
    assert_eq!(
        s.scores["B"],
        PlayerScore {
            temporary: 0,
            permanent: 0
        }
    );
}

#[test]
fn market_draw_rules() {
    // Two-player draws two tiles.
    let mut s = base_state(&["A", "B"]);
    s.market = vec![[1, 2], [3, 4], [5, 6]];
    s = draw_from_market(&s, "A").unwrap();
    assert_eq!(s.hands["A"], vec![[1, 2], [3, 4]]);
    assert_eq!(s.market, vec![[5, 6]]);
    assert_eq!(s.turn_index, 0, "drawing does not advance the turn");

    // Three-player draws one.
    let mut s3 = base_state(&["A", "B", "C"]);
    s3.market = vec![[5, 5], [6, 6]];
    s3 = draw_from_market(&s3, "A").unwrap();
    assert_eq!(s3.hands["A"], vec![[5, 5]]);
    assert_eq!(s3.market, vec![[6, 6]]);

    // Four-player: draw count zero; "market empty" is checked first (TS order).
    let mut s4 = base_state(&["A", "B", "C", "D"]);
    s4.market = vec![[2, 3]];
    assert_eq!(
        draw_from_market(&s4, "A").unwrap_err(),
        DominoError::NoMarketDrawsAllowed
    );
    let s4e = base_state(&["A", "B", "C", "D"]);
    assert_eq!(
        draw_from_market(&s4e, "A").unwrap_err(),
        DominoError::MarketEmpty
    );

    // Guards: not your turn / not playing.
    let mut sb = base_state(&["A", "B"]);
    sb.market = vec![[1, 2]];
    assert_eq!(
        draw_from_market(&sb, "B").unwrap_err(),
        DominoError::NotYourTurn
    );
    sb.status = GameStatus::Finished;
    assert_eq!(
        draw_from_market(&sb, "A").unwrap_err(),
        DominoError::NotPlaying
    );
}

#[test]
fn play_turn_guards_and_end_validation() {
    let mut s = base_state(&["A", "B"]);
    s.hands.insert("A".into(), vec![[3, 3], [1, 2]]);
    s.hands.insert("B".into(), vec![[0, 0]]);

    // Tile ownership checked before anything about the board.
    assert_eq!(
        play_tile(&s, "A", [6, 6], End::Left).unwrap_err(),
        DominoError::TileNotInHand
    );
    assert_eq!(
        play_tile(&s, "B", [3, 3], End::Left).unwrap_err(),
        DominoError::NotYourTurn
    );
    s.status = GameStatus::RoundEnd;
    assert_eq!(
        play_tile(&s, "A", [3, 3], End::Left).unwrap_err(),
        DominoError::NotPlaying
    );
    s.status = GameStatus::Playing;

    // Opening with the double leaves ends [3,3]; A keeps (1,2) so play continues.
    let mut s = play_tile(&s, "A", [3, 3], End::Left).unwrap();
    assert_eq!(s.status, GameStatus::Playing);
    s.turn_index = 1;
    assert_eq!(
        play_tile(&s, "B", [0, 0], End::Left).unwrap_err(),
        DominoError::InvalidMoveOnLeftEnd
    );

    // Orientation with reversed hand storage: hand holds [5,3]; a `left` play of
    // tile [3,5] matches via its second pip → replaces the left end with 5.
    let mut se = s;
    se.hands.insert("B".into(), vec![[5, 3]]);
    let se = play_tile(&se, "B", [3, 5], End::Left).unwrap();
    assert_eq!(
        se.board,
        vec![[5, 3], [3, 3]],
        "left placement prepends the oriented tile"
    );
    assert_eq!(se.open_ends, [Some(5), Some(3)]);
}

#[test]
fn next_round_and_match_finish() {
    let mut s = base_state(&["A", "B"]);
    s.board = vec![[3, 3]];
    s.open_ends = [Some(3), Some(3)];
    s.hands.insert("A".into(), vec![[4, 4]]);
    s.hands.insert("B".into(), vec![[2, 2]]);

    // Neither can play against [3|3]… but we pass directly (pass is always legal).
    s = pass_turn(&s, "A").unwrap();
    s = pass_turn(&s, "B").unwrap();
    assert_eq!(s.status, GameStatus::RoundEnd);
    // B's (2,2)=4 < A's (4,4)=8 → B wins; A's 8 pips go to B's temporary score.
    assert_eq!(s.last_winner.as_deref(), Some("B"));
    assert_eq!(
        s.scores["B"],
        PlayerScore {
            temporary: 8,
            permanent: 0
        }
    );

    let s = next_round_with(&s, &mut GameRng::from_seed(3)).unwrap();
    assert_eq!(s.status, GameStatus::Playing);
    assert_eq!(s.round_number, 2);
    assert_eq!(s.turn_index, 1, "previous winner opens the next round");
    for hand in s.hands.values() {
        assert_eq!(hand.len(), 7, "7 tiles dealt to each player");
    }
    assert_eq!(
        s.hands.values().map(Vec::len).sum::<usize>() + s.market.len(),
        28
    );
    assert_eq!(s.board, Vec::<Tile>::new());
    assert_eq!(s.open_ends, [None, None]);

    // next_round on a playing state is an error (TS throws).
    let ready = s;
    assert!(matches!(
        next_round_with(&ready, &mut GameRng::from_seed(1)).unwrap_err(),
        DominoError::RoundNotEnded
    ));

    // check_game_over only fires at ≥101 permanent.
    let mut fin = base_state(&["A", "B"]);
    fin.scores.insert(
        "A".into(),
        PlayerScore {
            temporary: 0,
            permanent: 101,
        },
    );
    assert_eq!(check_game_over(&fin).status, GameStatus::Finished);
    let mut near = base_state(&["A", "B"]);
    near.scores.insert(
        "A".into(),
        PlayerScore {
            temporary: 50,
            permanent: 100,
        },
    );
    assert_eq!(check_game_over(&near).status, GameStatus::Playing);
}

#[test]
fn deal_invariants_hold_across_seeds_and_player_counts() {
    for seed in 0..50u64 {
        for (players, market_len) in [
            (&["A", "B"][..], 14),
            (&["A", "B", "C"][..], 7),
            (&["A", "B", "C", "D"][..], 0),
        ] {
            let s = create_game_with(players, GameMode::OneOhOne, &mut GameRng::from_seed(seed));
            assert_eq!(s.status, GameStatus::Playing);
            assert_eq!(s.round_number, 1);
            for hand in s.hands.values() {
                assert_eq!(hand.len(), 7);
            }
            assert_eq!(
                s.market.len(),
                market_len,
                "market sized by player count (seed {seed})"
            );
            // Round 1 guarantees the opener holds at least one double.
            let opener_hand = &s.hands[&s.players[s.turn_index]];
            assert!(
                opener_hand.iter().any(|t| t[0] == t[1]),
                "opener must hold a double (seed {seed}, {} players)",
                players.len()
            );
        }
    }
}

#[test]
fn draw_count_helper_matches_ts() {
    assert_eq!(draw_count_for(2), 2);
    assert_eq!(draw_count_for(3), 1);
    assert_eq!(draw_count_for(4), 0);
    assert_eq!(draw_count_for(7), 0);
}

#[test]
fn ai_moves_obey_opening_and_attachment_rules() {
    // Round 1: priority double opening.
    let mut s = base_state(&["A", "b"]);
    s.hands.insert("b".into(), vec![[2, 4], [3, 3]]);
    assert_eq!(
        get_ai_move(&s, "b"),
        AIAction::Play {
            tile: [3, 3],
            end: End::Left
        }
    );

    // Highest pip sum wins; legal ends decide. Ends [6|1]: (6,2)→left 8; (1,4)→right 5.
    let mut s = base_state(&["A", "B"]);
    s.round_number = 2;
    s.board = vec![[3, 3]];
    s.open_ends = [Some(6), Some(1)];
    s.hands.insert("B".into(), vec![[6, 2], [1, 4]]);
    assert_eq!(
        get_ai_move(&s, "B"),
        AIAction::Play {
            tile: [6, 2],
            end: End::Left
        }
    );

    // Tie score: left pushed first, stable order keeps left.
    let mut s = base_state(&["A", "B"]);
    s.round_number = 2;
    s.board = vec![[3, 3]];
    s.open_ends = [Some(2), Some(4)];
    s.hands.insert("B".into(), vec![[2, 4]]);
    assert_eq!(
        get_ai_move(&s, "B"),
        AIAction::Play {
            tile: [2, 4],
            end: End::Left
        }
    );

    // Stuck: draw when market allows, else pass.
    let mut s = base_state(&["A", "B"]);
    s.round_number = 2;
    s.board = vec![[3, 3]];
    s.open_ends = [Some(3), Some(3)];
    s.hands.insert("B".into(), vec![[0, 0]]);
    s.market = vec![[4, 4], [5, 5]];
    assert_eq!(get_ai_move(&s, "B"), AIAction::Draw);
    s.market.clear();
    assert_eq!(get_ai_move(&s, "B"), AIAction::Pass);
}

#[test]
fn ai_round1_opening_never_proposes_an_illegal_non_double() {
    // A hand holding a double always opens with the priority double (TS returns
    // it before the greedy sort ever runs).
    let mut s = base_state(&["A", "B"]);
    s.hands.insert("B".into(), vec![[2, 5], [3, 3]]);
    assert_eq!(
        get_ai_move(&s, "B"),
        AIAction::Play {
            tile: [3, 3],
            end: End::Left
        }
    );

    // A malformed/legacy opener without a double must not propose an illegal
    // non-double. It draws when possible, otherwise passes.
    let mut s = base_state(&["A", "B"]);
    s.hands.insert("B".into(), vec![[2, 5], [4, 1]]);
    assert_eq!(get_ai_move(&s, "B"), AIAction::Pass);
    s.turn_index = 1;
    assert!(matches!(
        play_tile(&s, "B", [2, 5], End::Left),
        Err(DominoError::MustStartWithADouble)
    ));
}

#[test]
fn ai_action_wire_format_matches_ts() {
    let a: AIAction = serde_json::from_str(r#"{"type":"play","tile":[3,3],"end":"left"}"#).unwrap();
    assert_eq!(
        a,
        AIAction::Play {
            tile: [3, 3],
            end: End::Left
        }
    );
    assert_eq!(
        serde_json::to_string(&AIAction::Draw).unwrap(),
        r#"{"type":"draw"}"#
    );
    assert_eq!(
        serde_json::to_string(&AIAction::Pass).unwrap(),
        r#"{"type":"pass"}"#
    );
}

#[test]
fn error_messages_match_ts_strings() {
    let s = base_state(&["A", "B"]);
    assert_eq!(
        play_tile(&s, "B", [1, 1], End::Left)
            .unwrap_err()
            .to_string(),
        "Not your turn"
    );
    let mut fin = s.clone();
    fin.status = GameStatus::Finished;
    assert_eq!(
        pass_turn(&fin, "A").unwrap_err().to_string(),
        "Game is not in playing state"
    );
    let mut blocked = s;
    blocked.board = vec![[3, 3]];
    blocked.open_ends = [Some(3), Some(3)];
    blocked.hands.insert("A".into(), vec![[0, 0]]);
    assert_eq!(
        play_tile(&blocked, "A", [0, 0], End::Left)
            .unwrap_err()
            .to_string(),
        "Invalid move on left end"
    );
}

#[test]
fn btreemap_keys_stay_stable_like_json_objects() {
    // Documented difference: hands/scores iterate in sorted key order (BTreeMap)
    // instead of TS insertion order; membership and semantics are unchanged.
    let s = base_state(&["C", "A", "B"]);
    let keys: Vec<String> = s.hands.keys().cloned().collect();
    assert_eq!(keys, vec!["A", "B", "C"]);
    let _ = s;
}
