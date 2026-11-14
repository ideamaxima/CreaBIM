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

/// What a polyline was drawn as: a rectangle keeps its right angles when
/// edited, a free one moves corner by corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    #[default]
    Free,
    Rectangle,
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
    /// Drawn as a rectangle: it stays one.
    #[serde(default, skip_serializing_if = "is_free")]
    pub shape: ShapeKind,
    /// Holes cut in a closed one, each a closed outline inside it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<DVec2>>,
}

fn is_free(k: &ShapeKind) -> bool {
    *k == ShapeKind::Free
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
            shape: ShapeKind::Free,
            holes: vec![],
        }
    }

    /// A rectangle of four corners, anticlockwise.
    pub fn rectangle(view: ElementId, corners: [DVec2; 4], look: &LineLook) -> Self {
        Self {
            shape: ShapeKind::Rectangle,
            ..Self::new(view, corners.to_vec(), true, look)
        }
    }

    /// Is it a rectangle that keeps its right angles?
    pub fn is_rectangle(&self) -> bool {
        self.shape == ShapeKind::Rectangle && self.points.len() == 4 && self.closed
    }

    /// A rectangle's width (corner 0 to 1) and height (1 to 2).
    pub fn size(&self) -> Option<(f64, f64)> {
        self.is_rectangle().then(|| {
            (
                self.points[0].distance(self.points[1]),
                self.points[1].distance(self.points[2]),
            )
        })
    }

    /// Give a rectangle this width and height, corner 0 staying put and its
    /// sides keeping their directions.
    pub fn set_size(&mut self, w: f64, h: f64) {
        if !self.is_rectangle() || w <= 0.0 || h <= 0.0 {
            return;
        }
        let p = &self.points;
        let u = (p[1] - p[0]).normalize_or(DVec2::X);
        let v = (p[3] - p[0]).normalize_or(u.perp());
        let o = p[0];
        self.points = vec![o, o + u * w, o + u * w + v * h, o + v * h];
    }

    /// A rectangle with corner `i` dragged to `q`: the opposite corner
    /// stays, the sides keep their directions (it may turn inside out).
    pub fn drag_corner(&mut self, i: usize, q: DVec2) {
        if !self.is_rectangle() {
            return;
        }
        let p = self.points.clone();
        let o = p[(i + 2) % 4];
        let u = (p[1] - p[0]).normalize_or(DVec2::X);
        let v = u.perp();
        let local = |x: DVec2| DVec2::new((x - o).dot(u), (x - o).dot(v));
        let (old, new) = (local(p[i]), local(q));
        let k = |a: f64, b: f64| if a.abs() > 1e-9 { b / a } else { 1.0 };
        let (sx, sy) = (k(old.x, new.x), k(old.y, new.y));
        self.points = p
            .iter()
            .map(|x| {
                let l = local(*x);
                o + u * (l.x * sx) + v * (l.y * sy)
            })
            .collect();
    }

    /// A rectangle with side `i` (corner `i` to `i+1`) moved `by` square
    /// to itself.
    pub fn push_side(&mut self, i: usize, by: f64) {
        if !self.is_rectangle() {
            return;
        }
        let (a, b) = (i % 4, (i + 1) % 4);
        let n = (self.points[b] - self.points[a])
            .normalize_or(DVec2::X)
            .perp();
        // out of the shape: the centre is on the other side
        let out = if (self.points[a] - self.centre()).dot(n) < 0.0 {
            -n
        } else {
            n
        };
        self.points[a] += out * by;
        self.points[b] += out * by;
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

    /// The area inside, holes left out, when closed (none when open).
    pub fn area(&self) -> Option<f64> {
        if !self.closed || self.points.len() < 3 {
            return None;
        }
        let ring = |p: &[DVec2]| crate::polygon::signed_area2(p).abs() / 2.0;
        Some(ring(&self.points) - self.holes.iter().map(|h| ring(h)).sum::<f64>())
    }

    /// The sides of its holes.
    pub fn hole_sides(&self) -> Vec<(DVec2, DVec2)> {
        self.holes
            .iter()
            .flat_map(|h| (0..h.len()).map(move |i| (h[i], h[(i + 1) % h.len()])))
            .collect()
    }

    /// The outline with its holes as one polygon, for filling.
    pub fn fill_polygon(&self) -> Vec<DVec2> {
        if self.holes.is_empty() {
            self.points.clone()
        } else {
            crate::polygon::bridged(&self.points, &self.holes)
        }
    }

    /// Where its middle is: the centre of its area, holes left out (its
    /// corners' average when it has no area).
    pub fn centroid(&self) -> DVec2 {
        let ring = |p: &[DVec2]| {
            let n = p.len();
            let (mut a, mut c) = (0.0, DVec2::ZERO);
            for i in 0..n {
                let (u, v) = (p[i], p[(i + 1) % n]);
                let k = u.perp_dot(v);
                a += k;
                c += (u + v) * k;
            }
            (
                a.abs() / 2.0,
                if a.abs() > 1e-12 {
                    c / (3.0 * a)
                } else {
                    DVec2::ZERO
                },
            )
        };
        if !self.closed || self.points.len() < 3 {
            return self.centre();
        }
        let (a0, c0) = ring(&self.points);
        let (mut a, mut m) = (a0, c0 * a0);
        for h in &self.holes {
            let (ah, ch) = ring(h);
            a -= ah;
            m -= ch * ah;
        }
        if a > 1e-12 {
            m / a
        } else {
            self.centre()
        }
    }

    /// Cut outline `cutter` out of a closed one: inside it, a hole; across
    /// its edge, it is bitten and comes back as free polylines (the
    /// largest first, none when nothing is left). Err when it misses it or
    /// the cut would leave a bad shape.
    pub fn cut(&self, cutter: &[DVec2]) -> Result<Vec<DraftPolyline>, String> {
        use crate::polygon::{contains, inside, outlines_cross, overlap, signed_area2};
        use crate::regions::{faces, filled, Loop};
        if !self.closed || self.points.len() < 3 || cutter.len() < 3 {
            return Err("only a closed shape takes a hole".into());
        }
        let touches_hole = self.holes.iter().any(|h| overlap(h, cutter));
        if inside(cutter, &self.points) && !outlines_cross(cutter, &self.points) && !touches_hole {
            let mut out = self.clone();
            out.holes.push(cutter.to_vec());
            return Ok(vec![out]);
        }
        if !overlap(cutter, &self.points) {
            return Err("the cut misses the shape: draw it on it".into());
        }
        // what is left: the shape's faces outside its holes and the cut
        let mut loops: Vec<Loop> = vec![Loop {
            points: self.points.clone(),
            solid: false,
        }];
        loops.extend(self.holes.iter().map(|h| Loop {
            points: h.clone(),
            solid: false,
        }));
        loops.push(Loop {
            points: cutter.to_vec(),
            solid: false,
        });
        let kept: Vec<Loop> = faces(&loops, &[])
            .into_iter()
            .filter(|f| {
                let s = f.sample;
                contains(&self.points, s)
                    && !self.holes.iter().any(|h| contains(h, s))
                    && !contains(cutter, s)
            })
            .flat_map(|f| {
                std::iter::once(f.outer)
                    .chain(f.holes)
                    .map(|points| Loop {
                        points,
                        solid: false,
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        // the kept faces merged where they meet
        let rings = filled(&kept, &[]);
        let (outers, holes): (Vec<_>, Vec<_>) =
            rings.into_iter().partition(|r| signed_area2(r) > 0.0);
        let mut pieces: Vec<DraftPolyline> = outers
            .into_iter()
            .map(|o| {
                let mine: Vec<Vec<DVec2>> = holes
                    .iter()
                    .filter(|h| contains(&o, h[0]))
                    .map(|h| h.iter().rev().copied().collect())
                    .collect();
                let mut p = self.clone();
                p.points = o;
                p.holes = mine;
                p.shape = ShapeKind::Free;
                p
            })
            .collect();
        pieces.sort_by(|a, b| b.area().unwrap_or(0.0).total_cmp(&a.area().unwrap_or(0.0)));
        Ok(pieces)
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
        if self.closed && !self.holes.is_empty() {
            if let Some(why) = crate::slab::outline_problem(&self.points, true, &self.holes) {
                return Err(why);
            }
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
    fn a_rectangle_dragged_by_a_corner_keeps_the_opposite_and_its_right_angles() {
        let d = std::f64::consts::FRAC_1_SQRT_2;
        // turned 45°: 2 wide, 1 tall
        let (u, v) = (DVec2::new(d, d), DVec2::new(-d, d));
        let o = DVec2::new(1.0, 1.0);
        let corners = [o, o + u * 2.0, o + u * 2.0 + v, o + v];
        let mut r = DraftPolyline::rectangle(ElementId::new(), corners, &LineLook::default());
        r.drag_corner(2, o + u * 3.0 + v * 2.0);
        assert!(
            (r.points[0] - o).length() < 1e-9,
            "the opposite corner stays"
        );
        let (w, h) = r.size().unwrap();
        assert!((w - 3.0).abs() < 1e-9 && (h - 2.0).abs() < 1e-9);
        let side = |i: usize| r.points[(i + 1) % 4] - r.points[i];
        assert!(side(0).dot(side(1)).abs() < 1e-9, "still square-cornered");
        r.push_side(1, 0.5);
        assert!((r.size().unwrap().0 - 3.5).abs() < 1e-9);
        r.set_size(4.0, 1.0);
        assert!((r.points[0] - o).length() < 1e-9);
        assert!((r.area().unwrap() - 4.0).abs() < 1e-9);
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<DVec2> {
        vec![
            DVec2::new(x0, y0),
            DVec2::new(x1, y0),
            DVec2::new(x1, y1),
            DVec2::new(x0, y1),
        ]
    }

    #[test]
    fn a_cut_inside_is_a_hole_and_the_area_leaves_it_out() {
        let p = square(true); // 2 × 3
        let cut = p.cut(&rect(0.5, 0.5, 1.5, 1.5)).unwrap();
        assert_eq!(cut.len(), 1);
        assert_eq!(cut[0].holes.len(), 1);
        assert!((cut[0].area().unwrap() - 5.0).abs() < 1e-9);
        assert!(cut[0].validate().is_ok());
        assert!(
            p.cut(&rect(5.0, 5.0, 6.0, 6.0)).is_err(),
            "a cut that misses"
        );
    }

    #[test]
    fn a_cut_across_the_edge_bites_and_one_through_it_splits() {
        let p = square(true); // 2 × 3
        let bitten = p.cut(&rect(1.0, -1.0, 3.0, 1.0)).unwrap();
        assert_eq!(bitten.len(), 1);
        assert!(
            (bitten[0].area().unwrap() - 5.0).abs() < 1e-9,
            "2 × 3 less 1 × 1"
        );
        assert_eq!(bitten[0].shape, ShapeKind::Free);
        let split = p.cut(&rect(-1.0, 1.0, 3.0, 2.0)).unwrap();
        assert_eq!(split.len(), 2);
        assert!((split[0].area().unwrap() - 2.0).abs() < 1e-9);
        assert!(
            p.cut(&rect(-1.0, -1.0, 3.0, 4.0)).unwrap().is_empty(),
            "all of it"
        );
    }

    #[test]
    fn its_centroid_leaves_its_holes_out() {
        let mut p = square(true);
        assert!((p.centroid() - DVec2::new(1.0, 1.5)).length() < 1e-9);
        p.holes
            .push(rect(0.0, 1.5, 2.0, 3.0).into_iter().rev().collect());
        assert!((p.centroid() - DVec2::new(1.0, 0.75)).length() < 1e-9);
    }

    #[test]
    fn it_is_born_empty_and_outlined() {
        let p = square(true);
        assert!(!p.filled() && p.outline);
        assert!(p.validate().is_ok());
    }
}
