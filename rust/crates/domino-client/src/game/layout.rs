//! Contact-based snake packing in tile-half units. Doubles consume one
//! horizontal unit, ordinary tiles two. Two touching vertical tiles bridge
//! rows far enough apart for perpendicular doubles without breaking the chain.
#[derive(Debug, Clone, Copy)]
pub struct Slot {
    pub x: f64,
    pub y: f64,
    pub vertical: bool,
    pub reversed: bool,
}
impl Slot {
    /// Marker geometry stays inside its reserved 2×1 footprint. The inset
    /// leaves about 1–2 screen pixels before the adjacent tile at normal scale.
    pub fn drop_bounds(&self) -> (f64, f64, f64, f64) {
        (self.x - 1. + 0.075, self.y - 0.5, 1.85, 1.)
    }
    pub fn size(&self) -> (f64, f64) {
        if self.vertical {
            (1., 2.)
        } else {
            (2., 1.)
        }
    }
}
#[derive(Debug, Clone)]
pub struct Snake {
    pub slots: Vec<Slot>,
    pub width: f64,
    pub height: f64,
}

pub fn fit(board: &[[u8; 2]], width: f64, height: f64) -> Snake {
    fit_at_scale(board, width, height, if width < 600. { 32. } else { 34. })
}

pub fn fit_at_scale(board: &[[u8; 2]], width: f64, height: f64, nominal: f64) -> Snake {
    let mut doubles = vec![false];
    if !board.is_empty() {
        doubles.extend(board.iter().map(|tile| tile[0] == tile[1]));
        doubles.push(false);
    }
    let full_span: usize = doubles
        .iter()
        .map(|double| if *double { 1 } else { 2 })
        .sum();

    let mut best: Option<Snake> = None;
    let mut best_scale = 0.;
    for span in 4..=full_span.max(4) {
        let chain = pack(&doubles, span as f64);
        let scale = (width.max(1.) / chain.width)
            .min(height.max(1.) / chain.height)
            .min(nominal);
        let same_scale = (scale - best_scale).abs() < 1e-9;
        if scale > best_scale + 1e-9
            || (same_scale && best.as_ref().is_none_or(|old| chain.height < old.height))
        {
            best_scale = scale;
            best = Some(chain);
        }
    }
    best.expect("at least one packing candidate")
}

fn pack(doubles: &[bool], span: f64) -> Snake {
    let mut slots = Vec::with_capacity(doubles.len());
    let (mut cursor, mut row_y, mut reverse, mut row_count) = (0., 0., false, 0);
    let mut index = 0;
    while index < doubles.len() {
        let length = if doubles[index] { 1. } else { 2. };
        let exceeds = if reverse {
            cursor - length < 0.
        } else {
            cursor + length > span
        };
        // Only actual tiles turn a corner; endpoint controls stay horizontal.
        if row_count > 0 && exceeds && index > 0 && index + 1 < doubles.len() - 1 {
            let turn_x = cursor + if reverse { -0.5 } else { 0.5 };
            slots.push(Slot {
                x: turn_x,
                y: row_y + 0.5,
                vertical: true,
                reversed: false,
            });
            slots.push(Slot {
                x: turn_x,
                y: row_y + 2.5,
                vertical: true,
                reversed: false,
            });
            index += 2;
            row_y += 3.;
            reverse = !reverse;
            row_count = 0;
            continue;
        }
        slots.push(Slot {
            x: cursor + if reverse { -length / 2. } else { length / 2. },
            y: row_y,
            vertical: doubles[index],
            reversed: reverse,
        });
        cursor += if reverse { -length } else { length };
        row_count += 1;
        index += 1;
    }
    let min_x = slots
        .iter()
        .map(|s| s.x - s.size().0 / 2.)
        .fold(f64::INFINITY, f64::min);
    let min_y = slots
        .iter()
        .map(|s| s.y - s.size().1 / 2.)
        .fold(f64::INFINITY, f64::min);
    let max_x = slots
        .iter()
        .map(|s| s.x + s.size().0 / 2.)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = slots
        .iter()
        .map(|s| s.y + s.size().1 / 2.)
        .fold(f64::NEG_INFINITY, f64::max);
    for slot in &mut slots {
        slot.x += 0.55 - min_x;
        slot.y += 0.55 - min_y;
    }
    Snake {
        slots,
        width: max_x - min_x + 1.1,
        height: max_y - min_y + 1.1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_connected(chain: &Snake) {
        for pair in chain.slots.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (aw, ah) = a.size();
            let (bw, bh) = b.size();
            let x_gap = (a.x - b.x).abs() - (aw + bw) / 2.;
            let y_gap = (a.y - b.y).abs() - (ah + bh) / 2.;
            assert!(
                (x_gap.abs() < 1e-8 && y_gap < -1e-8) || (y_gap.abs() < 1e-8 && x_gap < -1e-8),
                "disconnected: {a:?} {b:?}"
            );
        }
    }
    fn assert_bounds(chain: &Snake) {
        for (i, a) in chain.slots.iter().enumerate() {
            let (aw, ah) = a.size();
            assert!(a.x - aw / 2. >= 0. && a.x + aw / 2. <= chain.width + 1e-8);
            assert!(a.y - ah / 2. >= 0. && a.y + ah / 2. <= chain.height + 1e-8);
            for b in chain.slots.iter().skip(i + 1) {
                let (bw, bh) = b.size();
                assert!(
                    (a.x - b.x).abs() + 1e-8 >= (aw + bw) / 2.
                        || (a.y - b.y).abs() + 1e-8 >= (ah + bh) / 2.,
                    "overlap: {a:?} {b:?}"
                );
            }
        }
    }
    #[test]
    fn end_markers_never_overlap_a_single_double_or_wrapped_chain() {
        for board in [vec![[1, 1]], vec![[1, 2]], vec![[3, 3]; 28]] {
            for (w, h) in [(286., 128.), (356., 404.), (1148., 473.)] {
                let chain = fit(&board, w, h);
                for end in [0, chain.slots.len() - 1] {
                    let (x, y, width, height) = chain.slots[end].drop_bounds();
                    assert!(
                        x >= 0.
                            && y >= 0.
                            && x + width <= chain.width
                            && y + height <= chain.height
                    );
                    for tile in &chain.slots[1..chain.slots.len() - 1] {
                        let (tw, th) = tile.size();
                        assert!(
                            x + width < tile.x - tw / 2.
                                || tile.x + tw / 2. < x
                                || y + height <= tile.y - th / 2.
                                || tile.y + th / 2. <= y
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn normal_tiles_and_doubles_touch_on_a_straight_chain() {
        let board = [[1, 2], [2, 2], [2, 4], [4, 4], [4, 6]];
        let chain = fit(&board, 1148., 473.);
        assert_eq!(chain.slots.len(), 7);
        assert_eq!(chain.slots[2].size(), (1., 2.));
        assert_connected(&chain);
        assert_bounds(&chain);
        assert!(chain.slots.iter().all(|s| s.y == chain.slots[0].y));
    }
    #[test]
    fn every_stage_of_a_round_stays_connected_without_overlaps() {
        let base: Vec<_> = (0..=6).flat_map(|a| (a..=6).map(move |b| [a, b])).collect();
        for seed in 0..24u64 {
            let mut tiles = base.clone();
            let mut random = seed + 1;
            for i in (1..tiles.len()).rev() {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                tiles.swap(i, (random as usize) % (i + 1));
            }
            for count in 0..=28 {
                for (w, h) in [
                    (286., 128.),
                    (356., 404.),
                    (640., 120.),
                    (1148., 473.),
                    (260., 240.),
                ] {
                    let chain = fit(&tiles[..count], w, h);
                    assert_eq!(chain.slots.len(), if count == 0 { 1 } else { count + 2 });
                    assert!(chain.width.is_finite() && chain.height.is_finite());
                    assert_connected(&chain);
                    assert_bounds(&chain);
                }
            }
        }
    }
    #[test]
    fn wrapped_chain_uses_contacting_turns_and_reverses_second_row() {
        let board = vec![[1, 2]; 28];
        let chain = fit(&board, 286., 200.);
        assert!(chain.slots.iter().any(|s| s.vertical));
        assert!(chain.slots.iter().any(|s| s.reversed));
        assert_connected(&chain);
        assert_bounds(&chain);
    }
}
