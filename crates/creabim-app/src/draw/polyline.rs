//! A drawn polyline in the app (a Rectangle's shape): its fields in
//! Properties — corners, area and perimeter, pen, outline and fill — and
//! how the working view draws its fill.

use crate::planview::Xf;
use creabim_core::draw::line::LineLook;
use creabim_core::draw::DraftPolyline;
use creabim_core::Terminator;
use eframe::egui::{self, Color32, Stroke};
use glam::DVec2;

/// A selected polyline's fields in Properties; returns the edit's name
/// when one changed. `patterns` are the fill patterns it can take.
pub fn fields(ui: &mut egui::Ui, l: &mut DraftPolyline, patterns: &[String]) -> &'static str {
    use crate::app::stylesui::colour_pick;
    let mut label = "";
    crate::props::row(ui, "Corners", |ui| ui.label(l.points.len().to_string()));
    if l.points.len() > 2 {
        crate::props::row(ui, "Closed", |ui| {
            if ui
                .checkbox(&mut l.closed, "")
                .on_hover_text("The last corner joins the first: it has an inside to fill")
                .changed()
            {
                label = "Polyline closed";
            }
        });
    }
    // the area is a reading of the shape: there always, off when open
    crate::props::row(ui, "Area", |ui| {
        ui.add_enabled(
            l.area().is_some(),
            egui::Label::new(l.area().map_or("—".into(), crate::units::area)),
        )
    });
    let name = if l.closed { "Perimeter" } else { "Length" };
    crate::props::row(ui, name, |ui| ui.label(crate::units::len(l.length())));

    let mut look = LineLook {
        weight: l.weight,
        pattern: l.pattern.clone(),
        colour: l.colour,
        start: Terminator::None,
        end: Terminator::None,
    };
    if crate::draw::line::look_fields(ui, "polyline", &mut look, false) {
        l.weight = look.weight;
        l.pattern = look.pattern;
        l.colour = look.colour;
        label = "Polyline look";
    }
    crate::props::row(ui, "Outline", |ui| {
        if ui
            .checkbox(&mut l.outline, "")
            .on_hover_text("Its sides are drawn; off leaves the fill alone")
            .changed()
        {
            label = "Polyline outline";
        }
    });

    // the fill: only a closed shape has one; off it is remembered
    ui.add_enabled_ui(l.closed && l.points.len() > 2, |ui| {
        let f = &mut l.fill;
        crate::props::row(ui, "Fill", |ui| {
            if ui
                .checkbox(&mut f.on, "")
                .on_hover_text("Fill its inside with a colour, a hatch pattern or both")
                .changed()
            {
                label = "Fill on";
            }
        });
        ui.add_enabled_ui(f.on, |ui| {
            crate::props::row(ui, "Colour", |ui| {
                if colour_pick(
                    ui,
                    &mut f.colour,
                    true,
                    "The colour behind; off leaves it clear",
                ) {
                    label = "Fill colour";
                }
            });
            crate::props::row(ui, "Hatch", |ui| {
                egui::ComboBox::from_id_salt("polyline.fill.pattern")
                    .icon(crate::props::combo_icon)
                    .selected_text(f.pattern.as_deref().unwrap_or("None"))
                    .show_ui(ui, |ui| {
                        if ui.selectable_value(&mut f.pattern, None, "None").changed() {
                            label = "Fill hatch";
                        }
                        for n in patterns {
                            let pick = Some(n.clone());
                            if ui.selectable_value(&mut f.pattern, pick, n).changed() {
                                label = "Fill hatch";
                            }
                        }
                    })
                    .response
                    .on_hover_text("A pattern of Styles › Patterns, or a built-in one");
            });
            ui.add_enabled_ui(f.pattern.is_some(), |ui| {
                crate::props::row(ui, "Hatch colour", |ui| {
                    if colour_pick(
                        ui,
                        &mut f.pattern_colour,
                        false,
                        "Its own colour for the hatch lines; off follows the drawing's ink",
                    ) {
                        label = "Fill hatch colour";
                    }
                });
                crate::props::row(ui, "Rotation", |ui| {
                    let d = egui::DragValue::new(&mut f.rotation)
                        .speed(0.5)
                        .range(-360.0..=360.0)
                        .max_decimals(1)
                        .suffix("°");
                    if ui.add(d).changed() {
                        label = "Fill rotation";
                    }
                });
                crate::props::row(ui, "Scale", |ui| {
                    let d = egui::DragValue::new(&mut f.scale)
                        .speed(0.02)
                        .range(0.05..=100.0)
                        .max_decimals(2)
                        .prefix("× ");
                    if ui.add(d).changed() {
                        label = "Fill scale";
                    }
                });
            });
            crate::props::row(ui, "Transparency", |ui| {
                let mut pct = f.transparency * 100.0;
                let d = egui::DragValue::new(&mut pct)
                    .speed(1.0)
                    .range(0.0..=100.0)
                    .max_decimals(0)
                    .suffix(" %");
                if ui.add(d).changed() {
                    f.transparency = pct / 100.0;
                    label = "Fill transparency";
                }
            });
        });
    });
    ui.label(
        egui::RichText::new(
            "Drawn shapes show in this view only, never in 3D, schedules or IFC. Tab edits their corners.",
        )
        .weak(),
    );
    label
}

/// Draw a shape's fill in the working view: its colour at its
/// transparency, then its hatch lines; none when it is selected but the
/// highlight.
#[allow(clippy::too_many_arguments)]
pub fn draw_fill(
    painter: &egui::Painter,
    xf: &Xf,
    points: &[DVec2],
    rgb: Option<[f32; 3]>,
    alpha: f32,
    lines: &[(DVec2, DVec2)],
    ink: Option<[f32; 3]>,
    default_ink: Color32,
    selected: bool,
) {
    let c32 = |c: [f32; 3], a: f32| {
        let [r, g, b] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
        Color32::from_rgba_unmultiplied(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
    };
    if let Some(c) = rgb {
        crate::planview::fill_polygon(painter, xf, points, c32(c, alpha));
    } else if selected {
        crate::planview::fill_polygon(painter, xf, points, c32([1.0, 0.8, 0.6], 0.25));
    }
    let colour = ink.map_or(default_ink, |c| c32(c, 1.0));
    let stroke = Stroke::new(0.6, colour);
    for (a, b) in lines {
        painter.line_segment([xf.to_screen(*a), xf.to_screen(*b)], stroke);
    }
}
