//! Model checks: furniture and fixtures standing in a doorway, in the way
//! of a door's swing or across a window, and where an object put against
//! a wall can go clear of its doors and windows.
//!
//! A door keeps clear the gap in its wall, a strip in front of it on the
//! side it does not open to (to walk through), and the quarter circle its
//! leaf sweeps on the side it opens to. A window keeps clear only the gap
//! in its wall. Both only up to their head and from their sill: a light
//! over a door or a radiator under a window is fine.

use crate::objects::{self, Object, ObjectType};
use crate::{ElementId, ElementKind, Model, Opening, OpeningKind, Wall};
use glam::DVec2;

/// How far in front of a door, on the side it doesn't open to, is kept
/// clear to walk through.
pub const PASSAGE: f64 = 0.3;
/// Clearance either side of a door or window kept free of objects put
/// against its wall: the frame and architrave.
pub const JAMB: f64 = 0.05;
/// How far in front of a window's wall faces an object counts as fixed
/// across it (a socket in the glass, a radiator in front of a screen).
pub const FACE: f64 = 0.02;

/// What an object is in the way of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocks {
    /// It stands in the gap of the wall, or right in front of a door.
    Opening,
    /// It is in the way of a door's leaf as it opens.
    Swing,
}

/// An object in the way of a door or window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clash {
    pub object: ElementId,
    pub opening: ElementId,
    pub blocks: Blocks,
}

/// The plan zones an opening keeps clear, each a convex polygon, and the
/// heights (absolute) between which it does.
struct Zones {
    parts: Vec<(Blocks, Vec<DVec2>)>,
    z: (f64, f64),
}

fn zones(model: &Model, o: &Opening, w: &Wall) -> Zones {
    let d = w.direction();
    let n = d.perp();
    let t = w.thickness / 2.0;
    let (u0, u1) = (o.offset, o.offset + o.width);
    let at = |u: f64, s: f64| w.start + d * u + n * s;
    let base = crate::walls::base_z(model, w);
    let z = (base + o.sill, base + o.sill + o.height);
    let mut parts = vec![];
    if o.kind == OpeningKind::Door {
        let side = if o.faces_left(w) { 1.0 } else { -1.0 };
        // the gap and the strip in front of it, on the side it doesn't open to
        let back = -side * (t + PASSAGE);
        parts.push((
            Blocks::Opening,
            vec![
                at(u0, back),
                at(u1, back),
                at(u1, side * t),
                at(u0, side * t),
            ],
        ));
        // each leaf's sweep: an arc of the swing angle about its hinge, the
        // main leaf hinged by the door's hand, a second at the other jamb
        let widths = o.frame.leaf_widths(o.width, o.width);
        let turn = o.swing_angle.clamp(0.0, 90.0).to_radians();
        let main = if o.flip_hand { (u1, -1.0) } else { (u0, 1.0) };
        let leaves = [main, (u0 + u1 - main.0, -main.1)];
        for (&(hinge_u, towards), &r) in leaves.iter().zip(&widths) {
            let hinge = at(hinge_u, side * t);
            let along = d * towards;
            let out = n * side;
            let mut sweep = vec![hinge];
            for i in 0..=8 {
                let a = turn * i as f64 / 8.0;
                sweep.push(hinge + (along * a.cos() + out * a.sin()) * r);
            }
            // shut (0°): nothing swept
            if turn > 1e-9 {
                parts.push((Blocks::Swing, sweep));
            }
        }
    } else {
        // and a sliver in front of each face: what is fixed to the wall there
        let t = t + FACE;
        parts.push((
            Blocks::Opening,
            vec![at(u0, -t), at(u1, -t), at(u1, t), at(u0, t)],
        ));
    }
    Zones { parts, z }
}

/// Do two convex polygons overlap by more than touching? (Separating axes.)
fn convex_overlap(a: &[DVec2], b: &[DVec2]) -> bool {
    const EPS: f64 = 1e-4;
    for poly in [a, b] {
        let n = poly.len();
        for i in 0..n {
            let e = poly[(i + 1) % n] - poly[i];
            if e.length_squared() < 1e-18 {
                continue;
            }
            let axis = e.perp().normalize();
            let range = |p: &[DVec2]| {
                p.iter().fold((f64::MAX, f64::MIN), |(lo, hi), q| {
                    let v = q.dot(axis);
                    (lo.min(v), hi.max(v))
                })
            };
            let (a0, a1) = range(a);
            let (b0, b1) = range(b);
            if a1 <= b0 + EPS || b1 <= a0 + EPS {
                return false;
            }
        }
    }
    true
}

/// A placed object's footprint (the hull of its parts in plan) and the
/// heights it spans.
fn footprint(model: &Model, o: &Object, t: &ObjectType) -> (Vec<DVec2>, (f64, f64)) {
    let pts: Vec<DVec2> = t
        .parts
        .iter()
        .flat_map(|p| p.mesh.vertices.iter())
        .map(|v| o.to_plan(v.truncate()))
        .collect();
    let z0 = objects::base_z_of(model, o, t);
    let (lo, hi) = t.bounds();
    (objects::hull(&pts), (z0 + lo.z, z0 + hi.z))
}

/// Elements sitting exactly on another of the same kind and shape (a
/// copy left on its original): every one but the first of each set
/// (MU24). Levels, views, sheets, types and other project settings are
/// Pairs of walls on one level that run along each other and overlap
/// (MU25): parallel, their centre lines closer than half their
/// thicknesses together, and sharing some length. The plan merges their
/// cut fills, so they look clean there, but they stay two solids in 3D.
pub fn overlapping_walls(model: &Model) -> Vec<(crate::ElementId, crate::ElementId)> {
    use crate::ElementKind as K;
    let walls: Vec<(crate::ElementId, &crate::Wall)> = model
        .iter()
        .filter_map(|e| match &e.kind {
            // a curved wall's straight chord says nothing about overlap
            // (MU40: the two halves of a circle share theirs)
            K::Wall(w) if w.length() > 1e-6 && !w.is_arc() => Some((e.id, w)),
            _ => None,
        })
        .collect();
    let mut out = vec![];
    for (i, (a, wa)) in walls.iter().enumerate() {
        let d = wa.direction();
        for (b, wb) in &walls[i + 1..] {
            if wa.level != wb.level || d.perp_dot(wb.direction()).abs() > 1e-3 {
                continue;
            }
            // one stacked on the other (courses, bands: Divide up) shares
            // no height, so it doesn't overlap (MU38)
            let (za, zb) = (
                (wa.base_offset, wa.base_offset + wa.height),
                (wb.base_offset, wb.base_offset + wb.height),
            );
            if za.1.min(zb.1) - za.0.max(zb.0) < 1e-3 {
                continue;
            }
            let off = (wb.start - wa.start).perp_dot(d).abs();
            if off >= (wa.thickness + wb.thickness) / 2.0 - 1e-6 {
                continue;
            }
            let (s0, s1) = ((wb.start - wa.start).dot(d), (wb.end - wa.start).dot(d));
            let (lo, hi) = (s0.min(s1), s0.max(s1));
            let shared = hi.min(wa.length()) - lo.max(0.0);
            if shared > 0.05 {
                out.push((*a, *b));
            }
        }
    }
    out
}

/// never counted.
pub fn stacked_copies(model: &Model) -> Vec<crate::ElementId> {
    use crate::ElementKind as K;
    let mut seen: std::collections::HashMap<String, crate::ElementId> = Default::default();
    let mut out = vec![];
    for e in model.iter() {
        let physical = matches!(
            e.kind,
            K::Wall(_)
                | K::Slab(_)
                | K::Roof(_)
                | K::Column(_)
                | K::Beam(_)
                | K::Footing(_)
                | K::Opening(_)
                | K::Object(_)
                | K::FreeMesh(_)
                | K::Sketch(_)
                | K::Stair(_)
                | K::Railing(_)
                | K::Shaft(_)
        );
        if !physical {
            continue;
        }
        let Ok(key) = serde_json::to_string(&e.kind) else {
            continue;
        };
        if let std::collections::hash_map::Entry::Vacant(v) = seen.entry(key) {
            v.insert(e.id);
        } else {
            out.push(e.id);
        }
    }
    out
}

/// Every object in the way of a door or window on its level.
pub fn objects_in_openings(model: &Model) -> Vec<Clash> {
    let mut openings: Vec<(ElementId, ElementId, Zones)> = vec![];
    for e in model.iter() {
        let ElementKind::Opening(o) = &e.kind else {
            continue;
        };
        let Some(ElementKind::Wall(w)) = model.get(o.host).map(|h| &h.kind) else {
            continue;
        };
        openings.push((e.id, w.level, zones(model, o, w)));
    }
    let mut out = vec![];
    for e in model.iter() {
        let ElementKind::Object(o) = &e.kind else {
            continue;
        };
        let Some(t) = objects::type_of(model, e) else {
            continue;
        };
        let (hull, (z0, z1)) = footprint(model, o, t);
        if hull.len() < 3 {
            continue;
        }
        for (oid, level, zs) in &openings {
            if *level != o.level || z1 <= zs.z.0 + 1e-6 || z0 >= zs.z.1 - 1e-6 {
                continue;
            }
            // the gap first: an object in a doorway is in its swing too
            if let Some((blocks, _)) = zs.parts.iter().find(|(_, p)| convex_overlap(&hull, p)) {
                out.push(Clash {
                    object: e.id,
                    opening: *oid,
                    blocks: *blocks,
                });
            }
        }
    }
    out
}

/// Each clash as a line of a report: "Boiler (Utility) is in door D09's
/// swing", naming the object, its level and the door or window's mark.
pub fn report(model: &Model, clashes: &[Clash]) -> Vec<String> {
    let marks = crate::marks::marks(model);
    clashes
        .iter()
        .map(|c| {
            let name = model.get(c.object).map_or("?", |e| e.name.as_str());
            let level = match model.get(c.object).map(|e| &e.kind) {
                Some(ElementKind::Object(o)) => model.get(o.level).map_or("", |l| l.name.as_str()),
                _ => "",
            };
            let kind = match model.get(c.opening).map(|e| &e.kind) {
                Some(ElementKind::Opening(o)) => match o.kind {
                    OpeningKind::Door => "door",
                    OpeningKind::Window => "window",
                    _ => "opening",
                },
                _ => "opening",
            };
            let mark = marks
                .get(&c.opening)
                .cloned()
                .unwrap_or_else(|| model.get(c.opening).map_or("?".into(), |e| e.name.clone()));
            let what = match c.blocks {
                Blocks::Opening => format!("stands in {kind} {mark}"),
                Blocks::Swing => format!("is in {kind} {mark}'s swing"),
            };
            format!("{name} ({level}) {what}")
        })
        .collect()
}

/// What a layer moved by hand does where its wall meets another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayerFit {
    /// Held back from the join: a gap that far wide at the corner.
    Gap(f64),
    /// Run on past the join: into the other wall's layers that far.
    Overlap(f64),
}

/// A wall layer whose end was moved by hand at a join.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerIssue {
    pub wall: ElementId,
    pub layer: usize,
    /// The wall's end rather than its start.
    pub at_end: bool,
    pub fit: LayerFit,
}

/// Wall layers moved by hand at an end where the wall meets another: held
/// back they leave a gap, run on they overlap what they meet. Ends that meet
/// nothing are left alone, since a layer stopped short or run on there is
/// only longer or shorter. Edits under 1 mm are ignored.
pub fn layer_issues(model: &Model) -> Vec<LayerIssue> {
    let joins = crate::joins::wall_joins(model);
    let mut out = vec![];
    for e in model.iter() {
        let ElementKind::Wall(w) = &e.kind else {
            continue;
        };
        if w.layer_ends.is_empty() {
            continue;
        }
        let ends = joins.get(&e.id);
        for at_end in [false, true] {
            let trimmed = ends.is_some_and(|j| {
                let i = usize::from(at_end);
                let f = if at_end { j.end } else { j.start };
                f != [0.0; 2] || j.core[i] != 0.0 || j.core_line[i].is_some()
            });
            if !trimmed && crate::joins::corner_partner(model, e.id, at_end).is_none() {
                continue;
            }
            for l in &w.layer_ends {
                let by = if at_end { l.end } else { l.start };
                let fit = if by < -1e-3 {
                    LayerFit::Gap(-by)
                } else if by > 1e-3 {
                    LayerFit::Overlap(by)
                } else {
                    continue;
                };
                out.push(LayerIssue {
                    wall: e.id,
                    layer: l.layer,
                    at_end,
                    fit,
                });
            }
        }
    }
    out
}

/// Each layer issue as a line of a report: "GF South: Plaster, gypsum is
/// held back 50 mm at its start, leaving a gap".
pub fn layer_report(model: &Model, issues: &[LayerIssue]) -> Vec<String> {
    issues
        .iter()
        .map(|i| {
            let e = model.get(i.wall);
            let wall = e.map_or("?", |e| e.name.as_str());
            let layer = e
                .and_then(|e| match &e.kind {
                    ElementKind::Wall(w) => w.assembly,
                    _ => None,
                })
                .and_then(|a| match &model.get(a)?.kind {
                    ElementKind::Assembly(a) => a.layers.get(i.layer)?.material,
                    _ => None,
                })
                .and_then(|m| model.get(m))
                .map_or_else(|| format!("layer {}", i.layer + 1), |m| m.name.clone());
            let end = if i.at_end { "end" } else { "start" };
            let mm = |v: f64| (v * 1000.0).round();
            match i.fit {
                LayerFit::Gap(v) => format!(
                    "{wall}: {layer} is held back {} mm at its {end}, leaving a gap",
                    mm(v)
                ),
                LayerFit::Overlap(v) => format!(
                    "{wall}: {layer} runs {} mm on at its {end}, into the wall it meets",
                    mm(v)
                ),
            }
        })
        .collect()
}

/// The stretches along `wall` (distances from its start) that an object
/// spanning heights `z` must keep clear of: its doors and windows at those
/// heights, and their jambs.
pub fn blocked_spans(model: &Model, wall: ElementId, z: (f64, f64)) -> Vec<(f64, f64)> {
    let Some(ElementKind::Wall(w)) = model.get(wall).map(|e| &e.kind) else {
        return vec![];
    };
    let base = crate::walls::base_z(model, w);
    let mut out: Vec<(f64, f64)> = model
        .dependents(wall)
        .into_iter()
        .filter_map(|id| match &model.get(id)?.kind {
            ElementKind::Opening(o) => {
                let (o0, o1) = (base + o.sill, base + o.sill + o.height);
                (z.1 > o0 + 1e-6 && z.0 < o1 - 1e-6)
                    .then_some((o.offset - JAMB, o.offset + o.width + JAMB))
            }
            _ => None,
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// Where along a wall an object `span` wide about its origin (its own x
/// from `span.0` to `span.1`, the way the wall runs) goes nearest `u`
/// without covering any of `blocked`, within `0..len`. `None` when it fits
/// nowhere.
pub fn clear_spot(u: f64, span: (f64, f64), blocked: &[(f64, f64)], len: f64) -> Option<f64> {
    let free = |v: f64| {
        let (a, b) = (v + span.0, v + span.1);
        a >= -1e-9 && b <= len + 1e-9 && blocked.iter().all(|g| b <= g.0 + 1e-9 || a >= g.1 - 1e-9)
    };
    if free(u) {
        return Some(u);
    }
    let mut candidates = vec![-span.0, len - span.1];
    for g in blocked {
        candidates.push(g.0 - span.1);
        candidates.push(g.1 - span.0);
    }
    candidates
        .into_iter()
        .filter(|v| free(*v))
        .min_by(|a, b| (a - u).abs().total_cmp(&(b - u).abs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The swing zones of a 1.4 m door in a straight wall.
    fn swings(leaves: u32, swing_angle: f64) -> Vec<Vec<DVec2>> {
        let mut m = Model::default();
        let l = crate::Element::new(
            "L0",
            crate::ElementKind::Level(crate::Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let w = Wall::new(l.id, DVec2::ZERO, DVec2::new(6.0, 0.0), 0.2, 3.0);
        m.insert(l).unwrap();
        let o = Opening {
            host: crate::ElementId::new(),
            kind: OpeningKind::Door,
            offset: 2.0,
            sill: 0.0,
            width: 1.4,
            height: 2.1,
            material: None,
            frame: crate::OpeningFrame {
                leaves,
                ..Default::default()
            },
            flip_hand: false,
            flip_facing: false,
            swing_angle,
        };
        zones(&m, &o, &w)
            .parts
            .into_iter()
            .filter(|(b, _)| *b == Blocks::Swing)
            .map(|(_, p)| p)
            .collect()
    }

    #[test]
    fn each_leaf_keeps_its_own_sweep_clear_to_its_swing_angle() {
        assert_eq!(swings(1, 90.0).len(), 1);
        let two = swings(2, 90.0);
        assert_eq!(two.len(), 2);
        // each half as wide as the door: 0.7 from its hinge
        for z in &two {
            assert!((z[0].distance(z[1]) - 0.7).abs() < 1e-9);
        }
        // opened 45°: the far edge of the sweep lies at 45° from the wall
        let z = &swings(1, 45.0)[0];
        let edge = *z.last().unwrap() - z[0];
        assert!((edge.y.atan2(edge.x).abs() - std::f64::consts::FRAC_PI_4).abs() < 1e-9);
    }

    #[test]
    fn walls_lying_along_each_other_are_found_and_neighbours_are_not() {
        // MU25: a copy run inside its thickened original
        let mut m = crate::sample::sample_house();
        let (id, w) = m
            .iter()
            .find_map(|e| match &e.kind {
                crate::ElementKind::Wall(w) => Some((e.id, w.clone())),
                _ => None,
            })
            .unwrap();
        let before = overlapping_walls(&m).len();
        let mut copy = w.clone();
        let n = copy.direction().perp();
        copy.start += n * 0.05 + copy.direction() * 0.5;
        copy.end += n * 0.05;
        m.insert(crate::Element::new("Copy", crate::ElementKind::Wall(copy)))
            .unwrap();
        let pairs = overlapping_walls(&m);
        assert_eq!(pairs.len(), before + 1);
        assert!(pairs.iter().any(|(a, b)| *a == id || *b == id));
    }

    #[test]
    fn a_spot_slides_off_a_door_to_the_nearer_side() {
        let blocked = [(2.0, 3.0)];
        // a 1 m wide object centred at 2.4 overlaps the door: it goes left
        assert_eq!(clear_spot(2.4, (-0.5, 0.5), &blocked, 6.0), Some(1.5));
        assert_eq!(clear_spot(2.8, (-0.5, 0.5), &blocked, 6.0), Some(3.5));
        assert_eq!(clear_spot(5.0, (-0.5, 0.5), &blocked, 6.0), Some(5.0));
        // nowhere to go on a wall the door fills
        assert_eq!(clear_spot(0.5, (-0.5, 0.5), &[(0.0, 1.0)], 1.0), None);
    }

    #[test]
    fn layers_moved_at_a_join_are_gaps_or_overlaps_and_free_ends_are_not() {
        let m = crate::sample::typed_house();
        let (id, n) = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Wall(w) => {
                    let a = w.assembly?;
                    match &m.get(a)?.kind {
                        ElementKind::Assembly(a) if a.layers.len() > 1 => {
                            Some((e.id, a.layers.len()))
                        }
                        _ => None,
                    }
                }
                _ => None,
            })
            .unwrap();
        assert!(layer_issues(&m).is_empty());
        let joined = crate::joins::wall_joins(&m)[&id].clone();
        assert!(joined.start != [0.0; 2] || crate::joins::corner_partner(&m, id, false).is_some());
        let mut all: Vec<_> = m.iter().cloned().collect();
        for e in &mut all {
            if let (true, ElementKind::Wall(w)) = (e.id == id, &mut e.kind) {
                w.set_layer_ext(0, false, -0.05);
                w.set_layer_ext(n - 1, false, 0.02);
            }
        }
        let m = Model::from_elements(all).unwrap();
        let issues = layer_issues(&m);
        assert_eq!(issues.len(), 2, "{issues:?}");
        assert!(issues
            .iter()
            .any(|i| i.layer == 0 && matches!(i.fit, LayerFit::Gap(v) if (v - 0.05).abs() < 1e-9)));
        assert!(issues
            .iter()
            .any(|i| i.layer == n - 1 && matches!(i.fit, LayerFit::Overlap(_))));
        let lines = layer_report(&m, &issues);
        assert!(
            lines
                .iter()
                .any(|l| l.contains("held back 50 mm at its start")),
            "{lines:?}"
        );
    }

    #[test]
    fn convex_shapes_that_only_touch_do_not_overlap() {
        let sq = |x: f64| crate::polygon::rectangle(DVec2::new(x, 0.0), DVec2::new(x + 1.0, 1.0));
        assert!(convex_overlap(&sq(0.0), &sq(0.5)));
        assert!(!convex_overlap(&sq(0.0), &sq(1.0)));
        assert!(!convex_overlap(&sq(0.0), &sq(2.0)));
    }
}
