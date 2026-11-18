use super::*;

fn conservation(g: &Game) {
    let mut ids: Vec<_> = g
        .hands
        .iter()
        .flatten()
        .chain(g.draw.iter())
        .chain(g.discard.iter())
        .map(|c| c.id)
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, (0..108).collect::<Vec<u8>>());
}
fn fixture(hands: &[&[(Option<Color>, Value)]]) -> Game {
    let mut g = Game::seeded(hands.len(), 42, 0).unwrap();
    let mut cards = deck();
    g.hands = hands
        .iter()
        .map(|hand| {
            hand.iter()
                .map(|&(color, value)| {
                    let i = cards
                        .iter()
                        .position(|c| c.color == color && c.value == value)
                        .unwrap();
                    cards.remove(i)
                })
                .collect()
        })
        .collect();
    let i = cards
        .iter()
        .position(|c| c.color == Some(Color::Red) && c.value == Value::Number(7))
        .unwrap();
    g.discard = vec![cards.remove(i)];
    g.draw = cards;
    g.color = Color::Red;
    g.turn = 0;
    g.direction = 1;
    g.phase = Phase::Turn;
    g.pending = None;
    g
}
fn play(g: &mut Game, index: usize, uno: bool) {
    let seat = g.turn;
    let c = g.hands[seat][index];
    g.apply(
        seat,
        g.sequences[seat] + 1,
        Action::Play {
            card: c.id,
            color: c.color.is_none().then_some(Color::Blue),
            uno,
        },
        g.last_time + 1,
    )
    .unwrap();
    conservation(g);
}
const R: Option<Color> = Some(Color::Red);
const B: Option<Color> = Some(Color::Blue);

#[test]
fn deck_and_all_openers() {
    let cards = deck();
    assert_eq!(cards.len(), 108);
    for color in Color::ALL {
        assert_eq!(cards.iter().filter(|c| c.color == Some(color)).count(), 25);
    }
    let mut seen = std::collections::HashSet::new();
    for seed in 0..500 {
        let g = Game::seeded(10, seed, 0).unwrap();
        conservation(&g);
        let top = g.discard[0];
        seen.insert(format!("{:?}", top.value));
        let first = (g.dealer + 1) % 10;
        match top.value {
            Value::WildFour => panic!("+4 cannot open"),
            Value::Skip => assert_eq!(g.turn, (first + 1) % 10),
            Value::Reverse => {
                assert_eq!(g.turn, g.dealer);
                assert_eq!(g.direction, -1);
            }
            Value::DrawTwo => {
                assert_eq!(g.hands[first].len(), 9);
                assert_eq!(g.turn, (first + 1) % 10);
            }
            Value::Wild => assert_eq!(g.phase, Phase::OpeningColor),
            _ => assert_eq!(g.turn, first),
        }
    }
    assert!(
        seen.contains("Skip")
            && seen.contains("Reverse")
            && seen.contains("DrawTwo")
            && seen.contains("Wild")
    );
    assert!(Game::seeded(1, 0, 0).is_err());
    assert!(Game::seeded(11, 0, 0).is_err());
}

#[test]
fn two_player_actions_and_final_penalty() {
    for value in [Value::Reverse, Value::Skip, Value::DrawTwo] {
        let mut g = fixture(&[
            &[(R, value), (B, Value::Number(1))],
            &[(B, Value::Number(2))],
        ]);
        play(&mut g, 0, true);
        assert_eq!(g.turn, 0);
        assert_eq!(
            g.hands[1].len(),
            if value == Value::DrawTwo { 3 } else { 1 }
        );
    }
    let mut g = fixture(&[&[(R, Value::DrawTwo)], &[(B, Value::Number(2))]]);
    play(&mut g, 0, false);
    assert_eq!(g.phase, Phase::Finished { winner: Some(0) });
    assert_eq!(g.hands[1].len(), 3);
}

#[test]
fn drawn_card_only_transaction_and_duplicate() {
    let mut g = fixture(&[
        &[(R, Value::Number(3)), (B, Value::Number(1))],
        &[(B, Value::Number(2))],
    ]);
    let i = g.draw.iter().position(|c| c.color == R).unwrap();
    let c = g.draw.remove(i);
    g.draw.push(c);
    g.apply(0, 1, Action::Draw, 1).unwrap();
    assert_eq!(g.phase, Phase::Drawn { card: c.id });
    let before = serde_json::to_string(&g).unwrap();
    assert_eq!(
        g.apply(
            0,
            2,
            Action::Play {
                card: g.hands[0][0].id,
                color: None,
                uno: false
            },
            2
        )
        .unwrap_err(),
        Error::Card
    );
    assert_eq!(serde_json::to_string(&g).unwrap(), before);
    assert_eq!(
        g.apply(0, 1, Action::Draw, 2).unwrap_err(),
        Error::Duplicate
    );
    g.apply(0, 2, Action::Keep, 2).unwrap();
    assert_eq!(g.turn, 1);
    conservation(&g);
}

#[test]
fn challenge_paths_and_private_evidence() {
    for illegal in [false, true] {
        for challenge in [false, true] {
            let other = if illegal { R } else { B };
            let mut g = fixture(&[
                &[(None, Value::WildFour), (other, Value::Number(3))],
                &[(B, Value::Number(2))],
            ]);
            play(&mut g, 0, true);
            assert_eq!(
                g.phase,
                Phase::Challenge {
                    sender: 0,
                    recipient: 1
                }
            );
            let json = serde_json::to_value(g.view(1).unwrap()).unwrap();
            assert!(json.get("pending").is_none());
            assert!(json.get("challenge_evidence").is_none());
            let r = g
                .apply(
                    1,
                    1,
                    if challenge {
                        Action::ChallengeFour
                    } else {
                        Action::AcceptFour
                    },
                    2,
                )
                .unwrap();
            if challenge {
                assert_eq!(r.challenge_succeeded, Some(illegal));
                assert_eq!(r.challenge_evidence.unwrap().len(), 2);
            }
            if challenge && illegal {
                assert_eq!(g.hands[0].len(), 5);
                assert_eq!(g.hands[1].len(), 1);
                assert_eq!(g.turn, 1);
            } else {
                assert_eq!(g.hands[1].len(), if challenge { 7 } else { 5 });
                assert_eq!(g.turn, 0);
            }
            assert_eq!(g.color, Color::Blue);
            conservation(&g);
        }
    }
}

#[test]
fn uno_catch_precedes_challenge_and_preserves_evidence() {
    let mut g = fixture(&[
        &[(None, Value::WildFour), (R, Value::Number(3))],
        &[(B, Value::Number(2))],
    ]);
    assert_eq!(g.apply(1, 1, Action::CatchUno, 0).unwrap_err(), Error::Turn);
    play(&mut g, 0, false);
    assert_eq!(g.phase, Phase::Uno { player: 0 });
    g.apply(1, 1, Action::CatchUno, 2).unwrap();
    assert_eq!(g.hands[0].len(), 3);
    assert_eq!(g.apply(0, 2, Action::CallUno, 3).unwrap_err(), Error::Turn);
    let r = g.apply(1, 2, Action::ChallengeFour, 3).unwrap();
    assert_eq!(r.challenge_evidence.unwrap().len(), 2);
    assert_eq!(g.hands[0].len(), 7);
    conservation(&g);
}

#[test]
fn recovery_timeout_pause_and_serialized_resume() {
    let mut g = fixture(&[
        &[(R, Value::Number(3)), (B, Value::Number(1))],
        &[(B, Value::Number(2))],
    ]);
    play(&mut g, 0, false);
    let deadline = g.deadline;
    g.pause(10).unwrap();
    assert!(!g.tick(100_000).unwrap());
    assert_eq!(
        g.apply(0, 2, Action::CallUno, 100_001).unwrap_err(),
        Error::Paused
    );
    let saved = serde_json::to_string(&g).unwrap();
    let mut g: Game = serde_json::from_str(&saved).unwrap();
    g.resume(100_010).unwrap();
    assert_eq!(g.deadline, 100_010 + deadline - 10);
    g.apply(0, 2, Action::CallUno, 100_011).unwrap();
    assert_eq!(g.turn, 1);
    let deadline = g.deadline;
    g.tick(deadline).unwrap();
    assert_eq!(g.turn, 0);
    conservation(&g);
}

#[test]
fn final_four_waits_for_response() {
    let mut g = fixture(&[&[(None, Value::WildFour)], &[(B, Value::Number(2))]]);
    play(&mut g, 0, false);
    assert!(matches!(g.phase, Phase::Challenge { .. }));
    g.apply(1, 1, Action::ChallengeFour, 2).unwrap();
    assert_eq!(g.phase, Phase::Finished { winner: Some(0) });
    assert_eq!(g.hands[1].len(), 7);
}

#[test]
fn recycling_and_exhaustion() {
    let mut g = fixture(&[&[(R, Value::Number(3))], &[(B, Value::Number(2))]]);
    // Put every buried card into the discard, leaving the current red 7 on top.
    let top = g.discard.pop().unwrap();
    g.discard.append(&mut g.draw);
    g.discard.push(top);
    let color = g.color;
    assert_eq!(g.take(0, 2).len(), 2);
    assert_eq!(g.discard, vec![top]);
    assert_eq!(g.color, color);
    conservation(&g);
    g.hands[0].append(&mut g.draw);
    assert_eq!(g.take(1, 6).len(), 0);
    assert!(g.pile_empty);
    conservation(&g);
    g.apply(0, 1, Action::Draw, 1).unwrap();
    g.apply(1, 1, Action::Draw, 2).unwrap();
    assert_eq!(g.phase, Phase::Finished { winner: None });
    conservation(&g);
}

#[test]
fn full_bot_rounds_every_seat_count() {
    for seats in 2..=10 {
        for seed in 0..60 {
            let mut g = Game::seeded(seats, seed, 0).unwrap();
            for _step in 1..=20_000 {
                conservation(&g);
                if matches!(g.phase, Phase::Finished { .. }) {
                    break;
                }
                let now = match g.phase {
                    Phase::TouchFive { starts_at, .. } => starts_at.max(g.last_time + 1),
                    _ => g.last_time + 1,
                };
                g.tick(now).unwrap();
                let seat = match &g.phase {
                    Phase::Uno { player } => *player,
                    Phase::TouchFive { taps, .. } => {
                        (0..seats).find(|s| !taps.contains(s)).unwrap()
                    }
                    _ => g.turn,
                };
                let view = g.view(seat).unwrap();
                let action = bot_action(&view, seat).expect("bot makes progress");
                g.apply(seat, g.sequences[seat] + 1, action, now).unwrap();
            }
            assert!(
                matches!(g.phase, Phase::Finished { .. }),
                "unfinished seats={seats} seed={seed}"
            );
        }
    }
}

#[test]
fn seat_edges_and_reverse_at_every_table_size() {
    for seats in 3..=10 {
        for direction in [-1, 1] {
            for turn in [0, seats - 1] {
                let mut g = Game::seeded(seats, 19, 0).unwrap();
                // Redistribute a complete real deck into a controlled table.
                let mut cards = deck();
                let reverse = cards.remove(
                    cards
                        .iter()
                        .position(|c| c.color == R && c.value == Value::Reverse)
                        .unwrap(),
                );
                let top = cards.remove(
                    cards
                        .iter()
                        .position(|c| c.color == R && c.value == Value::Number(7))
                        .unwrap(),
                );
                g.hands = (0..seats)
                    .map(|_| vec![cards.pop().unwrap(), cards.pop().unwrap()])
                    .collect();
                g.hands[turn].push(reverse);
                g.discard = vec![top];
                g.draw = cards;
                g.color = Color::Red;
                g.direction = direction;
                g.turn = turn;
                g.phase = Phase::Turn;
                let expected =
                    (turn as isize - direction as isize).rem_euclid(seats as isize) as usize;
                play(&mut g, 2, false);
                assert_eq!(g.turn, expected);
                assert_eq!(g.direction, -direction);
            }
        }
    }
}

#[test]
fn exact_deadline_uno_expiry_and_four_timeout() {
    let mut g = fixture(&[
        &[(None, Value::WildFour), (B, Value::Number(3))],
        &[(B, Value::Number(2))],
    ]);
    play(&mut g, 0, false);
    let due = g.deadline;
    assert_eq!(
        g.apply(1, 1, Action::CatchUno, due).unwrap_err(),
        Error::Deadline
    );
    assert!(!g.tick(due - 1).unwrap());
    assert!(g.tick(due).unwrap());
    assert_eq!(
        g.phase,
        Phase::Challenge {
            sender: 0,
            recipient: 1
        }
    );
    assert_eq!(g.deadline, due + CHALLENGE_MS);
    g.tick(g.deadline).unwrap();
    assert_eq!(g.hands[1].len(), 5);
    assert_eq!(g.turn, 0);
    assert_eq!(
        g.apply(1, 1, Action::CatchUno, g.last_time).unwrap_err(),
        Error::Turn
    );
    conservation(&g);
}

#[test]
fn wild_selection_is_atomic_and_does_not_restart_turn_clock() {
    let mut g = fixture(&[
        &[(None, Value::Wild), (B, Value::Number(3))],
        &[(B, Value::Number(2))],
    ]);
    let before = serde_json::to_string(&g).unwrap();
    assert_eq!(
        g.apply(
            0,
            1,
            Action::Play {
                card: g.hands[0][0].id,
                color: None,
                uno: true
            },
            5
        )
        .unwrap_err(),
        Error::Color
    );
    assert_eq!(serde_json::to_string(&g).unwrap(), before);
    g.phase = Phase::OpeningColor;
    g.apply(0, 1, Action::ChooseColor(Color::Green), 500)
        .unwrap();
    assert_eq!(g.deadline, TURN_MS);
    assert_eq!(g.turn, 0);
    assert_eq!(g.color, Color::Green);
}

#[test]
fn timeout_keeps_drawn_card_without_playing_old_hand() {
    let mut g = fixture(&[&[(R, Value::Number(3))], &[(B, Value::Number(2))]]);
    let index = g.draw.iter().position(|c| c.color == R).unwrap();
    let drawn = g.draw.remove(index);
    g.draw.push(drawn);
    let top = *g.discard.last().unwrap();
    g.tick(TURN_MS).unwrap();
    assert_eq!(g.turn, 1);
    assert_eq!(g.hands[0].len(), 2);
    assert_eq!(*g.discard.last().unwrap(), top);
    conservation(&g);
}

#[test]
fn save_restore_every_phase_in_a_complete_round() {
    let mut g = Game::seeded(4, 18, 0).unwrap();
    for now in 1..=20_000 {
        let encoded = serde_json::to_string(&g).unwrap();
        let restored: Game = serde_json::from_str(&encoded).unwrap();
        for seat in 0..4 {
            assert_eq!(
                serde_json::to_value(g.view(seat).unwrap()).unwrap(),
                serde_json::to_value(restored.view(seat).unwrap()).unwrap()
            );
        }
        g = restored;
        if matches!(g.phase, Phase::Finished { .. }) {
            return;
        }
        let now = now.max(g.last_time + 1);
        let now = match g.phase {
            Phase::TouchFive { starts_at, .. } => starts_at.max(now),
            _ => now,
        };
        g.tick(now).unwrap();
        let seat = match &g.phase {
            Phase::Uno { player } => *player,
            Phase::TouchFive { taps, .. } => {
                (0..g.hands.len()).find(|s| !taps.contains(s)).unwrap()
            }
            _ => g.turn,
        };
        let action = bot_action(&g.view(seat).unwrap(), seat).unwrap();
        g.apply(seat, g.sequences[seat] + 1, action, now).unwrap();
        conservation(&g);
    }
    panic!("saved round did not finish");
}

fn house_top(g: &mut Game, color: Color, value: Value) {
    let index = g
        .draw
        .iter()
        .position(|c| c.color == Some(color) && c.value == value)
        .unwrap();
    let top = g.draw.remove(index);
    g.draw.append(&mut g.discard);
    g.discard.push(top);
    g.color = color;
}
#[test]
fn number_groups_preserve_selected_order_and_are_atomic() {
    for reverse in [false, true] {
        let mut g = fixture(&[
            &[
                (Some(Color::Green), Value::Number(8)),
                (Some(Color::Yellow), Value::Number(8)),
                (R, Value::Number(1)),
            ],
            &[(B, Value::Number(4))],
        ]);
        house_top(&mut g, Color::Red, Value::Number(8));
        let mut cards = vec![g.hands[0][0].id, g.hands[0][1].id];
        if reverse {
            cards.reverse();
        }
        let final_color = g.hands[0]
            .iter()
            .find(|c| c.id == cards[1])
            .unwrap()
            .color
            .unwrap();
        assert!(number_group_playable(&g.view(0).unwrap(), &cards));
        let before = serde_json::to_value(&g).unwrap();
        assert!(g
            .apply(
                0,
                1,
                Action::PlayNumbers {
                    cards: vec![cards[0], cards[0]],
                    uno: true
                },
                1
            )
            .is_err());
        assert_eq!(before, serde_json::to_value(&g).unwrap());
        assert!(g
            .apply(
                0,
                1,
                Action::PlayNumbers {
                    cards: vec![cards[0], g.hands[0][2].id],
                    uno: true
                },
                1
            )
            .is_err());
        assert_eq!(before, serde_json::to_value(&g).unwrap());
        g.apply(
            0,
            1,
            Action::PlayNumbers {
                cards: cards.clone(),
                uno: true,
            },
            1,
        )
        .unwrap();
        assert_eq!(g.color, final_color);
        assert_eq!(
            g.discard[g.discard.len() - 2..]
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            cards
        );
        assert_eq!(g.hands[0].len(), 1);
        assert_eq!(g.turn, 1);
        conservation(&g);
        assert_eq!(
            g.apply(0, 1, Action::Draw, 2).unwrap_err(),
            Error::Duplicate
        );
    }
}
#[test]
fn group_needs_a_match_and_drawn_card_cannot_extend_group() {
    let mut g = fixture(&[
        &[
            (B, Value::Number(8)),
            (B, Value::Number(8)),
            (R, Value::Number(2)),
        ],
        &[(R, Value::Number(4))],
    ]);
    let ids = vec![g.hands[0][0].id, g.hands[0][1].id];
    let before = serde_json::to_value(&g).unwrap();
    assert!(!number_group_playable(&g.view(0).unwrap(), &ids));
    assert!(g
        .apply(
            0,
            1,
            Action::PlayNumbers {
                cards: ids.clone(),
                uno: true
            },
            1
        )
        .is_err());
    assert_eq!(before, serde_json::to_value(&g).unwrap());
    g.phase = Phase::Drawn { card: ids[0] };
    assert!(g
        .apply(
            0,
            1,
            Action::PlayNumbers {
                cards: ids,
                uno: true
            },
            1
        )
        .is_err());
    conservation(&g);
}
#[test]
fn any_card_can_anchor_a_number_group_and_order_still_sets_color() {
    for number in [7,8] {
        let mut g=fixture(&[&[(B,Value::Number(number)),(R,Value::Number(number)),(B,Value::Number(2))],&[(R,Value::Number(4))]]);
        let ids=vec![g.hands[0][0].id,g.hands[0][1].id];
        assert!(number_group_playable(&g.view(0).unwrap(),&ids));
        g.apply(0,1,Action::PlayNumbers{cards:ids.clone(),uno:true},1).unwrap();
        assert_eq!(g.color,Color::Red);
        assert_eq!(g.discard[g.discard.len()-2].id,ids[0]);
        assert_eq!(g.discard.last().unwrap().id,ids[1]);
        conservation(&g);
    }
}
#[test]
fn zero_swap_is_optional_atomic_and_survives_save() {
    for swap in [false, true] {
        let mut g = fixture(&[
            &[
                (R, Value::Number(0)),
                (B, Value::Number(1)),
                (B, Value::Number(2)),
            ],
            &[(B, Value::Number(3))],
            &[(B, Value::Number(4)), (B, Value::Number(6))],
        ]);
        play(&mut g, 0, true);
        assert_eq!(g.phase, Phase::Swap { player: 0 });
        let mut g: Game = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        let original = g.hands.clone();
        assert!(g.apply(1, 1, Action::SwapHands { target: 0 }, 2).is_err());
        assert!(g.apply(0, 2, Action::SwapHands { target: 0 }, 2).is_err());
        assert_eq!(g.hands, original);
        g.apply(
            0,
            2,
            if swap {
                Action::SwapHands { target: 1 }
            } else {
                Action::KeepHand
            },
            2,
        )
        .unwrap();
        assert_eq!(
            g.hands[0],
            if swap {
                original[1].clone()
            } else {
                original[0].clone()
            }
        );
        assert_eq!(
            g.hands[1],
            if swap {
                original[0].clone()
            } else {
                original[1].clone()
            }
        );
        assert_eq!(g.turn, 1);
        conservation(&g);
    }
}
#[test]
fn grouped_zero_swaps_once_timeout_keeps_cards_and_last_zero_wins() {
    let mut g = fixture(&[
        &[
            (R, Value::Number(0)),
            (B, Value::Number(0)),
            (R, Value::Number(1)),
        ],
        &[(B, Value::Number(2))],
    ]);
    let ids = g.hands[0][..2].iter().map(|c| c.id).collect();
    g.apply(
        0,
        1,
        Action::PlayNumbers {
            cards: ids,
            uno: true,
        },
        1,
    )
    .unwrap();
    assert_eq!(g.phase, Phase::Swap { player: 0 });
    let hands = g.hands.clone();
    g.tick(g.deadline).unwrap();
    assert_eq!(g.hands, hands);
    assert_eq!(g.turn, 1);
    conservation(&g);
    let mut g = fixture(&[&[(R, Value::Number(0))], &[(B, Value::Number(2))]]);
    play(&mut g, 0, true);
    assert_eq!(g.phase, Phase::Finished { winner: Some(0) });
}
fn five_game() -> Game {
    let mut g = fixture(&[
        &[
            (R, Value::Number(5)),
            (R, Value::Number(1)),
            (B, Value::Number(2)),
        ],
        &[(B, Value::Number(3))],
        &[(B, Value::Number(4))],
    ]);
    play(&mut g, 0, true);
    g
}
#[test]
fn five_taps_are_ordered_once_and_the_last_draws_one() {
    let mut g = five_game();
    let Phase::TouchFive {
        challenge,
        starts_at,
        ..
    } = g.phase.clone()
    else {
        panic!()
    };
    assert!(g
        .apply(1, 1, Action::TouchFive { challenge }, starts_at - 1)
        .is_err());
    assert!(g
        .apply(
            1,
            1,
            Action::TouchFive {
                challenge: challenge + 1
            },
            starts_at
        )
        .is_err());
    let counts: Vec<_> = g.hands.iter().map(Vec::len).collect();
    g.apply(1, 1, Action::TouchFive { challenge }, starts_at)
        .unwrap();
    assert_eq!(
        g.apply(1, 2, Action::TouchFive { challenge }, starts_at + 1)
            .unwrap_err(),
        Error::Duplicate
    );
    g.apply(0, 2, Action::TouchFive { challenge }, starts_at + 2)
        .unwrap();
    g.apply(2, 1, Action::TouchFive { challenge }, starts_at + 3)
        .unwrap();
    assert_eq!(g.hands[2].len(), counts[2] + 1);
    assert_eq!(g.hands[0].len(), counts[0]);
    assert_eq!(g.turn, 1);
    assert_eq!(g.last_touch.as_ref().unwrap().order, vec![1, 0, 2]);
    assert_eq!(g.last_touch.as_ref().unwrap().penalty, 2);
    conservation(&g);
}
#[test]
fn five_timeout_only_penalizes_a_nonresponder_and_never_freezes() {
    for seed in 1..40 {
        let mut g = five_game();
        g.rng = seed;
        let Phase::TouchFive {
            challenge,
            starts_at,
            ..
        } = g.phase.clone()
        else {
            panic!()
        };
        g.apply(0, 2, Action::TouchFive { challenge }, starts_at)
            .unwrap();
        g.tick(g.deadline).unwrap();
        let result = g.last_touch.as_ref().unwrap();
        assert!(result.missed.contains(&result.penalty));
        assert_ne!(result.penalty, 0);
        assert!(matches!(g.phase, Phase::Turn));
        conservation(&g);
    }
}
#[test]
fn five_final_card_waits_and_penalty_can_revoke_provisional_win() {
    for player_last in [false, true] {
        let mut g = fixture(&[&[(R, Value::Number(5))], &[(B, Value::Number(3))]]);
        play(&mut g, 0, true);
        let Phase::TouchFive {
            challenge,
            starts_at,
            ..
        } = g.phase.clone()
        else {
            panic!()
        };
        let first = if player_last { 1 } else { 0 };
        let last = 1 - first;
        g.apply(
            first,
            g.sequences[first] + 1,
            Action::TouchFive { challenge },
            starts_at,
        )
        .unwrap();
        g.apply(
            last,
            g.sequences[last] + 1,
            Action::TouchFive { challenge },
            starts_at + 1,
        )
        .unwrap();
        assert_eq!(
            matches!(g.phase, Phase::Finished { winner: Some(0) }),
            !player_last
        );
        conservation(&g);
    }
}
#[test]
fn grouped_fives_trigger_one_challenge_and_pause_recovery_preserves_ready_time() {
    let mut g = fixture(&[
        &[
            (R, Value::Number(5)),
            (B, Value::Number(5)),
            (R, Value::Number(1)),
        ],
        &[(B, Value::Number(3))],
    ]);
    let ids = g.hands[0][..2].iter().map(|c| c.id).collect();
    g.apply(
        0,
        1,
        Action::PlayNumbers {
            cards: ids,
            uno: true,
        },
        1,
    )
    .unwrap();
    assert_eq!(g.reaction_serial, 1);
    g.pause(101).unwrap();
    let remaining = g.deadline - 101;
    g.resume(10_000).unwrap();
    assert_eq!(g.deadline, 10_000 + remaining);
    let Phase::TouchFive { starts_at, .. } = g.phase else {
        panic!()
    };
    assert_eq!(starts_at, 11_100);
    let mut restored: Game = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    restored.rebase_clock(10_000, 20_000).unwrap();
    let Phase::TouchFive { starts_at, .. } = restored.phase else {
        panic!()
    };
    assert_eq!(starts_at, 21_100);
    conservation(&restored);
}
#[test]
fn legacy_saved_rounds_keep_classic_rules() {
    let mut g = fixture(&[
        &[
            (R, Value::Number(5)),
            (R, Value::Number(0)),
            (B, Value::Number(2)),
        ],
        &[(B, Value::Number(3))],
    ]);
    g.rules_version = "classic-108-single-v1".into();
    let mut raw = serde_json::to_value(&g).unwrap();
    raw.as_object_mut().unwrap().remove("reaction_serial");
    raw.as_object_mut().unwrap().remove("last_touch");
    let mut g: Game = serde_json::from_value(raw).unwrap();
    play(&mut g, 0, true);
    assert!(matches!(g.phase, Phase::Turn));
    assert!(g.last_touch.is_none());
    conservation(&g);
}

#[test]
fn a_group_that_leaves_one_still_needs_an_explicit_uno_call() {
    let mut g = fixture(&[
        &[
            (R, Value::Number(8)),
            (B, Value::Number(8)),
            (R, Value::Number(1)),
        ],
        &[(B, Value::Number(3))],
    ]);
    let cards = g.hands[0][..2].iter().map(|c| c.id).collect();
    g.apply(0, 1, Action::PlayNumbers { cards, uno: false }, 1)
        .unwrap();
    assert_eq!(g.phase, Phase::Uno { player: 0 });
    g.apply(1, 1, Action::CatchUno, 2).unwrap();
    assert_eq!(g.hands[0].len(), 3);
    assert_eq!(g.turn, 1);
    conservation(&g);
}
