//! The window's size and place: the first time, 70% of the screen and
//! centred; after that, where it was when CreaBIM last closed.

use eframe::egui::{self, Vec2};

/// How much of the screen the first window takes.
const FIRST_SHARE: f32 = 0.7;
/// The smallest window kept between sessions, in points.
const SMALLEST: Vec2 = Vec2::new(480.0, 320.0);
/// Frames to wait for the screen size before giving up on placing.
const WAIT_FRAMES: u32 = 30;

/// A window's place and size: x, y, width, height in points.
pub type Place = [f32; 4];

/// The first window on a screen of `monitor`: 70% of it, centred.
pub fn first(monitor: Vec2) -> Place {
    let size = monitor * FIRST_SHARE;
    let at = (monitor - size) / 2.0;
    [at.x, at.y, size.x, size.y]
}

/// Can a window kept from last time open at `p` on a screen of `monitor`:
/// big enough, and its top-left part on the screen (a monitor unplugged
/// since would leave it out of reach).
pub fn fits(p: Place, monitor: Vec2) -> bool {
    let [x, y, w, h] = p;
    let reach = 100.0;
    p.iter().all(|v| v.is_finite())
        && w >= SMALLEST.x
        && h >= SMALLEST.y
        && w <= monitor.x * 1.5
        && h <= monitor.y * 1.5
        && x > -w + reach
        && y >= -10.0
        && x < monitor.x - reach
        && y < monitor.y - reach
}

/// Places the window once at start, then follows where it is.
#[derive(Default)]
pub struct Keeper {
    placed: bool,
    frames: u32,
    /// Where the window is now, when it is neither maximised nor full screen.
    pub now: Option<Place>,
}

impl Keeper {
    /// Call each frame; `saved` is where it was left last time.
    pub fn frame(&mut self, ctx: &egui::Context, saved: Option<Place>) {
        let (monitor, inner, outer, maxed) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.monitor_size,
                v.inner_rect,
                v.outer_rect,
                v.maximized.unwrap_or(false) || v.fullscreen.unwrap_or(false),
            )
        });
        if !self.placed {
            self.frames += 1;
            let Some(monitor) = monitor.filter(|m| m.x > 0.0 && m.y > 0.0) else {
                self.placed = self.frames > WAIT_FRAMES;
                return;
            };
            self.placed = true;
            if saved.is_some_and(|p| fits(p, monitor)) {
                return;
            }
            let [x, y, w, h] = first(monitor);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(w, h)));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
            return;
        }
        if let (Some(inner), Some(outer), false) = (inner, outer, maxed) {
            self.now = Some([outer.min.x, outer.min.y, inner.width(), inner.height()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Vec2 = Vec2::new(3840.0, 2160.0);

    #[test]
    fn the_first_window_takes_70_percent_of_the_screen_centred() {
        let [x, y, w, h] = first(SCREEN);
        assert_eq!((w, h), (2688.0, 1512.0));
        assert_eq!((x, y), (576.0, 324.0));
        assert!(fits(first(SCREEN), SCREEN));
    }

    #[test]
    fn a_kept_window_off_the_screen_or_too_small_opens_afresh() {
        assert!(fits([100.0, 100.0, 1400.0, 860.0], SCREEN));
        // a second monitor to the right, since unplugged
        assert!(!fits([4000.0, 100.0, 1400.0, 860.0], SCREEN));
        // above the top: the title bar out of reach
        assert!(!fits([100.0, -500.0, 1400.0, 860.0], SCREEN));
        assert!(!fits([100.0, 100.0, 200.0, 100.0], SCREEN));
        assert!(!fits([f32::NAN, 100.0, 1400.0, 860.0], SCREEN));
    }
}
