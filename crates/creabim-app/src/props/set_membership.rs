//! Set membership shown alongside the selected element's name.

use creabim_core::{sets, Element, Model};
use eframe::egui;

pub fn show(ui: &mut egui::Ui, model: &Model, element: &Element) {
    let mut paths: Vec<_> = element
        .sets
        .iter()
        .map(|id| sets::path(model, *id))
        .collect();
    paths.sort_by_cached_key(|path| path.to_lowercase());
    let text = if paths.is_empty() {
        "No set".to_owned()
    } else {
        paths.join("\n")
    };
    super::head_row(ui, "Sets", |ui| {
        ui.add(egui::Label::new(text).wrap());
    });
}
