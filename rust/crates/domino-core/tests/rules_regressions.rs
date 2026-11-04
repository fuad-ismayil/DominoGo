mod common;
use common::{assert_invariants, base_state};
use domino_core::*;

#[test]
fn cannot_manufacture_a_blocked_round_by_passing() {
    let mut s = base_state(&["A", "B"]);
    s.round_number = 2;
    s.board = vec![[3, 3]];
    s.open_ends = [Some(3), Some(3)];
    s.hands.insert("A".into(), vec![[3, 6]]);
    s.market = vec![[1, 2]];
    assert_eq!(pass_turn(&s, "A"), Err(DominoError::PlayableTileAvailable));
    s.hands.insert("A".into(), vec![[5, 6]]);
    assert_eq!(pass_turn(&s, "A"), Err(DominoError::MustDrawBeforePassing));
    let drawn = draw_from_market(&s, "A").unwrap();
    assert!(pass_turn(&drawn, "A").is_ok());
}

#[test]
fn voluntary_draw_preserves_the_turn_board_and_playable_tiles() {
    for players in [&["A", "B"][..], &["A", "B", "C"][..]] {
        let mut state = base_state(players);
        state.round_number = 2;
        state.board = vec![[3, 3]];
        state.open_ends = [Some(3), Some(3)];
        state.hands.insert("A".into(), vec![[3, 6]]);
        state.market = vec![[1, 2], [4, 5], [0, 0]];
        let count = draw_count_for(players.len());
        let drawn = draw_from_market(&state, "A").unwrap();
        assert_eq!(drawn.hands["A"].len(), 1 + count);
        assert_eq!(drawn.market, state.market[count..]);
        assert_eq!(drawn.board, state.board);
        assert_eq!(drawn.turn_index, state.turn_index);
        assert_eq!(drawn.scores, state.scores);
        assert_eq!(
            pass_turn(&drawn, "A"),
            Err(DominoError::PlayableTileAvailable)
        );
        assert!(play_tile(&drawn, "A", [3, 6], End::Right).is_ok());
        let mut exhausted = drawn;
        while !exhausted.market.is_empty() {
            exhausted = draw_from_market(&exhausted, "A").unwrap();
        }
        assert_eq!(exhausted.hands["A"].len(), 4);
        assert_eq!(
            draw_from_market(&exhausted, "A"),
            Err(DominoError::MarketEmpty)
        );
    }
}

#[test]
fn invalid_lobbies_are_rejected_instead_of_hanging_or_overdealing() {
    for players in [
        &[][..],
        &["A"][..],
        &["A", "A"][..],
        &["A", " "][..],
        &["A", "B", "C", "D", "E"][..],
    ] {
        assert_eq!(
            try_create_game_with(players, GameMode::OneOhOne, &mut GameRng::from_seed(1)),
            Err(DominoError::InvalidPlayers)
        );
    }
}

#[test]
fn bot_preserves_a_follow_up_instead_of_stranding_itself_for_extra_pips() {
    let mut s = base_state(&["A", "B"]);
    s.round_number = 2;
    s.board = vec![[6, 6]];
    s.open_ends = [Some(6), Some(6)];
    s.hands.insert("A".into(), vec![[6, 5], [6, 4], [4, 4]]);
    assert_eq!(
        get_ai_move(&s, "A"),
        AIAction::Play {
            tile: [6, 4],
            end: End::Left
        }
    );
    // Hidden opponent information cannot affect the decision.
    let choice = get_ai_move(&s, "A");
    s.hands.insert("B".into(), vec![[5, 5], [5, 4]]);
    s.market = vec![[0, 1]];
    assert_eq!(get_ai_move(&s, "A"), choice);
}

#[test]
fn all_bot_matches_make_legal_progress_and_finish() {
    for count in 2..=4 {
        for seed in 0..96 {
            let players = &["A", "B", "C", "D"][..count];
            let mut rng = GameRng::from_seed(seed);
            let mut s = create_game_with(players, GameMode::OneOhOne, &mut rng);
            let mut finished = false;
            for _ in 0..5000 {
                assert_invariants(&s);
                match s.status {
                    GameStatus::Finished => {
                        finished = true;
                        break;
                    }
                    GameStatus::RoundEnd => {
                        s = next_round_with(&s, &mut rng).unwrap();
                    }
                    GameStatus::Playing => {
                        let player = s.players[s.turn_index].clone();
                        s = match get_ai_move(&s, &player) {
                            AIAction::Play { tile, end } => play_tile(&s, &player, tile, end),
                            AIAction::Draw => draw_from_market(&s, &player),
                            AIAction::Pass => pass_turn(&s, &player),
                        }
                        .expect("the bot must propose a legal action");
                    }
                }
            }
            assert!(
                finished,
                "bot match did not finish: players={count}, seed={seed}"
            );
        }
    }
}

#[test]
fn scores_survive_multiple_rounds_switching_winners_and_pending_promotion() {
    let mut state = base_state(&["A", "B"]);
    let mut rng = GameRng::from_seed(707);
    let rounds = [
        ("A", [2, 3], 5, 0, 0, 0),
        ("B", [1, 6], 0, 0, 7, 0),
        ("B", [2, 6], 0, 0, 0, 15),
        ("A", [1, 5], 6, 0, 0, 15),
        ("B", [4, 6], 0, 0, 0, 25),
    ];
    for (index, (winner, loser_tile, a_pending, a_banked, b_pending, b_banked)) in
        rounds.into_iter().enumerate()
    {
        if index > 0 {
            let scores = state.scores.clone();
            state = next_round_with(&state, &mut rng).unwrap();
            assert_eq!(
                state.scores,
                scores,
                "dealing round {} must preserve every score",
                index + 1
            );
        }
        state.board = vec![[3, 6]];
        state.open_ends = [Some(3), Some(6)];
        let loser = if winner == "A" { "B" } else { "A" };
        state.turn_index = state.players.iter().position(|p| p == winner).unwrap();
        state.hands.insert(winner.into(), vec![[6, 0]]);
        state.hands.insert(loser.into(), vec![loser_tile]);
        state = play_tile(&state, winner, [6, 0], End::Right).unwrap();
        assert_eq!(state.status, GameStatus::RoundEnd);
        assert_eq!(
            state.scores["A"],
            PlayerScore {
                temporary: a_pending,
                permanent: a_banked
            }
        );
        assert_eq!(
            state.scores["B"],
            PlayerScore {
                temporary: b_pending,
                permanent: b_banked
            }
        );
    }
    assert_eq!(state.round_number, 5);
}

#[test]
fn restricted_drawing_requires_a_blocked_hand_and_survives_rounds() {
    let mut state = base_state(&["A", "B"]);
    state.draw_rule = DrawRule::WhenBlocked;
    state.round_number = 2;
    state.board = vec![[3, 3]];
    state.open_ends = [Some(3), Some(3)];
    state.hands.insert("A".into(), vec![[3, 6]]);
    state.market = vec![[1, 2], [4, 5]];
    assert_eq!(
        draw_from_market(&state, "A"),
        Err(DominoError::PlayableTileAvailable)
    );
    state.hands.insert("A".into(), vec![[5, 6]]);
    let drawn = draw_from_market(&state, "A").unwrap();
    assert_eq!(drawn.draw_rule, DrawRule::WhenBlocked);
    assert_eq!(drawn.hands["A"].len(), 3);
    let mut ended = drawn;
    ended.status = GameStatus::RoundEnd;
    ended.last_winner = Some("A".into());
    let next = next_round(&ended).unwrap();
    assert_eq!(next.draw_rule, DrawRule::WhenBlocked);
    let stored = serde_json::to_value(&next).unwrap();
    assert_eq!(stored["drawRule"], "when_blocked");
    let restored: GameState = serde_json::from_value(stored).unwrap();
    assert_eq!(restored.draw_rule, DrawRule::WhenBlocked);
}

#[test]
fn older_saved_matches_keep_voluntary_drawing() {
    let mut stored = serde_json::to_value(base_state(&["A", "B"])).unwrap();
    stored.as_object_mut().unwrap().remove("drawRule");
    let restored: GameState = serde_json::from_value(stored).unwrap();
    assert_eq!(restored.draw_rule, DrawRule::Anytime);
}
