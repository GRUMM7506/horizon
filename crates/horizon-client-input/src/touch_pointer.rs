//! A finger as the pointer of a screen (docs/android-port.md): the menus, the inventory and
//! every other screen are written for a mouse, and on a touch screen one finger stands in for it.
//!
//! The screens act when a button goes down, so a finger cannot simply press as it lands: a
//! swipe over a list would press whatever it started on. A finger that lands only moves the
//! pointer there. It presses once it has stayed put for [`HOLD`], or as it lifts (a tap), or as
//! it sets off sideways (a slider, a dragged item); one that sets off up or down scrolls instead
//! and never presses. Pure state: nothing here knows about the window or the game.
use std::time::{Duration, Instant};
use winit::event::TouchPhase;

/// A finger held still this long presses, so that a drag can start under it.
pub const HOLD: Duration = Duration::from_millis(150);

/// What the pointer does, in the order it happens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pointer {
    /// Moved to this point of the surface, in pixels.
    Moved([f32; 2]),
    /// Its button (the left one) went down or up.
    Button(bool),
    /// Scrolled by this many pixels, positive as the finger moves down the screen.
    Scrolled(f32),
}

#[derive(Clone, Copy, Debug)]
enum State {
    /// Landed, and has neither pressed nor scrolled yet.
    Landed {
        at: [f32; 2],
        since: Instant,
    },
    Pressed,
    Scrolling {
        y: f32,
    },
}

/// One finger at a time: a second one that lands while the first is down does nothing.
#[derive(Default, Debug)]
pub struct TouchPointer {
    finger: Option<(u64, State)>,
}

impl TouchPointer {
    /// Whether finger `id` is the pointer.
    pub fn owns(&self, id: u64) -> bool {
        self.finger.is_some_and(|(finger, _)| finger == id)
    }

    /// A touch event of finger `id` at `at` (pixels). A finger moves the pointer once it is
    /// more than `slop` pixels from where it landed.
    pub fn touch(
        &mut self,
        id: u64,
        phase: TouchPhase,
        at: [f32; 2],
        slop: f32,
        now: Instant,
    ) -> Vec<Pointer> {
        if phase == TouchPhase::Started {
            if self.finger.is_some() {
                return Vec::new();
            }
            self.finger = Some((id, State::Landed { at, since: now }));
            return vec![Pointer::Moved(at)];
        }
        let Some((_, state)) = self.finger.as_mut().filter(|(finger, _)| *finger == id) else {
            return Vec::new();
        };
        let out = match (phase, *state) {
            (TouchPhase::Moved, State::Landed { at: start, .. }) => {
                let [dx, dy] = [at[0] - start[0], at[1] - start[1]];
                if dx.hypot(dy) <= slop {
                    Vec::new()
                } else if dy.abs() > dx.abs() {
                    *state = State::Scrolling { y: at[1] };
                    vec![Pointer::Scrolled(dy)]
                } else {
                    *state = State::Pressed;
                    vec![Pointer::Button(true), Pointer::Moved(at)]
                }
            }
            (TouchPhase::Moved, State::Pressed) => vec![Pointer::Moved(at)],
            (TouchPhase::Moved, State::Scrolling { y }) => {
                *state = State::Scrolling { y: at[1] };
                vec![Pointer::Scrolled(at[1] - y)]
            }
            // A tap: it presses and lets go where it landed.
            (TouchPhase::Ended, State::Landed { .. }) => {
                vec![Pointer::Button(true), Pointer::Button(false)]
            }
            (TouchPhase::Ended, State::Pressed) => {
                vec![Pointer::Moved(at), Pointer::Button(false)]
            }
            (TouchPhase::Cancelled, State::Pressed) => vec![Pointer::Button(false)],
            _ => Vec::new(),
        };
        if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            self.finger = None;
        }
        out
    }

    /// Once per frame: a finger that stayed where it landed presses.
    pub fn poll(&mut self, now: Instant) -> Vec<Pointer> {
        match &mut self.finger {
            Some((_, state @ State::Landed { .. })) => {
                let State::Landed { since, .. } = *state else {
                    return Vec::new();
                };
                if now.duration_since(since) < HOLD {
                    return Vec::new();
                }
                *state = State::Pressed;
                vec![Pointer::Button(true)]
            }
            _ => Vec::new(),
        }
    }

    /// The screen went away under the finger: a held button comes up, and the finger does
    /// nothing more.
    pub fn cancel(&mut self) -> Vec<Pointer> {
        match self.finger.take() {
            Some((_, State::Pressed)) => vec![Pointer::Button(false)],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TouchPhase::{Cancelled, Ended, Moved, Started};

    const SLOP: f32 = 10.0;

    #[test]
    fn a_tap_moves_the_pointer_then_clicks_where_it_landed() {
        let now = Instant::now();
        let mut p = TouchPointer::default();
        assert_eq!(
            p.touch(1, Started, [50.0, 60.0], SLOP, now),
            [Pointer::Moved([50.0, 60.0])]
        );
        // A finger never rests exactly still.
        assert_eq!(p.touch(1, Moved, [53.0, 62.0], SLOP, now), []);
        assert_eq!(
            p.touch(1, Ended, [53.0, 62.0], SLOP, now),
            [Pointer::Button(true), Pointer::Button(false)]
        );
        assert!(!p.owns(1));
    }

    #[test]
    fn a_vertical_swipe_scrolls_and_never_presses() {
        let now = Instant::now();
        let mut p = TouchPointer::default();
        p.touch(1, Started, [50.0, 100.0], SLOP, now);
        assert_eq!(
            p.touch(1, Moved, [52.0, 130.0], SLOP, now),
            [Pointer::Scrolled(30.0)]
        );
        assert_eq!(
            p.touch(1, Moved, [52.0, 120.0], SLOP, now),
            [Pointer::Scrolled(-10.0)]
        );
        // Held long after: still a scroll.
        assert_eq!(p.poll(now + HOLD * 4), []);
        assert_eq!(p.touch(1, Ended, [52.0, 120.0], SLOP, now), []);
    }

    #[test]
    fn a_sideways_move_presses_where_it_landed_and_drags() {
        let now = Instant::now();
        let mut p = TouchPointer::default();
        p.touch(1, Started, [50.0, 100.0], SLOP, now);
        assert_eq!(
            p.touch(1, Moved, [80.0, 104.0], SLOP, now),
            [Pointer::Button(true), Pointer::Moved([80.0, 104.0])]
        );
        // Once pressed, moving down drags too.
        assert_eq!(
            p.touch(1, Moved, [80.0, 200.0], SLOP, now),
            [Pointer::Moved([80.0, 200.0])]
        );
        assert_eq!(
            p.touch(1, Ended, [81.0, 200.0], SLOP, now),
            [Pointer::Moved([81.0, 200.0]), Pointer::Button(false)]
        );
    }

    #[test]
    fn a_finger_held_still_presses_so_a_drag_can_follow() {
        let now = Instant::now();
        let mut p = TouchPointer::default();
        p.touch(1, Started, [50.0, 100.0], SLOP, now);
        assert_eq!(p.poll(now + HOLD / 2), []);
        assert_eq!(p.poll(now + HOLD), [Pointer::Button(true)]);
        assert_eq!(p.poll(now + HOLD * 2), [], "pressed once");
        assert_eq!(
            p.touch(1, Moved, [50.0, 160.0], SLOP, now),
            [Pointer::Moved([50.0, 160.0])]
        );
    }

    #[test]
    fn a_second_finger_is_not_the_pointer_and_a_cancel_lets_go() {
        let now = Instant::now();
        let mut p = TouchPointer::default();
        p.touch(1, Started, [50.0, 100.0], SLOP, now);
        assert_eq!(p.touch(2, Started, [300.0, 100.0], SLOP, now), []);
        assert_eq!(p.touch(2, Ended, [300.0, 100.0], SLOP, now), []);
        assert!(p.owns(1));
        p.poll(now + HOLD);
        assert_eq!(
            p.touch(1, Cancelled, [50.0, 100.0], SLOP, now),
            [Pointer::Button(false)]
        );
        // A finger that never landed here (it began on the game's own controls) does nothing.
        assert_eq!(p.touch(7, Moved, [1.0, 1.0], SLOP, now), []);
        assert_eq!(p.cancel(), []);
    }
}
