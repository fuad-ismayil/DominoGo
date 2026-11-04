//! Randomized cross-game simulation: apply legal engine actions until games
//! terminate, asserting structural invariants after every step. Deterministic
//! per seed (uses the crate's seeded RNG everywhere).

mod common;
use common::assert_invariants;
use domino_core::{
    can_play_tile, create_game_with, draw_count_for, draw_from_market, next_round_with, pass_turn,
    play_tile, End, GameMode, GameStatus,
};

fn simulate(players: &[&str], seed: u64, max_steps: usize) {
    let mut rng = domino_core::GameRng::from_seed(seed);
    let mut state = create_game_with(players, GameMode::OneOhOne, &mut rng);
    assert_invariants(&state);

    for step in 0..max_steps {
        match state.status {
            GameStatus::Finished => return,
            GameStatus::RoundEnd => {
                state = next_round_with(&state, &mut rng).expect("a round always deals next");
                continue;
            }
            GameStatus::Playing => {}
        }
        assert_invariants(&state);

        let player = state.players[state.turn_index].clone();
        let hand = state.hands.get(&player).cloned().unwrap_or_default();

        // Try (tile, end) candidates in hand order until the engine accepts one.
        let mut played = false;
        'outer: for tile in &hand {
            for end in [End::Left, End::Right] {
                if can_play_tile(*tile, state.open_ends) {
                    match play_tile(&state, &player, *tile, end) {
                        Ok(next) => {
                            state = next;
                            played = true;
                            break 'outer;
                        }
                        Err(_) => {
                            // Ordering constraints (round-1 double rule, orientation)
                            // make specific candidates illegal; try the next one.
                            continue;
                        }
                    }
                }
            }
        }
        if played {
            continue;
        }

        // No legal move: draw while allowed, otherwise pass (both rules exist).
        if !state.market.is_empty() && draw_count_for(state.players.len()) > 0 {
            state = draw_from_market(&state, &player).expect("market draw with non-empty market");
        } else {
            state = pass_turn(&state, &player).expect("passing is always legal in playing state");
        }
        assert_invariants(&state);
        let _ = step;
    }
    panic!("match did not terminate within {max_steps} actions, seed {seed}, state {state:?}");
}

#[test]
fn random_games_stay_consistent_and_terminate() {
    for seed in 0..48u64 {
        for players in [
            &["A", "B"][..],
            &["A", "B", "C"][..],
            &["A", "B", "C", "D"][..],
        ] {
            simulate(players, seed, 4000);
        }
    }
}
