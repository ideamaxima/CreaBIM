//! The Measure tool (always beside Select and Align): two clicks measure
//! the distance between two points, with the snaps of drawing, in a plan,
//! a section or a 3D view. It leaves nothing in the drawing: nothing goes
//! in the file or in Undo. The figures follow the pointer in a pill and
//! stay in the status bar after the second click.

use crate::planview::{PaneKind, PlanState, Xf};
use eframe::egui::{self, Pos2, Stroke, Vec2};
use glam::{DVec2, DVec3};

/// The Measure tool's state: the view its first point is in, and that
/// point in 3D (a plan or section keeps it in `PlanState::pending`).
#[derive(Clone, Debug, Default)]
pub struct MeasureTool {
    pub view: Option<creabim_core::ElementId>,
    pub first_3d: Option<DVec3>,
}

impl MeasureTool {
    pub fn clear(&mut self) {
        self.view = None;
        self.first_3d = None;
    }
}

/// What a measure from `a` to `b` reads: the distance, the run along each
/// axis and the angle in the drawing's plane (degrees, 0 along +x).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub dist: f64,
    pub d: DVec3,
    pub angle: f64,
}

impl Reading {
    pub fn of(a: DVec3, b: DVec3) -> Self {
        let d = b - a;
        Reading {
            dist: d.length(),
            d,
            angle: crate::tracker::angle_of(a.truncate(), b.truncate()),
        }
    }

    /// The figures, one each: in a plan ΔX ΔY, in a section along and
    /// height, in 3D ΔX ΔY ΔZ.
    pub fn parts(&self, kind: PaneKind) -> Vec<String> {
        let l = crate::units::len;
        let ab = |x: f64| l(x.abs());
        let mut v = vec![format!("Distance {}", l(self.dist))];
        match kind {
            PaneKind::Section { .. } => {
                v.push(format!("Along {}", ab(self.d.x)));
                v.push(format!("Height {}", ab(self.d.y)));
                v.push(format!("Angle {:.1}°", self.angle));
            }
            PaneKind::Axonometric => {
                v.push(format!("ΔX {}", ab(self.d.x)));
                v.push(format!("ΔY {}", ab(self.d.y)));
                v.push(format!("ΔZ {}", ab(self.d.z)));
            }
            _ => {
                v.push(format!("ΔX {}", ab(self.d.x)));
                v.push(format!("ΔY {}", ab(self.d.y)));
                v.push(format!("Angle {:.1}°", self.angle));
            }
        }
        v
    }

    /// The figures in a row, for the status bar.
    pub fn text(&self, kind: PaneKind) -> String {
        self.parts(kind).join("   ")
    }

    /// The figures one per line, for the pill by the pointer.
    pub fn lines(&self, kind: PaneKind) -> String {
        self.parts(kind).join("\n")
    }
}

/// Two points closer than this (metres) measure nothing: the second
/// click stays a first point.
const MIN: f64 = 0.0005;

/// A click at `p` in a plan or section: the first point, or the measure
/// to it, shown in the status bar.
pub fn click(state: &mut PlanState, view: creabim_core::ElementId, kind: PaneKind, p: DVec2) {
    let first = state.pending.filter(|_| state.measure.view == Some(view));
    match first {
        Some(a) if a.distance(p) > MIN => {
            state.pending = None;
            state.measure.clear();
            let r = Reading::of(a.extend(0.0), p.extend(0.0));
            state.status = Some(format!("{}. Click for a new measure.", r.text(kind)));
        }
        Some(_) => {}
        None => {
            state.pending = Some(p);
            state.measure.view = Some(view);
            state.status =
                Some("Click the second point. Shift keeps it square, Alt drops the snaps.".into());
        }
    }
}

/// A click at model point `p` in a 3D view.
pub fn click_3d(state: &mut PlanState, view: Option<creabim_core::ElementId>, p: Option<DVec3>) {
    let Some(p) = p else {
        state.status = Some("Click on the model: a corner, an edge or a face.".into());
        return;
    };
    let first = state
        .measure
        .first_3d
        .filter(|_| state.measure.view == view);
    match first {
        Some(a) if a.distance(p) > MIN => {
            state.measure.clear();
            let r = Reading::of(a, p);
            state.status = Some(format!(
                "{}. Click for a new measure.",
                r.text(PaneKind::Axonometric)
            ));
        }
        Some(_) => {}
        None => {
            state.measure.first_3d = Some(p);
            state.measure.view = view;
            state.status = Some("Click the second point. Shift keeps it along an axis.".into());
        }
    }
}

/// The model point taken in 3D from what is under the pointer: the snap,
/// else the face's point; with Shift held to an axis from the first point.
pub fn point_3d(
    state: &PlanState,
    view: Option<creabim_core::ElementId>,
    under: &crate::align3d::Under,
    shift: bool,
) -> Option<DVec3> {
    let p = under
        .snap
        .map(|s| s.p)
        .or_else(|| under.face.as_ref().map(|f| f.face.point))?;
    match state
        .measure
        .first_3d
        .filter(|_| state.measure.view == view)
    {
        Some(a) if shift => Some(square_3d(a, p)),
        _ => Some(p),
    }
}

/// The measure in a 3D view or window, drawn with `project`; a click
/// takes the point. `view` is none for the 3D window.
#[allow(clippy::too_many_arguments)]
pub fn in_3d(
    ui: &egui::Ui,
    response: &egui::Response,
    painter: &egui::Painter,
    project: &dyn Fn(DVec3) -> Option<Pos2>,
    state: &mut PlanState,
    view: Option<creabim_core::ElementId>,
    under: &crate::align3d::Under,
    shift: bool,
) {
    let here = point_3d(state, view, under, shift);
    if let Some(p) = here.and_then(project) {
        painter.circle_stroke(p, 5.0, Stroke::new(1.5, crate::theme::interaction()));
    }
    if let Some(a) = state
        .measure
        .first_3d
        .filter(|_| state.measure.view == view)
    {
        if let Some(sa) = project(a) {
            let b = here.and_then(|b| project(b).map(|sb| (sb, Reading::of(a, b))));
            preview(
                ui,
                painter,
                sa,
                b,
                PaneKind::Axonometric,
                response.hover_pos(),
            );
        }
    }
    if response.clicked() {
        click_3d(state, view, here);
    }
}

/// In 3D with Shift: `p` held to the axis from `a` it runs most along.
pub fn square_3d(a: DVec3, p: DVec3) -> DVec3 {
    let d = p - a;
    let (x, y, z) = (d.x.abs(), d.y.abs(), d.z.abs());
    if x >= y && x >= z {
        DVec3::new(p.x, a.y, a.z)
    } else if y >= z {
        DVec3::new(a.x, p.y, a.z)
    } else {
        DVec3::new(a.x, a.y, p.z)
    }
}

/// The measure in progress: a dashed line from the first point to the
/// pointer, dots on both ends and the figures in a pill by the pointer.
pub fn preview(
    ui: &egui::Ui,
    painter: &egui::Painter,
    a: Pos2,
    b: Option<(Pos2, Reading)>,
    kind: PaneKind,
    at: Option<Pos2>,
) {
    let ink = crate::theme::interaction();
    painter.circle_filled(a, 4.0, ink);
    let Some((b, r)) = b else {
        return;
    };
    painter.extend(egui::Shape::dashed_line(
        &[a, b],
        Stroke::new(1.5, ink),
        6.0,
        4.0,
    ));
    painter.circle_filled(b, 3.0, ink);
    if let Some(at) = at {
        let look = crate::handles::PillLook::of(&crate::theme::Palette::of(ui.ctx()));
        crate::handles::plain_pill(painter, at + Vec2::new(16.0, 18.0), &r.lines(kind), &look);
    }
}

/// The 2D preview from the state: a plan or section's first point to
/// `hover`.
#[allow(clippy::too_many_arguments)]
pub fn preview_2d(
    ui: &egui::Ui,
    painter: &egui::Painter,
    xf: &Xf,
    state: &PlanState,
    view: creabim_core::ElementId,
    kind: PaneKind,
    hover: Option<DVec2>,
    at: Option<Pos2>,
) {
    let Some(a) = state.pending.filter(|_| state.measure.view == Some(view)) else {
        return;
    };
    let b = hover
        .filter(|b| b.distance(a) > MIN)
        .map(|b| (xf.to_screen(b), Reading::of(a.extend(0.0), b.extend(0.0))));
    preview(ui, painter, xf.to_screen(a), b, kind, at);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reading_gives_the_distance_the_runs_and_the_angle() {
        let r = Reading::of(DVec3::ZERO, DVec3::new(3.0, 4.0, 0.0));
        assert!((r.dist - 5.0).abs() < 1e-9);
        assert!((r.d.x - 3.0).abs() < 1e-9 && (r.d.y - 4.0).abs() < 1e-9);
        assert!((r.angle - 53.13).abs() < 0.01);
        let r = Reading::of(DVec3::ZERO, DVec3::new(1.0, 2.0, 2.0));
        assert!((r.dist - 3.0).abs() < 1e-9 && (r.d.z - 2.0).abs() < 1e-9);
    }

    #[test]
    fn two_clicks_measure_and_start_again() {
        let mut state = PlanState::default();
        let view = creabim_core::ElementId::new();
        let kind = PaneKind::Plan { level: view };
        click(&mut state, view, kind, DVec2::ZERO);
        assert_eq!(state.pending, Some(DVec2::ZERO));
        click(&mut state, view, kind, DVec2::new(2.0, 0.0));
        assert!(state.pending.is_none() && state.measure.view.is_none());
        assert!(state.status.as_deref().unwrap_or("").contains("Distance"));
    }

    #[test]
    fn three_d_measures_and_shift_holds_an_axis() {
        let mut state = PlanState::default();
        let view = creabim_core::ElementId::new();
        click_3d(&mut state, Some(view), Some(DVec3::ZERO));
        assert!(state.has_input());
        click_3d(&mut state, Some(view), Some(DVec3::new(0.0, 0.0, 3.0)));
        assert!(!state.has_input());
        assert!(state.status.as_deref().unwrap_or("").contains("ΔZ"));
        let p = square_3d(DVec3::ZERO, DVec3::new(0.2, 5.0, 0.1));
        assert_eq!(p, DVec3::new(0.0, 5.0, 0.0));
    }
}
