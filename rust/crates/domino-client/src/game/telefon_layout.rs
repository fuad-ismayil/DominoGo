//! Contact-based Telefon packing. Matching halves share a physical edge;
//! bends and whole-board fitting use the same tile-half units as the 101 board.
use domino_core::{End, TelefonNode, Tile};

#[derive(Debug, Clone, Copy)]
pub struct Slot {
    pub x: f64,
    pub y: f64,
    pub direction: u8,
    pub vertical: bool,
    ports: [u8; 4],
    double: bool,
    marker: bool,
}
impl Slot {
    pub fn size(self) -> (f64, f64) {
        if self.marker {
            (1.4, 1.4)
        } else if self.vertical {
            (1., 2.)
        } else {
            (2., 1.)
        }
    }
    pub fn port(self, port: u8) -> (f64, f64, u8) {
        let d = self.ports[port as usize];
        let (dx, dy) = vector(d);
        if self.double {
            let (w, h) = self.size();
            (self.x + dx * w / 2., self.y + dy * h / 2., d)
        } else {
            let (ax, ay) = vector(self.direction);
            let sign = if port == 0 { -0.5 } else { 0.5 };
            (
                self.x + ax * sign + dx * 0.5,
                self.y + ay * sign + dy * 0.5,
                d,
            )
        }
    }
}
fn vector(d: u8) -> (f64, f64) {
    match d % 4 {
        0 => (1., 0.),
        1 => (0., 1.),
        2 => (-1., 0.),
        _ => (0., -1.),
    }
}
fn slot(tile: Option<Tile>, direction: u8) -> Slot {
    let double = tile.is_some_and(|t| t[0] == t[1]);
    Slot {
        x: 0.,
        y: 0.,
        direction,
        vertical: if double {
            direction.is_multiple_of(2)
        } else {
            direction % 2 == 1
        },
        ports: [
            (direction + 2) % 4,
            direction,
            (direction + 3) % 4,
            (direction + 1) % 4,
        ],
        double,
        marker: tile.is_none(),
    }
}
fn separated(a: Slot, b: Slot, gap: f64) -> bool {
    let (aw, ah) = a.size();
    let (bw, bh) = b.size();
    (a.x - b.x).abs() + 1e-8 >= (aw + bw) / 2. + gap
        || (a.y - b.y).abs() + 1e-8 >= (ah + bh) / 2. + gap
}
#[derive(Debug)]
pub struct Layout {
    pub slots: Vec<Slot>,
    pub targets: Vec<(End, Slot)>,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Copy)]
struct Entry {
    tile: Option<Tile>,
    parent: Option<usize>,
    port: u8,
}
fn bounds(slots: &[Slot]) -> (f64, f64, f64, f64) {
    let mut b = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for s in slots {
        let (w, h) = s.size();
        b.0 = b.0.min(s.x - w / 2.);
        b.1 = b.1.min(s.y - h / 2.);
        b.2 = b.2.max(s.x + w / 2.);
        b.3 = b.3.max(s.y + h / 2.);
    }
    b
}
fn solve(
    entries: &[Entry],
    placed: &mut Vec<Slot>,
    width: f64,
    height: f64,
    nominal: f64,
    budget: &mut usize,
    compact: bool,
) -> bool {
    let i = placed.len();
    if i == entries.len() {
        return true;
    }
    if *budget == 0 {
        return false;
    }
    *budget -= 1;
    let e = entries[i];
    let Some(parent) = e.parent else {
        placed.push(slot(e.tile, 0));
        return solve(entries, placed, width, height, nominal, budget, compact);
    };
    let original = placed[parent];
    let base = original.ports[e.port as usize];
    let directions = if original.double {
        vec![base]
    } else {
        vec![base, (base + 1) % 4, (base + 3) % 4]
    };
    let mut choices = Vec::new();
    for movement in directions {
        let mut p = original;
        p.ports[e.port as usize] = movement;
        let (x, y, _) = p.port(e.port);
        let (dx, dy) = vector(movement);
        let axes = if e.tile.is_none() || e.tile.is_some_and(|t| t[0] == t[1]) {
            vec![movement]
        } else {
            vec![movement, (movement + 1) % 4, (movement + 3) % 4]
        };
        for direction in axes {
            let mut child = slot(e.tile, direction);
            child.ports[0] = (movement + 2) % 4;
            let distance = if child.marker { 0.775 } else { 0.5 };
            let (ax, ay) = if child.double || child.marker {
                (0., 0.)
            } else {
                let (a, b) = vector(direction);
                (a * 0.5, b * 0.5)
            };
            child.x = x + dx * distance + ax;
            child.y = y + dy * distance + ay;
            if placed
                .iter()
                .enumerate()
                .any(|(j, s)| !separated(child, *s, if j == parent { 0. } else { 0.02 }))
            {
                continue;
            }
            placed.push(child);
            let b = bounds(placed);
            placed.pop();
            let w = b.2 - b.0 + 1.1;
            let h = b.3 - b.1 + 1.1;
            // Shrinking a completed straight chain cannot recover the space
            // above and below it. Make the target tile size a packing constraint
            // so the search must bend before accepting a smaller scale.
            if compact && (w * nominal > width + 1e-8 || h * nominal > height + 1e-8) {
                continue;
            }
            let scale = (width / w).min(height / h).min(nominal);
            let turns = (movement != base) as u8 + (direction != movement) as u8;
            let cost = if compact {
                -scale * 1000. + w * h * 0.01 + f64::from(turns) * 0.1
            } else {
                f64::from(turns) * 1000. + w * h * 0.01
            };
            choices.push((cost, p, child));
        }
    }
    choices.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, p, child) in choices {
        placed[parent] = p;
        placed.push(child);
        if solve(entries, placed, width, height, nominal, budget, compact) {
            return true;
        }
        placed.pop();
        placed[parent] = original;
    }
    false
}

pub fn fit(nodes: &[TelefonNode], width: f64, height: f64, nominal: f64) -> Layout {
    let targets = if nodes.is_empty() {
        vec![(End::Left, 0, false)]
    } else {
        domino_core::telefon::targets(nodes)
    };
    let mut entries: Vec<_> = nodes
        .iter()
        .map(|n| Entry {
            tile: Some(n.tile),
            parent: n.parent,
            port: n.port,
        })
        .collect();
    for (end, _, _) in &targets {
        let (parent, port) = match *end {
            End::Telefon(n, p) => (Some(n as usize), p),
            _ => (None, 0),
        };
        entries.push(Entry {
            tile: None,
            parent,
            port,
        });
    }
    // Reserve controls as soon as their parent is placed. Appending all of them
    // after the tiles could invalidate an otherwise complete compact board and
    // exhaust the search, which previously fell back to a single long row.
    let mut order = Vec::new();
    if nodes.is_empty() {
        order.push(0);
    }
    for i in 0..nodes.len() {
        order.push(i);
        order.extend((nodes.len()..entries.len()).filter(|&j| entries[j].parent == Some(i)));
    }
    let mut index = vec![0; entries.len()];
    for (new, &old) in order.iter().enumerate() {
        index[old] = new;
    }
    let ordered: Vec<_> = order
        .iter()
        .map(|&old| Entry {
            parent: entries[old].parent.map(|p| index[p]),
            ..entries[old]
        })
        .collect();
    let mut placed = Vec::new();
    let mut solved = false;
    let mut solved_in_original_order = false;
    let area: f64 = entries
        .iter()
        .map(|e| if e.tile.is_none() { 1.96 } else { 2. })
        .sum();
    for attempt in 0..16 {
        placed.clear();
        let target = nominal * 0.9f64.powi(attempt);
        let usable_width = width.max(1.) / target - 1.1;
        let usable_height = height.max(1.) / target - 1.1;
        if usable_width <= 0. || usable_height <= 0. || usable_width * usable_height < area {
            continue;
        }
        if solve(
            &ordered,
            &mut placed,
            width.max(1.),
            height.max(1.),
            target,
            &mut 6_000,
            true,
        ) {
            solved = true;
            break;
        }
        placed.clear();
        if solve(
            &entries,
            &mut placed,
            width.max(1.),
            height.max(1.),
            target,
            &mut 12_000,
            true,
        ) {
            solved = true;
            solved_in_original_order = true;
            break;
        }
    }
    // A dense branch junction may need a different traversal to keep all four
    // controls clear. Preserve the established unconstrained safety search only
    // after both packing orders have tried every readable target size.
    if !solved {
        placed.clear();
        solved = solve(
            &entries,
            &mut placed,
            width.max(1.),
            height.max(1.),
            nominal,
            &mut 300_000,
            false,
        );
        solved_in_original_order = true;
    }
    assert!(solved, "valid Telefon board must admit a contact layout");
    let mut slots = vec![slot(None, 0); placed.len()];
    if solved_in_original_order {
        slots = placed;
    } else {
        for (new, &old) in order.iter().enumerate() {
            slots[old] = placed[new];
        }
    }
    let b = bounds(&slots);
    for s in &mut slots {
        s.x += 0.55 - b.0;
        s.y += 0.55 - b.1;
    }
    let markers = slots.split_off(nodes.len());
    Layout {
        slots,
        targets: targets
            .into_iter()
            .zip(markers)
            .map(|((end, _, _), s)| (end, s))
            .collect(),
        width: b.2 - b.0 + 1.1,
        height: b.3 - b.1 + 1.1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn long_chain() -> Vec<TelefonNode> {
        // Euler trail uses each physical domino exactly once. It reproduces
        // the reported late-round board with internal doubles and no side plays.
        let mut edges: Vec<Tile> = (0..=6)
            .flat_map(|a| (a..=6).map(move |b| [a, b]))
            .filter(|t| *t != [2, 3])
            .collect();
        let mut stack = vec![3];
        let mut path = Vec::new();
        while let Some(&pip) = stack.last() {
            if let Some(i) = edges.iter().position(|t| t.contains(&pip)) {
                let tile = edges.remove(i);
                stack.push(if tile[0] == pip { tile[1] } else { tile[0] });
            } else {
                path.push(stack.pop().unwrap());
            }
        }
        path.reverse();
        std::iter::once([2, 3])
            .chain(path.windows(2).map(|p| [p[0], p[1]]))
            .enumerate()
            .map(|(i, tile)| TelefonNode {
                tile,
                parent: i.checked_sub(1),
                port: if i == 0 { 0 } else { 1 },
            })
            .collect()
    }
    #[test]
    fn long_rows_wrap_before_tiles_shrink_into_a_thin_strip() {
        let nodes = long_chain();
        assert_eq!(nodes.len(), 28);
        for count in [16, 21, 28] {
            for (width, height, nominal, minimum) in
                [(356., 404., 32., 23.), (1148., 473., 34., 30.)]
            {
                let board = fit(&nodes[..count], width, height, nominal);
                let scale = (width / board.width)
                    .min(height / board.height)
                    .min(nominal);
                assert!(
                    scale >= minimum,
                    "{count} tiles in {width}×{height}: scale={scale}, {}×{}",
                    board.width,
                    board.height
                );
                if count >= 21 {
                    assert!(
                        board.slots.iter().any(|s| !s.double && s.vertical),
                        "long chain must turn"
                    );
                }
                check(&nodes[..count], width, height);
            }
        }
    }
    fn check(nodes: &[TelefonNode], width: f64, height: f64) {
        let board = fit(nodes, width, height, 34.);
        for (i, s) in board.slots.iter().enumerate() {
            for other in &board.slots[..i] {
                assert!(separated(*s, *other, 0.), "tiles overlap");
            }
            if let Some(p) = nodes[i].parent {
                let (x, y, _) = board.slots[p].port(nodes[i].port);
                let (ex, ey, _) = s.port(0);
                assert!(
                    (x - ex).abs() < 1e-8 && (y - ey).abs() < 1e-8,
                    "disconnected matching halves"
                );
            }
        }
        let all: Vec<_> = board
            .slots
            .iter()
            .chain(board.targets.iter().map(|(_, s)| s))
            .collect();
        for (i, s) in all.iter().enumerate() {
            let (w, h) = s.size();
            assert!(
                s.x - w / 2. >= 0.
                    && s.y - h / 2. >= 0.
                    && s.x + w / 2. <= board.width
                    && s.y + h / 2. <= board.height
            );
            for other in &all[..i] {
                assert!(
                    separated(**s, **other, 0.),
                    "control overlaps tile or another control"
                );
            }
        }
        let scale = (width / board.width).min(height / board.height).min(34.);
        assert!(scale > 0. && scale <= 34.);
    }
    #[test]
    fn every_stage_of_real_branched_rounds_has_touching_tiles_and_clear_targets() {
        use domino_core::*;
        for count in 2..=4 {
            for seed in 0..8 {
                let mut s = create_game_with(
                    &["A", "B", "C", "D"][..count],
                    GameMode::Telefon,
                    &mut GameRng::from_seed(seed),
                );
                let mut checked_count = None;
                for _ in 0..120 {
                    if checked_count != Some(s.telefon.len()) {
                        checked_count = Some(s.telefon.len());
                        for (w, h) in [(286., 128.), (356., 404.), (640., 120.), (1148., 473.)] {
                            check(&s.telefon, w, h);
                        }
                    }
                    if s.status != GameStatus::Playing {
                        break;
                    }
                    let p = &s.players[s.turn_index];
                    s = match get_ai_move(&s, p) {
                        AIAction::Play { tile, end } => play_tile(&s, p, tile, end),
                        AIAction::Draw => draw_from_market(&s, p),
                        AIAction::Pass => pass_turn(&s, p),
                    }
                    .unwrap();
                }
            }
        }
    }
}
