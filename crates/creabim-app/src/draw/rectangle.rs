//! The Rectangle tool (Draw tab): a closed polyline of four corners in
//! the view clicked, drawn by one of three methods picked in the top bar:
//! two opposite corners, from the centre, or from a side (two clicks for
//! the side, a third for the height). The tracker types its width and
//! height; Shift makes it a square, Alt drops the snaps. It is drawn with
//! the Line tool's pen, pattern and colour, and born empty: its fill is
//! switched on in Properties.

use crate::planview::{PlanState, Xf};
use crate::selection::Selection;
use crate::tracker::Tracker;
use creabim_core::draw::DraftPolyline;
use creabim_core::{Document, Element, ElementId, ElementKind, ModelError, Transaction};
use eframe::egui::{self, Pos2, Stroke};
use glam::DVec2;

/// How the rectangle is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RectMethod {
    /// A corner, then the opposite one.
    #[default]
    Corners,
    /// Its centre, then a corner.
    Centre,
    /// Two clicks for one side (at any angle), a third for its height.
    Side,
}

impl RectMethod {
    pub const ALL: [RectMethod; 3] = [RectMethod::Corners, RectMethod::Centre, RectMethod::Side];

    pub fn label(self) -> &'static str {
        match self {
            RectMethod::Corners => "Two corners",
            RectMethod::Centre => "From centre",
            RectMethod::Side => "From a side",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            RectMethod::Corners => "A corner, then the opposite one",
            RectMethod::Centre => "Its centre, then a corner",
            RectMethod::Side => "Two points for one side, at any angle, then its height",
        }
    }

    fn start(self) -> &'static str {
        match self {
            RectMethod::Corners => "Rectangle: click a corner.",
            RectMethod::Centre => "Rectangle: click its centre.",
            RectMethod::Side => "Rectangle: click where its side starts.",
        }
    }
}

/// The Rectangle tool's state: the view its first point is in, the
/// method, and the side a From a side rectangle has so far.
#[derive(Clone, Debug, Default)]
pub struct RectTool {
    pub view: Option<ElementId>,
    pub method: RectMethod,
    pub side: Option<(DVec2, DVec2)>,
}

impl RectTool {
    pub fn clear(&mut self) {
        self.view = None;
        self.side = None;
    }
}

/// Rectangles smaller than this across (metres) are not made.
const MIN: f64 = 0.001;

fn typed_len(t: &Tracker, i: usize) -> Option<f64> {
    let s = t.fields[i].trim();
    (!s.is_empty())
        .then(|| crate::units::parse(s))
        .flatten()
        .filter(|v| *v > 0.0)
}

fn sign(v: f64) -> f64 {
    if v < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// Where the pointer at `p` puts the rectangle's next point, from its
/// first point `a`: the typed width and height take over from it, and
/// `square` makes the sides equal.
pub fn end(rt: &RectTool, t: &Tracker, a: DVec2, p: DVec2, square: bool) -> DVec2 {
    match (rt.method, rt.side) {
        (RectMethod::Side, None) => t.end(a, p),
        (RectMethod::Side, Some((s0, s1))) => {
            let along = (s1 - s0).normalize_or(DVec2::X);
            let across = along.perp();
            let h = (p - s0).dot(across);
            let len = typed_len(t, 0).unwrap_or(if square { s0.distance(s1) } else { h.abs() });
            s0 + along * (p - s0).dot(along) + across * sign(h) * len
        }
        (m, _) => {
            let d = p - a;
            let half = if m == RectMethod::Centre { 0.5 } else { 1.0 };
            let mut w = typed_len(t, 0).map_or(d.x.abs(), |v| v * half);
            let mut h = typed_len(t, 1).map_or(d.y.abs(), |v| v * half);
            if square {
                let s = w.max(h);
                (w, h) = (s, s);
            }
            a + DVec2::new(sign(d.x) * w, sign(d.y) * h)
        }
    }
}

/// The four corners, anticlockwise, from the first point `a` to `p`
/// (none while a From a side rectangle has no side yet).
pub fn corners(rt: &RectTool, t: &Tracker, a: DVec2, p: DVec2) -> Option<[DVec2; 4]> {
    let quad = match (rt.method, rt.side) {
        (RectMethod::Side, None) => return None,
        (RectMethod::Side, Some((s0, s1))) => {
            let along = (s1 - s0).normalize_or(DVec2::X);
            // a typed width resets the side's length
            let s1 = typed_len(t, 1).map_or(s1, |w| s0 + along * w);
            let h = (p - s0).dot(along.perp());
            let up = along.perp() * h;
            [s0, s1, s1 + up, s0 + up]
        }
        (RectMethod::Centre, _) => {
            let d = p - a;
            [
                a - d,
                a + DVec2::new(d.x, -d.y),
                a + d,
                a + DVec2::new(-d.x, d.y),
            ]
        }
        (RectMethod::Corners, _) => {
            let (lo, hi) = (a.min(p), a.max(p));
            [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)]
        }
    };
    let (w, h) = (quad[0].distance(quad[1]), quad[1].distance(quad[2]));
    if w < MIN || h < MIN {
        return None;
    }
    // anticlockwise, whichever way it was drawn
    let twice: f64 = (0..4).map(|i| quad[i].perp_dot(quad[(i + 1) % 4])).sum();
    Some(if twice < 0.0 {
        [quad[0], quad[3], quad[2], quad[1]]
    } else {
        quad
    })
}

/// A click at `p` (already through [`end`]) in `view`: the first point,
/// the side, or the rectangle. The tool stays on for the next one.
pub fn click(
    doc: &mut Document,
    view: ElementId,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let first = state.pending.filter(|_| state.rect.view == Some(view));
    let Some(a) = first else {
        state.pending = Some(p);
        state.rect.view = Some(view);
        state.rect.side = None;
        state.status = Some(match state.rect.method {
            RectMethod::Corners => "Click the opposite corner, or type its width (Tab: height) and press Enter. Shift makes a square.".into(),
            RectMethod::Centre => "Click a corner, or type its width (Tab: height) and press Enter. Shift makes a square.".into(),
            RectMethod::Side => "Click where the side ends, or type its length (Tab: angle) and press Enter.".into(),
        });
        return Ok(());
    };
    if state.rect.method == RectMethod::Side && state.rect.side.is_none() {
        if a.distance(p) > MIN {
            state.rect.side = Some((a, p));
            state.status =
                Some("Click its height, or type it and press Enter. Shift makes a square.".into());
        }
        return Ok(());
    }
    let Some(quad) = corners(&state.rect, &state.tracker, a, p) else {
        return Ok(());
    };
    state.pending = None;
    state.rect.clear();
    let l = DraftPolyline::new(view, quad.to_vec(), true, &state.line.look);
    let (w, h) = (quad[0].distance(quad[1]), quad[1].distance(quad[2]));
    let e = Element::new("Rectangle", ElementKind::DraftPolyline(l));
    selection.only(e.id);
    state.status = Some(format!(
        "Rectangle drawn, {} × {}. Click for the next one; its fill is in Properties.",
        crate::units::len(w),
        crate::units::len(h)
    ));
    doc.commit(Transaction::new("Draw rectangle").insert(e))
}

/// The rectangle being drawn, to `hover`, and the tracker's pill by the
/// pointer `at` with its width and height.
#[allow(clippy::too_many_arguments)]
pub fn preview(
    painter: &egui::Painter,
    xf: &Xf,
    state: &PlanState,
    view: ElementId,
    hover: Option<DVec2>,
    at: Option<Pos2>,
    step: &str,
    look: &crate::handles::PillLook,
) {
    let Some(a) = state.pending.filter(|_| state.rect.view == Some(view)) else {
        return;
    };
    let ink = crate::theme::interaction();
    painter.circle_filled(xf.to_screen(a), 4.5, ink);
    let Some(b) = hover else {
        return;
    };
    let stroke = Stroke::new(1.5, ink);
    let t = &state.tracker;
    let fields = match (state.rect.method, state.rect.side) {
        (RectMethod::Side, None) => {
            painter.line_segment([xf.to_screen(a), xf.to_screen(b)], stroke);
            [
                ("Length", crate::tracker::bare_len(a.distance(b)), ""),
                (
                    "Angle",
                    format!("{:.1}", crate::tracker::angle_of(a, b)),
                    "°",
                ),
            ]
        }
        _ => {
            let Some(q) = corners(&state.rect, t, a, b) else {
                return;
            };
            let pts: Vec<Pos2> = q.iter().map(|p| xf.to_screen(*p)).collect();
            painter.add(egui::Shape::closed_line(pts, stroke));
            let (w, h) = (q[0].distance(q[1]), q[1].distance(q[2]));
            if state.rect.method == RectMethod::Side {
                [
                    ("Height", crate::tracker::bare_len(h), ""),
                    ("Width", crate::tracker::bare_len(w), ""),
                ]
            } else {
                [
                    ("Width", crate::tracker::bare_len(w), ""),
                    ("Height", crate::tracker::bare_len(h), ""),
                ]
            }
        }
    };
    if let Some(at) = at {
        crate::tracker::draw_fields(painter, at, t, &fields, step, look);
    }
}

/// The method picker for the top bar.
pub fn options(rt: &RectTool) -> serde_json::Value {
    use serde_json::json;
    let items: Vec<serde_json::Value> = RectMethod::ALL
        .iter()
        .enumerate()
        .map(|(i, m)| {
            json!({"id": format!("rect.method:{i}"), "label": m.label(), "on": rt.method == *m, "hint": m.hint()})
        })
        .collect();
    json!({"kind": "seg", "items": items})
}

/// Run a pick of the method; the status to show, or none when `head` is
/// not the method's.
pub fn action(head: &str, arg: &str, state: &mut PlanState) -> Option<String> {
    if head != "rect.method" {
        return None;
    }
    let m = arg
        .parse::<usize>()
        .ok()
        .and_then(|i| RectMethod::ALL.get(i))?;
    state.rect.method = *m;
    state.rect.clear();
    state.pending = None;
    state.tracker.clear();
    Some(m.start().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(method: RectMethod) -> RectTool {
        RectTool {
            method,
            ..Default::default()
        }
    }

    #[test]
    fn two_corners_any_way_round_give_the_same_rectangle() {
        let t = Tracker::default();
        let rt = tool(RectMethod::Corners);
        let q = corners(&rt, &t, DVec2::new(2.0, 3.0), DVec2::ZERO).unwrap();
        assert_eq!(q[0], DVec2::ZERO);
        assert_eq!(q[2], DVec2::new(2.0, 3.0));
    }

    #[test]
    fn from_the_centre_it_spreads_both_ways() {
        let t = Tracker::default();
        let rt = tool(RectMethod::Centre);
        let q = corners(&rt, &t, DVec2::ZERO, DVec2::new(1.0, 2.0)).unwrap();
        assert!(q.contains(&DVec2::new(-1.0, -2.0)) && q.contains(&DVec2::new(1.0, 2.0)));
    }

    #[test]
    fn from_a_side_it_rises_square_to_it() {
        let t = Tracker::default();
        let mut rt = tool(RectMethod::Side);
        rt.side = Some((DVec2::ZERO, DVec2::new(3.0, 3.0)));
        let q = corners(&rt, &t, DVec2::ZERO, DVec2::new(-1.0, 1.0)).unwrap();
        let h = q[1].distance(q[2]).min(q[0].distance(q[1]));
        assert!((h - 2f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn typed_width_and_height_take_over_and_shift_squares_it() {
        crate::units::set(creabim_core::Units::metric());
        let t = Tracker {
            fields: ["2000".into(), "".into()],
            ..Default::default()
        };
        let rt = tool(RectMethod::Corners);
        let p = end(&rt, &t, DVec2::ZERO, DVec2::new(-0.5, 0.7), false);
        assert!((p.x + 2.0).abs() < 1e-9 && (p.y - 0.7).abs() < 1e-9);
        let t = Tracker::default();
        let p = end(&rt, &t, DVec2::ZERO, DVec2::new(1.0, 3.0), true);
        assert_eq!(p, DVec2::new(3.0, 3.0));
    }

    #[test]
    fn a_rectangle_is_one_closed_element_and_one_undo() {
        let mut doc = Document::new();
        let v = Element::new(
            "Section A",
            ElementKind::View(creabim_core::View::section(
                DVec2::ZERO,
                DVec2::new(5.0, 0.0),
                3.0,
                true,
            )),
        );
        let view = v.id;
        doc.commit(Transaction::new("setup").insert(v)).unwrap();
        let mut state = PlanState::default();
        let mut sel = Selection::default();
        click(&mut doc, view, DVec2::ZERO, &mut state, &mut sel).unwrap();
        click(&mut doc, view, DVec2::new(2.0, 1.0), &mut state, &mut sel).unwrap();
        let shapes: Vec<DraftPolyline> = doc
            .model()
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::DraftPolyline(l) => Some(l.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(shapes.len(), 1);
        assert!(shapes[0].closed && shapes[0].points.len() == 4);
        assert!(!shapes[0].fill.on, "born empty");
        assert!((shapes[0].area().unwrap() - 2.0).abs() < 1e-9);
        assert!(state.pending.is_none() && sel.active.is_some());
        assert!(doc.undo());
        assert_eq!(doc.model().iter().count(), 1);
    }
}
