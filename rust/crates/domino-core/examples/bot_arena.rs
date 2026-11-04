//! Reproducible strategy comparison: cargo run -p domino-core --example bot_arena
use domino_core::*;
fn main() {
    let mut wins = 0;
    let games = 1024;
    for seed in 0..games {
        let policy = if seed % 2 == 0 { "A" } else { "B" };
        let mut rng = GameRng::from_seed(seed / 2);
        let mut state = create_game_with(&["A", "B"], GameMode::OneOhOne, &mut rng);
        for _ in 0..8000 {
            match state.status {
                GameStatus::Finished => {
                    if state.last_winner.as_deref() == Some(policy) {
                        wins += 1;
                    }
                    break;
                }
                GameStatus::RoundEnd => state = next_round_with(&state, &mut rng).unwrap(),
                GameStatus::Playing => {
                    let player = state.players[state.turn_index].clone();
                    let action = if player == policy {
                        get_ai_move(&state, &player)
                    } else {
                        let mut moves = legal_moves(&state, &player);
                        moves.sort_by_key(|(tile, _)| {
                            std::cmp::Reverse(u16::from(tile[0]) + u16::from(tile[1]))
                        });
                        if let Some(&(tile, end)) = moves.first() {
                            AIAction::Play { tile, end }
                        } else if !state.market.is_empty() {
                            AIAction::Draw
                        } else {
                            AIAction::Pass
                        }
                    };
                    state = match action {
                        AIAction::Play { tile, end } => play_tile(&state, &player, tile, end),
                        AIAction::Draw => draw_from_market(&state, &player),
                        AIAction::Pass => pass_turn(&state, &player),
                    }
                    .unwrap();
                }
            }
        }
        assert_eq!(state.status, GameStatus::Finished);
    }
    println!("Strategy wins against pip-greedy baseline: {wins}/{games} ({:.1}%). Both seats played on each seeded deal.", 100.0 * f64::from(wins) / games as f64);
}
