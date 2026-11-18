use domino_core::*;

fn position() -> GameState {
    let mut s = create_game_with(&["A", "B"], GameMode::Telefon, &mut GameRng::from_seed(1));
    s.round_number = 2;
    s.turn_index = 0;
    s.market.clear();
    s.telefon = vec![
        TelefonNode {
            tile: [2, 3],
            parent: None,
            port: 0,
        },
        TelefonNode {
            tile: [2, 2],
            parent: Some(0),
            port: 0,
        },
        TelefonNode {
            tile: [2, 4],
            parent: Some(1),
            port: 1,
        },
        TelefonNode {
            tile: [3, 3],
            parent: Some(0),
            port: 1,
        },
        TelefonNode {
            tile: [3, 0],
            parent: Some(3),
            port: 1,
        },
    ];
    s.board = s.telefon.iter().map(|n| n.tile).collect();
    s.hands
        .insert("A".into(), vec![[2, 6], [3, 5], [2, 5], [1, 1]]);
    s.hands.insert("B".into(), vec![[4, 6], [0, 0]]);
    s
}

#[test]
fn every_first_deal_has_two_three_in_the_starting_hand() {
    for n in 2..=4 {
        for seed in 0..100 {
            let players: Vec<_> = ["A", "B", "C", "D"][..n].to_vec();
            let s = create_game_with(&players, GameMode::Telefon, &mut GameRng::from_seed(seed));
            let p = &s.players[s.turn_index];
            assert_eq!(legal_moves(&s, p), vec![([2, 3], End::Left)]);
            let next = play_tile(&s, p, [3, 2], End::Left).unwrap();
            assert_eq!(next.scores[p].permanent, 5);
            assert_eq!(telefon::end_total(&next.telefon), 5);
        }
    }
}

#[test]
fn two_different_middle_doubles_and_both_sides_can_score() {
    let s = position();
    assert_eq!(telefon::end_total(&s.telefon), 4);
    let mut a = play_tile(&s, "A", [6, 2], End::Telefon(1, 2)).unwrap();
    assert_eq!(a.scores["A"].permanent, 10);
    assert_eq!(a.telefon[5].tile, [2, 6]);
    a.turn_index = 0;
    let mut b = play_tile(&a, "A", [3, 5], End::Telefon(3, 2)).unwrap();
    assert_eq!(b.scores["A"].permanent, 25);
    assert_eq!(telefon::end_total(&b.telefon), 15);
    b.turn_index = 0;
    let c = play_tile(&b, "A", [2, 5], End::Telefon(1, 3)).unwrap();
    assert_eq!(c.scores["A"].permanent, 45);
    assert_eq!(telefon::end_total(&c.telefon), 20);
}

#[test]
fn non_scoring_side_moves_occupied_ports_and_non_double_branches_are_rejected() {
    let mut s = position();
    s.hands.get_mut("A").unwrap().push([2, 0]);
    for end in [
        End::Telefon(1, 2),
        End::Telefon(0, 2),
        End::Telefon(1, 0),
        End::Telefon(255, 3),
        End::Right,
    ] {
        assert!(play_tile(&s, "A", [2, 0], end).is_err());
    }
    assert!(!legal_moves(&s, "A").contains(&([2, 0], End::Telefon(1, 2))));
    assert_eq!(telefon::end_total(&s.telefon), 4);
}

#[test]
fn new_branch_can_continue_normally_without_scoring() {
    let s = position();
    let a = play_tile(&s, "A", [2, 6], End::Telefon(1, 2)).unwrap();
    let b = play_tile(&a, "B", [4, 6], End::Telefon(5, 1)).unwrap();
    assert_eq!(telefon::end_total(&b.telefon), 8);
    assert_eq!(b.scores["B"].permanent, 0);
}

#[test]
fn a_side_move_prevents_passing_and_blocked_rounds_keep_existing_points() {
    let mut s = position();
    assert_eq!(pass_turn(&s, "A"), Err(DominoError::PlayableTileAvailable));
    s.hands.insert("A".into(), vec![[1, 1]]);
    s.hands.insert("B".into(), vec![[1, 6]]);
    s.scores.get_mut("B").unwrap().permanent = 100;
    let a = pass_turn(&s, "A").unwrap();
    let b = pass_turn(&a, "B").unwrap();
    assert_eq!(b.status, GameStatus::RoundEnd);
    assert_eq!(b.last_winner.as_deref(), Some("A"));
    assert_eq!(b.scores["A"].permanent, 5);
    assert_eq!(b.scores["B"].permanent, 100);
}

#[test]
fn exposed_doubles_count_both_halves_and_unused_sides_count_zero() {
    let mut s = position();
    s.telefon.push(TelefonNode {
        tile: [4, 4],
        parent: Some(2),
        port: 1,
    });
    assert_eq!(telefon::end_total(&s.telefon), 8);
    assert!(!telefon::targets(&s.telefon)
        .iter()
        .any(|(e, _, _)| *e == End::Telefon(5, 2)));
}

#[test]
fn reaching_365_ends_immediately_even_with_tiles_left() {
    let mut s = position();
    s.scores.get_mut("A").unwrap().permanent = 355;
    let won = play_tile(&s, "A", [2, 6], End::Telefon(1, 2)).unwrap();
    assert_eq!(won.status, GameStatus::Finished);
    assert_eq!(won.last_winner.as_deref(), Some("A"));
    assert!(!won.hands["A"].is_empty());
    assert_eq!(won.scores["A"].permanent, 365);
    assert!(draw_from_market(&won, "A").is_err());
    assert!(next_round(&won).is_err());
}

#[test]
fn branch_topology_survives_save_and_next_round_clears_it() {
    let s = position();
    let mut a = play_tile(&s, "A", [2, 6], End::Telefon(1, 2)).unwrap();
    let saved: GameState = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
    assert_eq!(saved, a);
    assert_eq!(legal_moves(&saved, "B"), legal_moves(&a, "B"));
    a.status = GameStatus::RoundEnd;
    a.last_winner = Some("B".into());
    let next = next_round_with(&a, &mut GameRng::from_seed(3)).unwrap();
    assert!(next.telefon.is_empty());
    assert!(next.board.is_empty());
    assert_eq!(next.scores, a.scores);
    assert_eq!(next.players[next.turn_index], "B");
}

#[test]
fn round_bonus_rounds_to_nearest_five_without_pending_scores() {
    assert_eq!(
        (0..=12).map(telefon::round_bonus).collect::<Vec<_>>(),
        vec![0, 0, 0, 5, 5, 5, 5, 5, 10, 10, 10, 10, 10]
    );
    let mut s = position();
    s.hands.insert("A".into(), vec![[2, 6]]);
    let n = play_tile(&s, "A", [2, 6], End::Telefon(1, 2)).unwrap();
    // 10 from ends, 10 from 4–6 plus 0–0 in a multi-tile hand.
    assert_eq!(n.status, GameStatus::RoundEnd);
    assert_eq!(n.scores["A"].permanent, 20);
    assert_eq!(n.scores["A"].temporary, 0);
}

#[test]
fn ai_is_fair_and_complete_bot_matches_preserve_every_tile() {
    for count in 2..=4 {
        for seed in 0..20 {
            let ids = &["A", "B", "C", "D"][..count];
            let mut rng = GameRng::from_seed(seed);
            let mut s = create_game_with(ids, GameMode::Telefon, &mut rng);
            for step in 0..12000 {
                let p = s.players[s.turn_index].clone();
                if s.status == GameStatus::Finished {
                    break;
                }
                if s.status == GameStatus::RoundEnd {
                    s = next_round_with(&s, &mut rng).unwrap();
                    continue;
                }
                let action = get_ai_move(&s, &p);
                let mut hidden = s.clone();
                hidden.market.reverse();
                for (id, h) in &mut hidden.hands {
                    if id != &p {
                        h.reverse();
                    }
                }
                assert_eq!(action, get_ai_move(&hidden, &p));
                s = match action {
                    AIAction::Play { tile, end } => play_tile(&s, &p, tile, end),
                    AIAction::Draw => draw_from_market(&s, &p),
                    AIAction::Pass => pass_turn(&s, &p),
                }
                .unwrap();
                let tiles: Vec<_> = s
                    .board
                    .iter()
                    .chain(s.market.iter())
                    .chain(s.hands.values().flatten())
                    .map(|t| [t[0].min(t[1]), t[0].max(t[1])])
                    .collect();
                assert_eq!(tiles.len(), 28);
                let unique: std::collections::BTreeSet<_> = tiles.into_iter().collect();
                assert_eq!(unique.len(), 28);
                assert_eq!(s.board.len(), s.telefon.len());
                assert!(s
                    .scores
                    .values()
                    .all(|v| v.permanent % 5 == 0 && v.temporary == 0));
                assert!(s
                    .telefon
                    .iter()
                    .enumerate()
                    .all(|(i, n)| n.parent.is_none_or(|p| p < i)));
                assert!(step < 11999, "Telefon bots did not finish");
            }
            assert_eq!(s.status, GameStatus::Finished);
            let winner = s.last_winner.as_ref().unwrap();
            assert!(s.scores[winner].permanent >= 365);
        }
    }
}
