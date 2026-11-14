//! The tracker (brief 02, 2.10): while a line tool is drawing (a wall, a
//! beam, a grid line, a section...), a pill by the pointer shows the
//! **Length** and **Angle** of the line, and the step in use. Typing at
//! any moment fills the first field; Tab moves to the next; Enter places
//! the point; Backspace takes a character back and Esc clears what was
//! typed. A field left empty follows the pointer.

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke, Vec2};
use glam::DVec2;

/// What has been typed into the tracker so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracker {
    /// The Length field, then the Angle field (degrees).
    pub fields: [String; 2],
    /// The field typing goes to.
    pub active: usize,
}

/// What a frame's keys did to the tracker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keys {
    /// Nothing for the tracker.
    None,
    /// The fields changed.
    Typed,
    /// Enter: place the point.
    Enter,
}

impl Tracker {
    /// Has anything been typed?
    pub fn typing(&self) -> bool {
        self.fields.iter().any(|f| !f.is_empty())
    }

    pub fn clear(&mut self) {
        *self = Tracker::default();
    }

    /// Typing goes to the next field (Tab).
    pub fn next_field(&mut self) {
        self.active = (self.active + 1) % self.fields.len();
    }

    /// Take this frame's keys: digits and length marks go into the active
    /// field, Tab moves on, Backspace takes back, Esc clears, Enter places
    /// (only once something has been typed). Keys the tracker takes are
    /// consumed so no shortcut sees them.
    pub fn keys(&mut self, ctx: &egui::Context) -> Keys {
        if ctx.egui_wants_keyboard_input() {
            return Keys::None;
        }
        let typing = self.typing();
        let (text, tab, back, esc, enter) = ctx.input_mut(|i| {
            let mut text = String::new();
            i.events.retain(|e| match e {
                egui::Event::Text(t) if t.chars().all(accepts) => {
                    text.push_str(t);
                    false
                }
                _ => true,
            });
            let tab = i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
            let back = typing && i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace);
            let esc = typing && i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
            let enter = typing && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
            (text, tab, back, esc, enter)
        });
        if esc {
            self.clear();
            return Keys::Typed;
        }
        if enter {
            return Keys::Enter;
        }
        let mut changed = false;
        for c in text.chars() {
            self.fields[self.active].push(if c == ',' { '.' } else { c });
            changed = true;
        }
        if tab {
            self.active = (self.active + 1) % self.fields.len();
            changed = true;
        }
        if back {
            if self.fields[self.active].pop().is_none() && self.active > 0 {
                self.active -= 1;
            }
            changed = true;
        }
        if changed {
            Keys::Typed
        } else {
            Keys::None
        }
    }

    /// The typed length, metres, if one reads as a length.
    pub fn length(&self) -> Option<f64> {
        let t = self.fields[0].trim();
        (!t.is_empty())
            .then(|| crate::units::parse(t))
            .flatten()
            .filter(|l| *l > 0.0)
    }

    /// The typed angle, degrees counter-clockwise from +X.
    pub fn angle(&self) -> Option<f64> {
        let t = self.fields[1].trim();
        (!t.is_empty()).then(|| t.parse::<f64>().ok()).flatten()
    }

    /// Where the line from `a` ends: the pointer's point `b`, with the
    /// typed length and angle taking over from it.
    pub fn end(&self, a: DVec2, b: DVec2) -> DVec2 {
        let d = b - a;
        let len = self.length().unwrap_or(d.length());
        let ang = self.angle().map(f64::to_radians).unwrap_or_else(|| {
            if d.length() > 1e-9 {
                d.y.atan2(d.x)
            } else {
                0.0
            }
        });
        a + DVec2::from_angle(ang) * len
    }
}

/// Characters the tracker takes: numbers and the marks lengths are written
/// with (letters stay shortcuts).
fn accepts(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '\'' | '"' | '/' | ' ')
}

/// The line's angle, degrees counter-clockwise from +X, 0 to 360.
pub fn angle_of(a: DVec2, b: DVec2) -> f64 {
    let d = b - a;
    d.y.atan2(d.x).to_degrees().rem_euclid(360.0)
}

/// The tracker's pill below and right of `at`: "Length 3000  Angle 90°
/// Step 50 mm", a typed field in the accent with a caret on the active
/// one, on the theme's raised surface.
pub fn draw(
    painter: &Painter,
    at: Pos2,
    t: &Tracker,
    len: f64,
    angle: f64,
    step: &str,
    look: &crate::handles::PillLook,
) {
    let fields = [
        ("Length", bare(len), ""),
        ("Angle", format!("{angle:.1}"), "°"),
    ];
    draw_fields(painter, at, t, &fields, step, look);
}

/// A length as the tracker shows it: its number alone.
pub fn bare_len(metres: f64) -> String {
    bare(metres)
}

/// The tracker's pill with other fields: each its label, the value the
/// pointer gives (shown while nothing is typed) and a mark after it.
pub fn draw_fields(
    painter: &Painter,
    at: Pos2,
    t: &Tracker,
    fields: &[(&str, String, &str); 2],
    step: &str,
    look: &crate::handles::PillLook,
) {
    let font = egui::FontId::proportional(11.5);
    let mut job = egui::text::LayoutJob::default();
    let fmt = |c: Color32, under: bool| egui::TextFormat {
        font_id: font.clone(),
        color: c,
        underline: if under {
            Stroke::new(1.0, c)
        } else {
            Stroke::NONE
        },
        ..Default::default()
    };
    let typed = |i: usize| !t.fields[i].is_empty();
    let on = |i: usize| t.typing() && t.active == i;
    for (i, (label, value, mark)) in fields.iter().enumerate() {
        let gap = if i == 0 { "" } else { "   " };
        job.append(&format!("{gap}{label} "), 0.0, fmt(look.text, false));
        let text = if typed(i) {
            format!("{}{mark}", t.fields[i])
        } else {
            format!("{value}{mark}")
        };
        let colour = if typed(i) || on(i) {
            look.number
        } else {
            look.text
        };
        job.append(&text, 0.0, fmt(colour, on(i)));
    }
    job.append(
        &format!("   Step {step}"),
        0.0,
        fmt(look.text.gamma_multiply(0.75), false),
    );
    let galley = painter.layout_job(job);
    let size = galley.size() + Vec2::new(14.0, 6.0);
    let mut rect = Rect::from_min_size(at + Vec2::new(18.0, 18.0), size);
    let clip = painter.clip_rect().shrink(4.0);
    rect = rect.translate(Vec2::new(
        (clip.right() - rect.right()).min(0.0),
        (clip.bottom() - rect.bottom()).min(0.0),
    ));
    painter.rect(
        rect,
        6.0,
        look.ground,
        Stroke::new(1.0, look.line),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.min + Vec2::new(7.0, 3.0), galley, look.text);
}

/// A length's number alone, in the project's units.
fn bare(metres: f64) -> String {
    let s = crate::units::len(metres);
    let unit = format!(" {}", crate::units::symbol());
    s.strip_suffix(&unit).map(str::to_string).unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_length_and_angle_take_over_from_the_pointer() {
        crate::units::set(creabim_core::Units::metric());
        let a = DVec2::new(1.0, 1.0);
        let pointer = DVec2::new(4.2, 1.1);
        let mut t = Tracker::default();
        // nothing typed: the pointer
        assert!(t.end(a, pointer).distance(pointer) < 1e-9);
        // a length: along the pointer's way, that long
        t.fields[0] = "3000".into();
        let e = t.end(a, pointer);
        assert!((e.distance(a) - 3.0).abs() < 1e-9);
        assert!((angle_of(a, e) - angle_of(a, pointer)).abs() < 1e-9);
        // and an angle
        t.fields[1] = "90".into();
        assert!(t.end(a, pointer).distance(DVec2::new(1.0, 4.0)) < 1e-9);
        // units can be typed
        t.fields[0] = "2.5 m".into();
        assert!((t.length().unwrap() - 2.5).abs() < 1e-9);
    }

    #[test]
    fn the_tracker_takes_numbers_but_leaves_letters_to_shortcuts() {
        assert!(accepts('7') && accepts('.') && accepts('"'));
        assert!(!accepts('m') && !accepts('W'));
    }

    #[test]
    fn the_step_is_chosen_and_said() {
        crate::units::set(creabim_core::Units::metric());
        let mut s = crate::snaps::SnapSettings::ALL;
        // grid snap starts off (the owner's snapping review)
        assert_eq!(s.step_label(), "Off");
        s.increment = true;
        assert_eq!(s.step_label(), "50 mm");
        assert!((s.step_len() - 0.05).abs() < 1e-12);
        s.step_um = 100_000;
        assert_eq!(s.step_label(), "100 mm");
        assert!((s.step_len() - 0.1).abs() < 1e-12);
        s.increment = false;
        assert_eq!(s.step_label(), "Off");
        assert_eq!(s.step_len(), 0.0);
    }
}
