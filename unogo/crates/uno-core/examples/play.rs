//! Local rules playground; deliberately has no separate accounts or server.
use std::{
    io::{self, Write},
    time::Instant,
};
use uno_core::{bot_action, Action, Color, Game, Phase, View};

fn print_table(v: &View) {
    println!(
        "\nTop: {:?} {:?} | Active: {:?} | Direction: {} | Turn: seat {}",
        v.top.color,
        v.top.value,
        v.color,
        if v.direction == 1 {
            "clockwise"
        } else {
            "counterclockwise"
        },
        v.turn + 1
    );
    println!("Cards remaining: {:?}", v.counts);
    for (i, c) in v.hand.iter().enumerate() {
        println!(
            "  {}: {:?} {:?}{}",
            i + 1,
            c.color,
            c.value,
            if v.playable.contains(&c.id) {
                "  * playable"
            } else {
                ""
            }
        );
    }
}
fn color(text: Option<&str>) -> Option<Color> {
    match text {
        Some("r") => Some(Color::Red),
        Some("y") => Some(Color::Yellow),
        Some("g") => Some(Color::Green),
        Some("b") => Some(Color::Blue),
        _ => None,
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let seats = std::env::args()
        .nth(1)
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(4);
    let started = Instant::now();
    let mut game = Game::new(seats, 0)?;
    let mut sequences = vec![0u64; seats];
    println!("UnoGo — Classic / Single round. You are seat 1; other seats are bots.");
    println!("p NUMBER [r/y/g/b] [uno] | d draw | k keep | u UNO | c catch | a accept +4 | x challenge | color r/y/g/b | q quit");
    println!("Include 'uno' with a play to declare it. Timers resolve when you submit input.");
    loop {
        let now = started.elapsed().as_millis() as u64;
        game.tick(now)?;
        let view = game.view(0)?;
        if let Phase::Finished { winner } = view.phase {
            print_table(&view);
            match winner {
                Some(seat) => println!("Seat {} wins!", seat + 1),
                None => println!("No contest: no cards or progress available."),
            }
            break;
        }
        // An undeclared human UNO window is offered to the human for recovery;
        // bot players declare atomically, as they do in production policy.
        let actor = match view.phase {
            Phase::Uno { player } => player,
            _ => view.turn,
        };
        if actor != 0 {
            let bot_view = game.view(actor)?;
            let action = bot_action(&bot_view, actor).ok_or("bot has no action")?;
            println!("Seat {}: {:?}", actor + 1, action);
            sequences[actor] += 1;
            game.apply(actor, sequences[actor], action, now)?;
            continue;
        }
        print_table(&view);
        println!("Phase: {:?}", view.phase);
        print!("> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let words: Vec<_> = line.split_whitespace().collect();
        let action = match words.first().copied() {
            Some("q") => break,
            Some("d") => Some(Action::Draw),
            Some("k") => Some(Action::Keep),
            Some("u") => Some(Action::CallUno),
            Some("c") => Some(Action::CatchUno),
            Some("a") => Some(Action::AcceptFour),
            Some("x") => Some(Action::ChallengeFour),
            Some("color") => color(words.get(1).copied()).map(Action::ChooseColor),
            Some("p") => words
                .get(1)
                .and_then(|v| v.parse::<usize>().ok())
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| view.hand.get(i))
                .map(|card| Action::Play {
                    card: card.id,
                    color: color(words.get(2).copied()),
                    uno: words.contains(&"uno"),
                }),
            _ => None,
        };
        if let Some(action) = action {
            let now = started.elapsed().as_millis() as u64;
            if game.tick(now)? {
                println!("Decision timed out; table updated.");
                continue;
            }
            sequences[0] += 1;
            match game.apply(0, sequences[0], action, now) {
                Ok(receipt) => {
                    if let Some(evidence) = receipt.challenge_evidence {
                        println!(
                            "Private challenge evidence: {:?}. Successful: {:?}",
                            evidence, receipt.challenge_succeeded
                        );
                    }
                }
                Err(error) => println!("{error}"),
            }
        } else {
            println!("Unknown command or card number.");
        }
    }
    Ok(())
}
