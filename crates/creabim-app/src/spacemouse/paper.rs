//! SpaceMouse navigation on a paper canvas, in paper millimetres.

use eframe::egui;
use glam::DVec2;

/// Pan and zoom the hovered paper, or fit it with the left device button.
pub fn navigate(
    ui: &egui::Ui,
    hovered: bool,
    fit: impl FnOnce() -> (DVec2, f32),
    center: &mut DVec2,
    scale: &mut f32,
) {
    if !hovered {
        return;
    }
    if super::take_press(0) {
        (*center, *scale) = fit();
    }
    let m = super::current();
    if m.pan == [0.0; 2] && m.zoom == 0.0 {
        return;
    }
    let dt = ui.input(|i| i.stable_dt).min(0.05);
    let px = 800.0 * dt / *scale;
    *center -= DVec2::new(m.pan[0] as f64, m.pan[1] as f64) * px as f64;
    *scale = (*scale * (1.0 + m.zoom * 2.0 * dt)).clamp(0.2, 200.0);
    ui.ctx().request_repaint();
}
