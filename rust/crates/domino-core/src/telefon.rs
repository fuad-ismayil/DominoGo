//! Telefon's public board topology and scoring. Unused side ports contribute
//! nothing. Any internal double may open a side branch, but only for points.
use crate::{DominoError, End, GameState, GameStatus, TelefonNode, Tile};

pub fn round_bonus(pips: i32) -> i32 {
    ((pips + 2) / 5) * 5
}

pub fn occupied(nodes: &[TelefonNode], node: usize, port: u8) -> bool {
    (port == 0 && nodes[node].parent.is_some())
        || nodes
            .iter()
            .any(|n| n.parent == Some(node) && n.port == port)
}

/// All ordinary exposed ends, plus unused side ports of internal doubles.
pub fn targets(nodes: &[TelefonNode]) -> Vec<(End, u8, bool)> {
    let mut out = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        for port in 0..2 {
            if !occupied(nodes, i, port) {
                out.push((End::Telefon(i as u8, port), n.tile[port as usize], false));
            }
        }
        if n.tile[0] == n.tile[1] && occupied(nodes, i, 0) && occupied(nodes, i, 1) {
            for port in 2..4 {
                if !occupied(nodes, i, port) {
                    out.push((End::Telefon(i as u8, port), n.tile[0], true));
                }
            }
        }
    }
    out
}

pub fn end_total(nodes: &[TelefonNode]) -> i32 {
    if nodes.len() == 1 {
        return i32::from(nodes[0].tile[0] + nodes[0].tile[1]);
    }
    targets(nodes)
        .into_iter()
        .filter(|(_, _, side)| !side)
        .map(|(end, pip, _)| {
            let End::Telefon(i, _) = end else {
                unreachable!()
            };
            i32::from(pip)
                * if nodes[i as usize].tile[0] == nodes[i as usize].tile[1] {
                    2
                } else {
                    1
                }
        })
        .sum()
}

fn attached(nodes: &[TelefonNode], tile: Tile, end: End) -> Option<Vec<TelefonNode>> {
    if nodes.is_empty() {
        return (end == End::Left).then(|| {
            vec![TelefonNode {
                tile,
                parent: None,
                port: 0,
            }]
        });
    }
    let (_, pip, side) = targets(nodes).into_iter().find(|(e, _, _)| *e == end)?;
    let End::Telefon(node, port) = end else {
        return None;
    };
    let tile = if tile[0] == pip {
        tile
    } else if tile[1] == pip {
        [tile[1], tile[0]]
    } else {
        return None;
    };
    let mut next = nodes.to_vec();
    next.push(TelefonNode {
        tile,
        parent: Some(node as usize),
        port,
    });
    let total = end_total(&next);
    if side && (total == 0 || total % 5 != 0) {
        return None;
    }
    Some(next)
}

/// Public preview shares the exact server placement and branch restrictions.
pub fn placement_points(nodes: &[TelefonNode], tile: Tile, end: End) -> Option<i32> {
    attached(nodes, tile, end).map(|next| {
        let total = end_total(&next);
        if total % 5 == 0 {
            total
        } else {
            0
        }
    })
}

pub fn legal_moves(state: &GameState, player: &str) -> Vec<(Tile, End)> {
    if state.status != GameStatus::Playing {
        return Vec::new();
    }
    let Some(hand) = state.hands.get(player) else {
        return Vec::new();
    };
    let ends = if state.telefon.is_empty() {
        vec![End::Left]
    } else {
        targets(&state.telefon).into_iter().map(|t| t.0).collect()
    };
    hand.iter()
        .flat_map(|&tile| {
            ends.iter().copied().filter_map(move |end| {
                if state.telefon.is_empty()
                    && state.round_number == 1
                    && tile != [2, 3]
                    && tile != [3, 2]
                {
                    return None;
                }
                attached(&state.telefon, tile, end).map(|_| (tile, end))
            })
        })
        .collect()
}

pub fn play_tile(
    state: &GameState,
    player: &str,
    tile: Tile,
    end: End,
) -> Result<GameState, DominoError> {
    if state.status != GameStatus::Playing {
        return Err(DominoError::NotPlaying);
    }
    if state.players.get(state.turn_index).map(String::as_str) != Some(player) {
        return Err(DominoError::NotYourTurn);
    }
    let hand = state.hands.get(player).ok_or(DominoError::TileNotInHand)?;
    let index = hand
        .iter()
        .position(|t| *t == tile || *t == [tile[1], tile[0]])
        .ok_or(DominoError::TileNotInHand)?;
    if state.telefon.is_empty() && state.round_number == 1 && tile != [2, 3] && tile != [3, 2] {
        return Err(DominoError::MustStartWithTwoThree);
    }
    let nodes = attached(&state.telefon, tile, end).ok_or(DominoError::InvalidTelefonMove)?;
    let mut next = state.clone();
    next.board.push(nodes.last().expect("placed node").tile);
    next.telefon = nodes;
    next.hands
        .get_mut(player)
        .expect("player hand")
        .remove(index);
    next.consecutive_passes = 0;
    let total = end_total(&next.telefon);
    if total > 0 && total % 5 == 0 {
        next.scores.get_mut(player).expect("player score").permanent += total;
    }
    // End immediately on an in-play winning score, before any hand settlement.
    if next.scores[player].permanent >= 365 {
        next.status = GameStatus::Finished;
        next.last_winner = Some(player.into());
        return Ok(next);
    }
    if next.hands[player].is_empty() {
        let pips: i32 = next
            .hands
            .iter()
            .filter(|(p, _)| p.as_str() != player)
            .map(|(_, h)| {
                if h == &[[0, 0]] {
                    10
                } else {
                    h.iter().map(|t| i32::from(t[0] + t[1])).sum()
                }
            })
            .sum();
        next.scores.get_mut(player).expect("player score").permanent += round_bonus(pips);
        next.last_winner = Some(player.into());
        next.status = GameStatus::RoundEnd;
    } else {
        next.turn_index = (next.turn_index + 1) % next.players.len();
    }
    Ok(crate::check_game_over(&next))
}

impl End {
    pub fn token(self) -> String {
        match self {
            Self::Left => "left".into(),
            Self::Right => "right".into(),
            Self::Telefon(n, p) => format!("telefon-{n}-{p}"),
        }
    }
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => {
                let mut parts = token.strip_prefix("telefon-")?.split('-');
                let n = parts.next()?.parse().ok()?;
                let p = parts.next()?.parse().ok()?;
                (n < 28 && p < 4 && parts.next().is_none()).then_some(Self::Telefon(n, p))
            }
        }
    }
}
