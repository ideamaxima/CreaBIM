//! A polyline drawn in one view (the Draw tab's Rectangle, later Polyline
//! and Polygon): corners in the view's drawing coordinates, open or
//! closed, with its own pen, line pattern and colour. A closed one can be
//! filled with a colour, a hatch pattern or both, and its outline can be
//! hidden. Like a line it shows in its view only.

use super::line::LineLook;
use crate::styles::{LinePattern, PenName, Rgb, Weight};
use crate::ElementId;
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// What fills a closed polyline: a colour under a hatch pattern, either
/// or both. It is drawn only while `on`, and remembered while off.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapeFill {
    #[serde(default)]
    pub on: bool,
    /// The colour behind; none leaves it clear under the pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Rgb>,
    /// A hatch pattern of the project's styles, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// The pattern's lines in this colour; none follows the drawing's ink.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_colour: Option<Rgb>,
    /// The pattern turned this far, degrees counter-clockwise.
    #[serde(default)]
    pub rotation: f64,
    /// The pattern's spacing times this.
    #[serde(default = "one")]
    pub scale: f64,
    /// 0 opaque to 1 invisible.
    #[serde(default)]
    pub transparency: f32,
}

fn one() -> f64 {
    1.0
}

impl Default for ShapeFill {
    fn default() -> Self {
        Self {
            on: false,
            colour: Some(Rgb([0.8; 3])),
            pattern: None,
            pattern_colour: None,
            rotation: 0.0,
            scale: 1.0,
            transparency: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DraftPolyline {
    /// The view it is drawn in.
    pub view: ElementId,
    /// Its corners, in the view's drawing coordinates.
    pub points: Vec<DVec2>,
    /// The last corner joins the first.
    #[serde(default)]
    pub closed: bool,
    #[serde(default = "default_weight")]
    pub weight: Weight,
    #[serde(default = "default_pattern")]
    pub pattern: LinePattern,
    /// Its colour; none follows the drawing's ink.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Rgb>,
    /// Drawn only when closed.
    #[serde(default)]
    pub fill: ShapeFill,
    /// The outline is drawn; off leaves the fill alone.
    #[serde(default = "yes")]
    pub outline: bool,
}

fn default_weight() -> Weight {
    Weight::Pen(PenName::Projection)
}

fn default_pattern() -> LinePattern {
    LinePattern::Solid
}

fn yes() -> bool {
    true
}

impl DraftPolyline {
    pub fn new(view: ElementId, points: Vec<DVec2>, closed: bool, look: &LineLook) -> Self {
        Self {
            view,
            points,
            closed,
            weight: look.weight,
            pattern: look.pattern.clone(),
            colour: look.colour,
            fill: ShapeFill::default(),
            outline: true,
        }
    }

    /// Its sides, as pairs of corners.
    pub fn sides(&self) -> Vec<(DVec2, DVec2)> {
        let n = self.points.len();
        let k = if self.closed && n > 2 {
            n
        } else {
            n.saturating_sub(1)
        };
        (0..k)
            .map(|i| (self.points[i], self.points[(i + 1) % n]))
            .collect()
    }

    /// The length of its sides; a closed one's perimeter.
    pub fn length(&self) -> f64 {
        self.sides().iter().map(|(a, b)| a.distance(*b)).sum()
    }

    /// The area inside, when closed (none when open).
    pub fn area(&self) -> Option<f64> {
        if !self.closed || self.points.len() < 3 {
            return None;
        }
        let n = self.points.len();
        let twice: f64 = (0..n)
            .map(|i| self.points[i].perp_dot(self.points[(i + 1) % n]))
            .sum();
        Some(twice.abs() / 2.0)
    }

    /// Is the fill drawn?
    pub fn filled(&self) -> bool {
        self.closed && self.points.len() > 2 && self.fill.on
    }

    pub fn centre(&self) -> DVec2 {
        let n = self.points.len().max(1) as f64;
        self.points.iter().copied().sum::<DVec2>() / n
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.points.iter().any(|p| !p.is_finite()) {
            return Err("a polyline's corners must be finite".into());
        }
        if self.points.len() < 2 {
            return Err("a polyline needs two corners or more".into());
        }
        if self.length() < 1e-6 {
            return Err("a polyline needs two different corners".into());
        }
        if let Weight::Mm(w) = self.weight {
            if !(w.is_finite() && w > 0.0) {
                return Err("a polyline's width must be above zero".into());
            }
        }
        if !(self.fill.scale.is_finite() && self.fill.scale > 0.0) {
            return Err("a fill's scale must be above zero".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(closed: bool) -> DraftPolyline {
        let pts = vec![
            DVec2::ZERO,
            DVec2::new(2.0, 0.0),
            DVec2::new(2.0, 3.0),
            DVec2::new(0.0, 3.0),
        ];
        DraftPolyline::new(ElementId::new(), pts, closed, &LineLook::default())
    }

    #[test]
    fn a_closed_one_has_its_perimeter_and_area() {
        let p = square(true);
        assert!((p.length() - 10.0).abs() < 1e-9);
        assert!((p.area().unwrap() - 6.0).abs() < 1e-9);
        assert_eq!(p.sides().len(), 4);
    }

    #[test]
    fn an_open_one_has_no_area_and_one_side_fewer() {
        let p = square(false);
        assert!((p.length() - 7.0).abs() < 1e-9);
        assert!(p.area().is_none());
        assert_eq!(p.sides().len(), 3);
    }

    #[test]
    fn it_is_born_empty_and_outlined() {
        let p = square(true);
        assert!(!p.filled() && p.outline);
        assert!(p.validate().is_ok());
    }
}
