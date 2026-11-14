//! A drawn polyline ([`creabim_core::draw::DraftPolyline`]) as drawing
//! items: its fill (colour and hatch pattern, the pattern turned and
//! scaled as the shape says, anchored on its first corner), then its
//! sides. Its own pen, pattern and colour are applied by [`crate::looks`],
//! as for a drawn line. With neither fill nor outline it is drawn as a
//! faint guide, on screen only, so it can still be found.

use crate::looks::pattern_lines;
use crate::plan::{Pen, PlanItem};
use creabim_core::draw::DraftPolyline;
use creabim_core::styles::HatchPattern;
use creabim_core::ElementId;
use glam::DVec2;

/// The lines of `p` inside `polygon`, turned `rotation` degrees and
/// spaced `scale` times wider, about `origin`; `k` paper mm per drawing
/// unit.
pub fn shape_pattern_lines(
    p: &HatchPattern,
    polygon: &[DVec2],
    k: f64,
    origin: DVec2,
    rotation: f64,
    scale: f64,
) -> Vec<(DVec2, DVec2)> {
    let s = if scale > 0.0 { scale } else { 1.0 };
    let turn = DVec2::from_angle(rotation.to_radians());
    let back = DVec2::from_angle(-rotation.to_radians());
    let local: Vec<DVec2> = polygon
        .iter()
        .map(|q| back.rotate(*q - origin) / s)
        .collect();
    pattern_lines(p, &local, k)
        .into_iter()
        .map(|(a, b)| (origin + turn.rotate(a * s), origin + turn.rotate(b * s)))
        .collect()
}

/// The items of polyline `l`; `mm` is drawing units per paper millimetre,
/// `pattern` its fill's pattern found by name.
pub fn polyline_items(
    id: ElementId,
    l: &DraftPolyline,
    mm: f64,
    pattern: Option<&HatchPattern>,
) -> Vec<PlanItem> {
    let mut out = vec![];
    let filled = l.filled();
    let poly = l.fill_polygon();
    if filled {
        let f = &l.fill;
        let lines = match (pattern, l.points.first()) {
            (Some(p), Some(o)) if mm > 0.0 => {
                shape_pattern_lines(p, &poly, 1.0 / mm, *o, f.rotation, f.scale)
            }
            _ => vec![],
        };
        out.push(PlanItem::Hatched {
            id,
            points: poly,
            rgb: f.colour.map(|c| c.0),
            alpha: 1.0 - f.transparency.clamp(0.0, 1.0),
            lines,
            ink: f.pattern_colour.map(|c| c.0),
        });
    }
    let pen = if l.outline {
        Pen::Projection
    } else if filled {
        return out;
    } else {
        Pen::Guide
    };
    for (a, b) in l.sides().into_iter().chain(l.hole_sides()) {
        out.push(PlanItem::Line { id, pen, a, b });
    }
    out
}

/// What a tag on shape `l` reads: each of `fields` (its area, its
/// perimeter) on its own line, in the project's units.
pub fn reading(
    model: &creabim_core::Model,
    l: &DraftPolyline,
    fields: &[creabim_core::room::RoomField],
) -> String {
    use creabim_core::room::RoomField;
    let units = model.units();
    fields
        .iter()
        .filter_map(|f| match f {
            RoomField::Area => l.area().map(|a| units.area(a)),
            RoomField::Perimeter => Some(units.dimension(l.length())),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::draw::line::LineLook;
    use creabim_core::styles::builtin_pattern;
    use creabim_core::Hatch;

    fn square() -> DraftPolyline {
        let pts = vec![
            DVec2::ZERO,
            DVec2::new(1.0, 0.0),
            DVec2::new(1.0, 1.0),
            DVec2::new(0.0, 1.0),
        ];
        DraftPolyline::new(ElementId::new(), pts, true, &LineLook::default())
    }

    #[test]
    fn born_empty_it_is_four_sides() {
        let items = polyline_items(ElementId::new(), &square(), 0.01, None);
        assert_eq!(items.len(), 4);
        assert!(items.iter().all(|i| matches!(
            i,
            PlanItem::Line {
                pen: Pen::Projection,
                ..
            }
        )));
    }

    #[test]
    fn filled_it_draws_its_fill_first_and_hidden_outline_leaves_the_fill() {
        let mut l = square();
        l.fill.on = true;
        let p = builtin_pattern(Hatch::Diagonal);
        let items = polyline_items(ElementId::new(), &l, 0.01, Some(&p));
        assert!(matches!(&items[0], PlanItem::Hatched { lines, .. } if !lines.is_empty()));
        l.outline = false;
        assert_eq!(
            polyline_items(ElementId::new(), &l, 0.01, Some(&p)).len(),
            1
        );
        l.fill.on = false;
        let ghost = polyline_items(ElementId::new(), &l, 0.01, None);
        assert!(ghost.iter().all(|i| matches!(
            i,
            PlanItem::Line {
                pen: Pen::Guide,
                ..
            }
        )));
    }

    #[test]
    fn a_larger_scale_draws_fewer_lines() {
        let p = builtin_pattern(Hatch::Diagonal);
        let poly = square().points;
        let one = shape_pattern_lines(&p, &poly, 100.0, DVec2::ZERO, 0.0, 1.0).len();
        let two = shape_pattern_lines(&p, &poly, 100.0, DVec2::ZERO, 0.0, 2.0).len();
        assert!(two < one && two > 0);
        let turned = shape_pattern_lines(&p, &poly, 100.0, DVec2::ZERO, 45.0, 1.0);
        assert!(!turned.is_empty());
    }
}
