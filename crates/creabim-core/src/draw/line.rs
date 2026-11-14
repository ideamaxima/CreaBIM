//! A line drawn in one view (the Draw tab's Line): two points in the
//! view's drawing coordinates, with its own pen, line pattern, colour and
//! end marks. Like a detail component it shows in its view only.

use crate::styles::{LinePattern, PenName, Rgb, Weight};
use crate::{ElementId, Terminator};
use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DraftLine {
    /// The view it is drawn in.
    pub view: ElementId,
    /// Its start and end, in the view's drawing coordinates.
    pub a: DVec2,
    pub b: DVec2,
    /// Its pen: one of the view's, or a fixed width on paper.
    #[serde(default = "default_weight")]
    pub weight: Weight,
    #[serde(default = "default_pattern")]
    pub pattern: LinePattern,
    /// Its colour; none follows the drawing's ink.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Rgb>,
    /// The marks at its start and end.
    #[serde(default = "no_mark")]
    pub start: Terminator,
    #[serde(default = "no_mark")]
    pub end: Terminator,
}

fn default_weight() -> Weight {
    Weight::Pen(PenName::Projection)
}

fn default_pattern() -> LinePattern {
    LinePattern::Solid
}

fn no_mark() -> Terminator {
    Terminator::None
}

/// How the next line is drawn: what the Line tool's top bar sets.
#[derive(Clone, Debug, PartialEq)]
pub struct LineLook {
    pub weight: Weight,
    pub pattern: LinePattern,
    pub colour: Option<Rgb>,
    pub start: Terminator,
    pub end: Terminator,
}

impl Default for LineLook {
    fn default() -> Self {
        Self {
            weight: default_weight(),
            pattern: default_pattern(),
            colour: None,
            start: no_mark(),
            end: no_mark(),
        }
    }
}

impl DraftLine {
    pub fn new(view: ElementId, a: DVec2, b: DVec2, look: &LineLook) -> Self {
        Self {
            view,
            a,
            b,
            weight: look.weight,
            pattern: look.pattern.clone(),
            colour: look.colour,
            start: look.start,
            end: look.end,
        }
    }

    pub fn length(&self) -> f64 {
        self.a.distance(self.b)
    }

    /// Degrees counter-clockwise from the drawing's x axis, 0 to 360.
    pub fn angle(&self) -> f64 {
        let d = self.b - self.a;
        d.y.atan2(d.x).to_degrees().rem_euclid(360.0)
    }

    /// Keep the start, make it `len` long.
    pub fn set_length(&mut self, len: f64) {
        let dir = (self.b - self.a).normalize_or(DVec2::X);
        self.b = self.a + dir * len;
    }

    /// Keep the start and length, turn it to `deg`.
    pub fn set_angle(&mut self, deg: f64) {
        self.b = self.a + DVec2::from_angle(deg.to_radians()) * self.length();
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(self.a.is_finite() && self.b.is_finite()) {
            return Err("a line's points must be finite".into());
        }
        if self.length() < 1e-6 {
            return Err("a line needs two different points".into());
        }
        if let Weight::Mm(w) = self.weight {
            if !(w.is_finite() && w > 0.0) {
                return Err("a line's width must be above zero".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> DraftLine {
        DraftLine::new(
            ElementId::new(),
            DVec2::ZERO,
            DVec2::new(3.0, 4.0),
            &LineLook::default(),
        )
    }

    #[test]
    fn length_and_angle_move_the_end_only() {
        let mut l = line();
        assert!((l.length() - 5.0).abs() < 1e-9);
        l.set_length(10.0);
        assert!((l.b - DVec2::new(6.0, 8.0)).length() < 1e-9);
        l.set_angle(90.0);
        assert_eq!(l.a, DVec2::ZERO);
        assert!((l.b - DVec2::new(0.0, 10.0)).length() < 1e-9);
    }

    #[test]
    fn a_line_saves_and_reads_back() {
        let mut l = line();
        l.colour = Some(Rgb([1.0, 0.0, 0.0]));
        l.pattern = LinePattern::Dashed;
        l.end = Terminator::Arrow;
        l.weight = Weight::Mm(0.5);
        let s = serde_json::to_string(&l).unwrap();
        assert_eq!(serde_json::from_str::<DraftLine>(&s).unwrap(), l);
    }

    #[test]
    fn a_line_of_one_point_is_refused() {
        let mut l = line();
        l.b = l.a;
        assert!(l.validate().is_err());
    }
}
