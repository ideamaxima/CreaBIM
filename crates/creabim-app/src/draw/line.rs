//! The Line tool (Draw tab): two clicks draw a line in the view clicked,
//! shown in that view only. The tracker types its length and angle,
//! Shift keeps it square, Alt drops the snaps, as for every line tool.
//! What the next line is drawn with is in the top bar; a selected line's
//! points, pen, pattern, colour and end marks are in Properties.

use crate::planview::{PlanState, Xf};
use crate::selection::Selection;
use creabim_core::draw::line::LineLook;
use creabim_core::draw::DraftLine;
use creabim_core::styles::{LinePattern, PenName, Weight};
use creabim_core::{
    Document, Element, ElementId, ElementKind, ModelError, Terminator, Transaction,
};
use eframe::egui;
use glam::DVec2;

/// The Line tool's state: the view its first point is in, and how the
/// next line is drawn.
#[derive(Clone, Debug, Default)]
pub struct LineTool {
    pub view: Option<ElementId>,
    pub look: LineLook,
}

/// Two points closer than this (metres) do not make a line: the second
/// click stays a first point.
const MIN: f64 = 0.001;

/// A click with the Line tool at `p` in `view`: the first point, or the
/// line to it from the first. The tool stays on for the next line.
pub fn click(
    doc: &mut Document,
    view: ElementId,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let first = state.pending.filter(|_| state.line.view == Some(view));
    match first {
        Some(a) if a.distance(p) > MIN => {
            state.pending = None;
            state.line.view = None;
            let l = DraftLine::new(view, a, p, &state.line.look);
            let e = Element::new("Line", ElementKind::DraftLine(l));
            selection.only(e.id);
            state.status = Some(format!(
                "Line drawn, {} long. Click for the next one; its pen and look are in Properties.",
                crate::units::len(a.distance(p))
            ));
            doc.commit(Transaction::new("Draw line").insert(e))
        }
        Some(_) => Ok(()),
        None => {
            state.pending = Some(p);
            state.line.view = Some(view);
            state.status = Some(
                "Click the line's end, or type its length (Tab: angle) and press Enter.".into(),
            );
            Ok(())
        }
    }
}

/// The line being drawn, from its first point to `hover`, and a dot on
/// that first point.
pub fn preview(
    painter: &egui::Painter,
    xf: &Xf,
    state: &PlanState,
    view: ElementId,
    hover: Option<DVec2>,
    mm: f64,
) {
    let Some(a) = state.pending.filter(|_| state.line.view == Some(view)) else {
        return;
    };
    let ink = crate::theme::interaction();
    painter.circle_filled(xf.to_screen(a), 4.5, ink);
    let Some(b) = hover.filter(|b| b.distance(a) > MIN) else {
        return;
    };
    let l = DraftLine::new(view, a, b, &state.line.look);
    for it in creabim_geom::draw::line::line_items(ElementId::new(), &l, mm) {
        crate::planview::draw_item(painter, xf, &it, true, mm);
    }
}

/// How the working view draws line `l` with its own pen, pattern and
/// colour, as a view's override for it would.
pub fn own_look(l: &DraftLine, w: &creabim_core::LineWeights) -> crate::planview::Ovr {
    own_look_of(l.weight, &l.pattern, l.colour, w)
}

/// The same from a drawn shape's pen, pattern and colour.
pub fn own_look_of(
    weight: Weight,
    pattern: &LinePattern,
    colour: Option<creabim_core::styles::Rgb>,
    w: &creabim_core::LineWeights,
) -> crate::planview::Ovr {
    let f = (weight.mm(w) / w.projection.max(1e-6)) as f32;
    crate::planview::Ovr {
        colour: colour.map(|c| {
            let [r, g, b] = c.0.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
            egui::Color32::from_rgb(r, g, b)
        }),
        proj: f.clamp(0.2, 6.0),
        cut: f.clamp(0.2, 6.0),
        dashes: crate::linepattern::Dashes::from_pattern(pattern),
        own_colour: colour.is_some(),
        ..Default::default()
    }
}

/// A selected line's fields in Properties; returns the edit's name when
/// one changed.
pub fn line_fields(ui: &mut egui::Ui, l: &mut DraftLine) -> &'static str {
    let mut label = "";
    let point = |ui: &mut egui::Ui, name: &str, p: &mut DVec2| {
        let mut changed = false;
        crate::props::flow(ui, |ui| {
            ui.label(name);
            changed |= ui.add(crate::units::drag(&mut p.x).prefix("X ")).changed();
            changed |= ui.add(crate::units::drag(&mut p.y).prefix("Y ")).changed();
        });
        changed
    };
    if point(ui, "Start", &mut l.a) {
        label = "Line start";
    }
    if point(ui, "End", &mut l.b) {
        label = "Line end";
    }
    let mut len = l.length();
    let mut ang = l.angle();
    crate::props::flow(ui, |ui| {
        ui.label("Length");
        if ui
            .add(crate::units::drag(&mut len).range(0.001..=1.0e5))
            .changed()
        {
            l.set_length(len);
            label = "Line length";
        }
        ui.label("Angle");
        if ui
            .add(
                egui::DragValue::new(&mut ang)
                    .speed(0.5)
                    .range(-360.0..=360.0)
                    .max_decimals(2)
                    .suffix("°"),
            )
            .changed()
        {
            l.set_angle(ang);
            label = "Line angle";
        }
    });
    let mut look = LineLook {
        weight: l.weight,
        pattern: l.pattern.clone(),
        colour: l.colour,
        start: l.start,
        end: l.end,
    };
    if look_fields(ui, "line", &mut look, true) {
        l.weight = look.weight;
        l.pattern = look.pattern;
        l.colour = look.colour;
        l.start = look.start;
        l.end = look.end;
        label = "Line look";
    }
    ui.label(
        egui::RichText::new(
            "Lines are drawing: they show in this view only, never in 3D, schedules or IFC.",
        )
        .weak(),
    );
    label
}

/// Pen, pattern, colour and end marks, for a selected line or the next
/// one. True when one changed.
pub fn look_fields(ui: &mut egui::Ui, salt: &str, look: &mut LineLook, ends: bool) -> bool {
    use crate::app::stylesui::{colour_pick, pattern_combo, weight_combo};
    let mut changed = false;
    // the pickers offer "As drawn": for a line that is its usual pen and
    // a solid line
    let mut w = Some(look.weight);
    crate::props::row(ui, "Pen", |ui| {
        if weight_combo(ui, format!("{salt}.weight"), &mut w) {
            look.weight = w.unwrap_or(Weight::Pen(PenName::Projection));
            changed = true;
        }
    });
    let mut p = Some(look.pattern.clone());
    crate::props::row(ui, "Pattern", |ui| {
        if pattern_combo(ui, format!("{salt}.pattern"), &mut p) {
            look.pattern = p.clone().unwrap_or(LinePattern::Solid);
            changed = true;
        }
    });
    crate::props::row(ui, "Colour", |ui| {
        changed |= colour_pick(
            ui,
            &mut look.colour,
            false,
            "Its own colour; off follows the drawing's ink",
        );
    });
    // a closed shape has no ends to mark
    if !ends {
        return changed;
    }
    for (text, t, id) in [
        ("Start", &mut look.start, "start"),
        ("End", &mut look.end, "end"),
    ] {
        let mut o = Some(*t);
        if crate::annotate::terminator_combo(ui, &format!("{salt}.{id}"), text, "None", &mut o) {
            *t = o.unwrap_or(Terminator::None);
            changed = true;
        }
    }
    if ui
        .button("⇄ Swap ends")
        .on_hover_text("The start's mark goes to the end, and the end's to the start")
        .clicked()
    {
        std::mem::swap(&mut look.start, &mut look.end);
        changed = true;
    }
    changed
}

// ---- the top bar: how the next line is drawn ----

/// Fixed pen widths the top bar offers, paper mm, after the view's pens.
const WIDTHS: [f64; 6] = [0.13, 0.18, 0.25, 0.35, 0.5, 0.7];

/// Colours the top bar offers; Properties picks any.
const COLOURS: [(&str, Option<[f32; 3]>); 6] = [
    ("Ink", None),
    ("Red", Some([0.85, 0.15, 0.15])),
    ("Blue", Some([0.15, 0.35, 0.85])),
    ("Green", Some([0.1, 0.6, 0.25])),
    ("Orange", Some([0.95, 0.55, 0.1])),
    ("Grey", Some([0.5, 0.5, 0.5])),
];

fn pens() -> Vec<Weight> {
    PenName::ALL
        .into_iter()
        .map(Weight::Pen)
        .chain(WIDTHS.into_iter().map(Weight::Mm))
        .collect()
}

fn colour_name(c: Option<creabim_core::styles::Rgb>) -> String {
    COLOURS
        .iter()
        .find(|(_, v)| v.map(creabim_core::styles::Rgb) == c)
        .map_or_else(|| "Custom".into(), |(n, _)| n.to_string())
}

/// The Line tool's top bar entries.
pub fn options(look: &LineLook) -> Vec<serde_json::Value> {
    use serde_json::json;
    let menu = |key: &str, label: &str, value: String, hint: &str| json!({"kind": "menu", "key": key, "label": label, "value": value, "hint": hint});
    vec![
        menu(
            "line.pen",
            "Pen",
            look.weight.label(),
            "The next line's pen: one of the view's, or a fixed width",
        ),
        menu(
            "line.pattern",
            "Pattern",
            look.pattern.label().into(),
            "Solid, dashed, dotted…",
        ),
        menu(
            "line.colour",
            "Colour",
            colour_name(look.colour),
            "Ink follows the drawing; Properties picks any colour",
        ),
        menu(
            "line.start",
            "Start",
            look.start.label().into(),
            "The mark at the line's first point",
        ),
        menu(
            "line.end",
            "End",
            look.end.label().into(),
            "The mark at the line's second point",
        ),
    ]
}

/// The items of the Line tool's top bar menus.
pub fn menu(key: &str, look: &LineLook) -> Option<Vec<crate::ui_model::Item>> {
    use crate::ui_model::Item;
    let item =
        |i: usize, label: String, on: bool| Item::new(format!("{key}:{i}"), label).checked(on);
    Some(match key {
        "line.pen" => pens()
            .into_iter()
            .enumerate()
            .map(|(i, w)| item(i, w.label(), w == look.weight))
            .collect(),
        "line.pattern" => LinePattern::NAMED
            .iter()
            .enumerate()
            .map(|(i, p)| item(i, p.label().into(), *p == look.pattern))
            .collect(),
        "line.colour" => COLOURS
            .iter()
            .enumerate()
            .map(|(i, (n, v))| {
                item(
                    i,
                    n.to_string(),
                    v.map(creabim_core::styles::Rgb) == look.colour,
                )
            })
            .collect(),
        "line.start" | "line.end" => {
            let now = if key == "line.start" {
                look.start
            } else {
                look.end
            };
            Terminator::ALL
                .iter()
                .enumerate()
                .map(|(i, t)| item(i, t.label().into(), *t == now))
                .collect()
        }
        _ => return None,
    })
}

/// Run a pick from those menus; false when `head` is not one.
pub fn action(head: &str, arg: &str, look: &mut LineLook) -> bool {
    let Some(i) = arg.parse::<usize>().ok() else {
        return false;
    };
    match head {
        "line.pen" => {
            if let Some(w) = pens().get(i) {
                look.weight = *w;
            }
        }
        "line.pattern" => {
            if let Some(p) = LinePattern::NAMED.get(i) {
                look.pattern = p.clone();
            }
        }
        "line.colour" => {
            if let Some((_, v)) = COLOURS.get(i) {
                look.colour = v.map(creabim_core::styles::Rgb);
            }
        }
        "line.start" => {
            if let Some(t) = Terminator::ALL.get(i) {
                look.start = *t;
            }
        }
        "line.end" => {
            if let Some(t) = Terminator::ALL.get(i) {
                look.end = *t;
            }
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::View;

    fn doc_with_section() -> (Document, ElementId) {
        let mut doc = Document::new();
        let v = Element::new(
            "Section A",
            ElementKind::View(View::section(DVec2::ZERO, DVec2::new(5.0, 0.0), 3.0, true)),
        );
        let id = v.id;
        doc.commit(Transaction::new("setup").insert(v)).unwrap();
        (doc, id)
    }

    fn lines(doc: &Document) -> Vec<DraftLine> {
        doc.model()
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::DraftLine(l) => Some(l.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn two_clicks_draw_a_line_in_the_view_and_one_undo_takes_it() {
        let (mut doc, section) = doc_with_section();
        let mut state = PlanState::default();
        let mut sel = Selection::default();
        let p = DVec2::new;
        state.line.look.end = Terminator::Arrow;
        click(&mut doc, section, p(1.0, 0.5), &mut state, &mut sel).unwrap();
        assert!(lines(&doc).is_empty());
        // a second click on the first point is not a line
        click(&mut doc, section, p(1.0, 0.5), &mut state, &mut sel).unwrap();
        assert!(lines(&doc).is_empty());
        click(&mut doc, section, p(3.0, 0.5), &mut state, &mut sel).unwrap();
        let ls = lines(&doc);
        assert_eq!(ls.len(), 1);
        assert_eq!(
            (ls[0].view, ls[0].a, ls[0].b),
            (section, p(1.0, 0.5), p(3.0, 0.5))
        );
        assert_eq!(ls[0].end, Terminator::Arrow);
        assert!(state.pending.is_none(), "the next click starts a new line");
        assert!(sel.active.is_some());
        assert!(doc.undo());
        assert!(lines(&doc).is_empty());
    }

    #[test]
    fn a_first_point_in_another_view_starts_over() {
        let (mut doc, a) = doc_with_section();
        let (_, b) = doc_with_section();
        let mut state = PlanState::default();
        let mut sel = Selection::default();
        click(&mut doc, a, DVec2::ZERO, &mut state, &mut sel).unwrap();
        click(&mut doc, b, DVec2::X, &mut state, &mut sel).unwrap();
        assert!(lines(&doc).is_empty());
        assert_eq!(state.line.view, Some(b));
    }

    #[test]
    fn the_top_bar_sets_the_next_line() {
        let mut look = LineLook::default();
        assert!(action("line.pattern", "1", &mut look));
        assert_eq!(look.pattern, LinePattern::Dashed);
        assert!(action("line.end", "2", &mut look));
        assert_eq!(look.end, Terminator::Arrow);
        assert!(action("line.colour", "1", &mut look));
        assert_eq!(colour_name(look.colour), "Red");
        assert!(!action("dim.axis", "1", &mut look));
        let items = menu("line.end", &look).unwrap();
        assert_eq!(items.len(), Terminator::ALL.len());
    }
}
