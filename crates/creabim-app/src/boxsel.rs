//! Box selection, the same in plans, sections and 3D. Dragging left to
//! right takes what lies wholly inside the box; right to left also takes
//! what the box only touches, as in CAD.

use creabim_core::ElementId;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use std::collections::HashMap;

/// The shape a drag selects with (MU39, the tool bar's first chips).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Box,
    Circle,
    Lasso,
}

/// How what a drag (or click) takes joins what is selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Combine {
    New,
    Add,
    Subtract,
    Invert,
    Intersect,
}

static SHAPE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static COMBINE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

thread_local! {
    /// The lasso's points while it is drawn.
    static PATH: std::cell::RefCell<Vec<Pos2>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn shape() -> Shape {
    match SHAPE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => Shape::Circle,
        2 => Shape::Lasso,
        _ => Shape::Box,
    }
}

pub fn set_shape(s: Shape) {
    SHAPE.store(s as u8, std::sync::atomic::Ordering::Relaxed);
}

pub fn combine() -> Combine {
    match COMBINE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => Combine::Add,
        2 => Combine::Subtract,
        3 => Combine::Invert,
        4 => Combine::Intersect,
        _ => Combine::New,
    }
}

pub fn set_combine(c: Combine) {
    COMBINE.store(c as u8, std::sync::atomic::Ordering::Relaxed);
}

/// The combine a drag uses: the bar's, unless Shift (add) or Ctrl
/// (subtract) is held, as in Blender.
pub fn combine_with(shift: bool, ctrl: bool) -> Combine {
    match (shift, ctrl) {
        (_, true) => Combine::Subtract,
        (true, _) => Combine::Add,
        _ => combine(),
    }
}

/// Start a drag's lasso path at `p`, or add the pointer to it.
pub fn trace(p: Pos2, start: bool) {
    PATH.with(|v| {
        let mut v = v.borrow_mut();
        if start {
            v.clear();
        }
        if v.last().is_none_or(|l| l.distance(p) > 3.0) {
            v.push(p);
        }
    });
}

fn lasso() -> Vec<Pos2> {
    PATH.with(|v| v.borrow().clone())
}

/// Is `p` inside polygon `poly`?
fn in_poly(poly: &[Pos2], p: Pos2) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// What a drag from `start` to `now` covers, for circle and lasso: points
/// inside it (a circle round where the drag started).
fn covers(start: Pos2, now: Pos2, p: Pos2) -> bool {
    match shape() {
        Shape::Box => Rect::from_two_pos(start, now).contains(p),
        Shape::Circle => p.distance(start) <= start.distance(now),
        Shape::Lasso => {
            let l = lasso();
            l.len() >= 3 && in_poly(&l, p)
        }
    }
}

/// Does a drag from `start` to `now` take what it touches (green, dashed),
/// or only what lies wholly inside (blue)? Drawn leftward it touches, as
/// in CAD: a box, a circle, or a lasso ending left of where it began (MU39).
pub fn crossing(start: Pos2, now: Pos2) -> bool {
    match shape() {
        Shape::Lasso => {
            let l = lasso();
            match (l.first(), l.last()) {
                (Some(a), Some(b)) => b.x < a.x,
                _ => now.x < start.x,
            }
        }
        _ => now.x < start.x,
    }
}

fn look(start: Pos2, now: Pos2) -> (Color32, Stroke) {
    if crossing(start, now) {
        (
            Color32::from_rgba_unmultiplied(60, 170, 90, 30),
            Stroke::new(1.0, Color32::from_rgb(40, 140, 70)),
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(40, 110, 220, 30),
            Stroke::new(1.0, Color32::from_rgb(40, 110, 220)),
        )
    }
}

/// Join `ids` (what a drag or click took) to `sel` as `how` says.
pub fn apply(sel: &mut crate::selection::Selection, ids: Vec<ElementId>, how: Combine) {
    match how {
        Combine::New => sel.extend(ids, false),
        Combine::Add => sel.extend(ids, true),
        Combine::Subtract => sel.retain(|id| !ids.contains(&id)),
        Combine::Invert => {
            for id in ids {
                sel.toggle(id);
            }
        }
        Combine::Intersect => sel.retain(|id| ids.contains(&id)),
    }
}

/// The count of what a drag takes, at the pointer (MU39).
pub fn draw_count(painter: &egui::Painter, now: Pos2, n: usize) {
    let text = if n == 1 {
        "1 selected".to_string()
    } else {
        format!("{n} selected")
    };
    // in the theme's panel and text, ringed in the selection colour, so it
    // reads on any paper (MU39: grey on grey before)
    let p = crate::theme::Palette::of(painter.ctx());
    let ink = crate::theme::interaction();
    let g = painter.layout_no_wrap(text, egui::FontId::proportional(12.5), p.text);
    let r = Rect::from_min_size(
        now + egui::vec2(16.0, 14.0),
        g.size() + egui::vec2(24.0, 8.0),
    );
    painter.rect(
        r.translate(egui::vec2(0.0, 2.0)),
        9.0,
        Color32::from_black_alpha(60),
        Stroke::NONE,
        egui::StrokeKind::Middle,
    );
    painter.rect(
        r,
        9.0,
        p.panel,
        Stroke::new(1.5, ink),
        egui::StrokeKind::Middle,
    );
    painter.circle_filled(egui::pos2(r.left() + 9.0, r.center().y), 3.0, ink);
    painter.galley(r.min + egui::vec2(16.0, 4.0), g, p.text);
}

/// Draw the box (solid for inside, dashed for touching), the circle or
/// the lasso.
pub fn draw(painter: &egui::Painter, start: Pos2, now: Pos2) {
    let (fill, line) = look(start, now);
    let dashed = crossing(start, now);
    match shape() {
        Shape::Circle => {
            let r = start.distance(now);
            painter.circle_filled(start, r, fill);
            if dashed {
                let pts: Vec<Pos2> = (0..=64)
                    .map(|k| {
                        let t = k as f32 / 64.0 * std::f32::consts::TAU;
                        start + egui::vec2(t.cos(), t.sin()) * r
                    })
                    .collect();
                painter.extend(egui::Shape::dashed_line(&pts, line, 6.0, 4.0));
            } else {
                painter.circle_stroke(start, r, line);
            }
            return;
        }
        Shape::Lasso => {
            let mut l = lasso();
            if l.len() >= 3 {
                painter.add(egui::Shape::Path(egui::epaint::PathShape {
                    points: l.clone(),
                    closed: true,
                    fill,
                    stroke: egui::Stroke::NONE.into(),
                }));
            }
            if let Some(f) = l.first().copied() {
                l.push(f);
            }
            if dashed {
                painter.extend(egui::Shape::dashed_line(&l, line, 6.0, 4.0));
            } else {
                painter.add(egui::Shape::line(l, line));
            }
            return;
        }
        Shape::Box => {}
    }
    let r = Rect::from_two_pos(start, now);
    let (fill, stroke) = if crossing(start, now) {
        (
            Color32::from_rgba_unmultiplied(60, 170, 90, 30),
            Stroke::new(1.0, Color32::from_rgb(40, 140, 70)),
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(40, 110, 220, 30),
            Stroke::new(1.0, Color32::from_rgb(40, 110, 220)),
        )
    };
    painter.rect_filled(r, 0.0, fill);
    if crossing(start, now) {
        let c = [
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
            r.left_top(),
        ];
        painter.extend(egui::Shape::dashed_line(&c, stroke, 6.0, 4.0));
    } else {
        painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Middle);
    }
}

/// Does a drag from `start` to `now` take a shape drawn through `pts`
/// (screen)? Crossing, any of it touched; else all of it inside. The lines
/// between the points count too, so a long line crossing the box is
/// touched though both its ends lie outside.
pub fn takes(start: Pos2, now: Pos2, pts: &[Pos2], crossing: bool) -> bool {
    if pts.is_empty() {
        return false;
    }
    let mut dense = vec![pts[0]];
    for w in pts.windows(2) {
        let n = (w[0].distance(w[1]) / 4.0).ceil().clamp(1.0, 2000.0) as usize;
        dense.extend((1..=n).map(|k| w[0].lerp(w[1], k as f32 / n as f32)));
    }
    if crossing {
        dense.iter().any(|p| covers(start, now, *p))
    } else {
        dense.iter().all(|p| covers(start, now, *p))
    }
}

/// The ids a box from `start` to `now` takes, given each element's parts
/// as screen rectangles (an element may have several).
pub fn pick(
    start: Pos2,
    now: Pos2,
    parts: impl IntoIterator<Item = (ElementId, Rect)>,
) -> Vec<ElementId> {
    let r = Rect::from_two_pos(start, now);
    let touch = crossing(start, now);
    // per element: (any part touched, every part inside)
    let mut seen: Vec<ElementId> = Vec::new();
    let mut state: HashMap<ElementId, (bool, bool)> = HashMap::new();
    for (id, p) in parts {
        if id == ElementId::NIL {
            continue;
        }
        let s = state.entry(id).or_insert_with(|| {
            seen.push(id);
            (false, true)
        });
        if shape() == Shape::Box {
            s.0 |= r.intersects(p);
            s.1 &= r.contains_rect(p);
        } else {
            let pts = [
                p.center(),
                p.left_top(),
                p.right_top(),
                p.right_bottom(),
                p.left_bottom(),
            ];
            s.0 |= pts.iter().any(|q| covers(start, now, *q));
            s.1 &= pts[1..].iter().all(|q| covers(start, now, *q));
        }
    }
    seen.into_iter()
        .filter(|id| {
            let (any, all) = state[id];
            if touch {
                any
            } else {
                all
            }
        })
        .collect()
}

/// The ids a box takes in 3D, given each element's triangles on screen.
/// Testing triangles rather than bounding boxes keeps a crossing box from
/// picking a wall it only passes near.
pub fn pick_triangles(
    start: Pos2,
    now: Pos2,
    tris: impl IntoIterator<Item = (ElementId, [Pos2; 3])>,
) -> Vec<ElementId> {
    let r = Rect::from_two_pos(start, now);
    let touch = crossing(start, now);
    let mut seen: Vec<ElementId> = Vec::new();
    let mut state: HashMap<ElementId, (bool, bool)> = HashMap::new();
    for (id, t) in tris {
        if id == ElementId::NIL {
            continue;
        }
        let s = state.entry(id).or_insert_with(|| {
            seen.push(id);
            (false, true)
        });
        if shape() != Shape::Box {
            if !s.0 {
                let c = Pos2::new(
                    (t[0].x + t[1].x + t[2].x) / 3.0,
                    (t[0].y + t[1].y + t[2].y) / 3.0,
                );
                s.0 = t.iter().chain([&c]).any(|q| covers(start, now, *q));
            }
            s.1 &= t.iter().all(|q| covers(start, now, *q));
            continue;
        }
        if !s.0 {
            s.0 = touches(r, t);
        }
        s.1 &= t.iter().all(|p| r.contains(*p));
    }
    seen.into_iter()
        .filter(|id| {
            let (any, all) = state[id];
            if touch {
                any
            } else {
                all
            }
        })
        .collect()
}

/// Does triangle `t` overlap rectangle `r`?
fn touches(r: Rect, t: [Pos2; 3]) -> bool {
    let tb = Rect::from_points(&t);
    if !r.intersects(tb) {
        return false;
    }
    if t.iter().any(|p| r.contains(*p)) {
        return true;
    }
    let corners = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    ];
    let cross = |a: Pos2, b: Pos2, p: Pos2| (b - a).x * (p - a).y - (b - a).y * (p - a).x;
    let inside = |p: Pos2| {
        let d = [
            cross(t[0], t[1], p),
            cross(t[1], t[2], p),
            cross(t[2], t[0], p),
        ];
        d.iter().all(|v| *v >= 0.0) || d.iter().all(|v| *v <= 0.0)
    };
    if corners.iter().any(|c| inside(*c)) {
        return true;
    }
    // an edge of the triangle crossing an edge of the rectangle
    let hit = |a: Pos2, b: Pos2, c: Pos2, d: Pos2| {
        let (d1, d2) = (cross(c, d, a), cross(c, d, b));
        let (d3, d4) = (cross(a, b, c), cross(a, b, d));
        d1 * d2 < 0.0 && d3 * d4 < 0.0
    };
    (0..3).any(|i| {
        let (a, b) = (t[i], t[(i + 1) % 3]);
        (0..4).any(|k| hit(a, b, corners[k], corners[(k + 1) % 4]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn left_to_right_takes_whole_elements_right_to_left_takes_touched_ones() {
        let (a, b) = (ElementId::new(), ElementId::new());
        let parts = vec![
            (a, Rect::from_min_max(pos2(10.0, 10.0), pos2(20.0, 20.0))),
            (b, Rect::from_min_max(pos2(15.0, 15.0), pos2(60.0, 20.0))),
        ];
        let window = pick(pos2(0.0, 0.0), pos2(30.0, 30.0), parts.clone());
        assert_eq!(window, vec![a]);
        let cross = pick(pos2(30.0, 30.0), pos2(0.0, 0.0), parts);
        assert_eq!(cross, vec![a, b]);
    }

    #[test]
    fn a_crossing_box_in_the_corner_of_a_triangles_bounds_misses_it() {
        let a = ElementId::new();
        // a thin diagonal triangle from (0,0) to (100,100)
        let t = [pos2(0.0, 0.0), pos2(100.0, 100.0), pos2(0.0, 5.0)];
        assert!(pick_triangles(pos2(100.0, 0.0), pos2(80.0, 20.0), [(a, t)]).is_empty());
        assert_eq!(
            pick_triangles(pos2(60.0, 40.0), pos2(40.0, 60.0), [(a, t)]),
            vec![a]
        );
    }
}
