//! Conservative horizontal paging; vertical intent stays locked until release.
#[derive(Clone, Copy, Default, PartialEq)]
enum Axis {
    #[default]
    Undecided,
    Horizontal,
    Vertical,
}
#[derive(Default)]
pub struct Swipe {
    start: Option<(i32, f64, f64, f64)>,
    axis: Axis,
    dx: f64,
}
impl Swipe {
    pub fn begin(&mut self, id: i32, x: f64, y: f64, width: f64) {
        *self = Self {
            start: Some((id, x, y, width)),
            ..Self::default()
        };
    }
    pub fn motion(&mut self, id: i32, x: f64, y: f64) -> f64 {
        let Some((pointer, sx, sy, _)) = self.start else {
            return 0.;
        };
        if pointer != id {
            return 0.;
        }
        let (dx, dy) = (x - sx, y - sy);
        if self.axis == Axis::Undecided && dx.abs().max(dy.abs()) >= 12. {
            self.axis = if dx.abs() > dy.abs() * 1.7 {
                Axis::Horizontal
            } else {
                Axis::Vertical
            };
        }
        self.dx = dx;
        if self.axis == Axis::Horizontal {
            dx.clamp(-160., 160.) * 0.35
        } else {
            0.
        }
    }
    pub fn finish(&mut self, id: i32, x: f64, y: f64) -> Option<i32> {
        if self.start.is_none_or(|(pointer, _, _, _)| pointer != id) {
            return None;
        }
        self.motion(id, x, y);
        let (_, sx, sy, width) = self.start?;
        let enough = self.axis == Axis::Horizontal
            && self.dx.abs() >= (width * 0.2).clamp(56., 100.)
            && self.dx.abs() > (y - sy).abs() * 1.7
            && (x - sx).signum() == self.dx.signum();
        let step = if enough {
            Some(if self.dx < 0. { 1 } else { -1 })
        } else {
            None
        };
        self.cancel();
        step
    }
    pub fn cancel(&mut self) {
        *self = Self::default();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deliberate_swipes_page_once_in_each_direction() {
        let mut s = Swipe::default();
        s.begin(1, 200., 100., 390.);
        s.motion(1, 175., 102.);
        assert_eq!(s.finish(1, 100., 109.), Some(1));
        assert_eq!(s.finish(1, 0., 109.), None);
        s.begin(1, 100., 100., 390.);
        assert_eq!(s.finish(1, 200., 105.), Some(-1));
    }
    #[test]
    fn scrolling_diagonals_small_drags_and_cancellation_never_page() {
        let mut s = Swipe::default();
        s.begin(1, 0., 0., 390.);
        s.motion(1, 4., 20.);
        assert_eq!(s.finish(1, 120., 40.), None);
        s.begin(1, 0., 0., 390.);
        assert_eq!(s.finish(1, 50., 0.), None);
        s.begin(1, 0., 0., 390.);
        assert_eq!(s.finish(1, 100., 80.), None);
        s.begin(1, 0., 0., 390.);
        s.motion(1, 100., 0.);
        s.cancel();
        assert_eq!(s.finish(1, 200., 0.), None);
    }
    #[test]
    fn other_pointer_cannot_commit_and_wide_views_require_more_distance() {
        let mut s = Swipe::default();
        s.begin(1, 0., 0., 1000.);
        assert_eq!(s.finish(2, 200., 0.), None);
        assert_eq!(s.finish(1, 90., 0.), None);
    }
}
