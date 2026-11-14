//! The element action bar (brief 02, 2.7): a row of icon buttons under the
//! element's name in Properties. First the element's own actions, tinted;
//! then the common ones in a fixed order; then More (⋯) and Delete, set
//! apart in red. With several selected, only what applies to all of them
//! shows. Every action is in the right-click menu too.

use super::*;
use crate::commands::Command;
use creabim_core::{walls, OpeningKind};

/// One button of the bar.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Act {
    pub id: String,
    pub label: String,
    pub key: &'static str,
    pub icon: &'static str,
    pub hint: String,
    pub enabled: bool,
    /// Pressed (Edit outline while editing).
    pub on: bool,
}

impl Act {
    fn new(id: impl Into<String>, label: impl Into<String>, icon: &'static str) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            key: "",
            icon,
            hint: String::new(),
            enabled: true,
            on: false,
        }
    }
    fn key(mut self, k: &'static str) -> Self {
        self.key = k;
        self
    }
    fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }
    fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }
    /// The tooltip: its name and key, then what it does.
    pub fn tip(&self) -> String {
        let mut t = self.label.clone();
        if !self.key.is_empty() {
            t += &format!("  ({})", self.key);
        }
        if !self.hint.is_empty() {
            t += &format!("\n{}", self.hint);
        }
        t
    }
}

/// Annotation: drawn in one view, so not isolated.
fn annotation(kind: &ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::Dimension(_)
            | ElementKind::Tag(_)
            | ElementKind::Note(_)
            | ElementKind::Detail(_)
            | ElementKind::DraftLine(_)
            | ElementKind::View(_)
            | ElementKind::Viewport(_)
    )
}

/// The width of a button and the gap after it, in points.
const BUTTON: f32 = 22.0;
const GAP: f32 = 1.0;
/// The room between groups.
const SPLIT: f32 = 3.0;

impl CreaBimApp {
    /// One element's own actions: what this kind of element can do.
    fn own_actions(&self, id: ElementId, alone: bool) -> Vec<Act> {
        let model = self.doc.model();
        let Some(e) = model.get(id) else {
            return vec![];
        };
        let mut out = vec![];
        // Edit mode moved to the view header's mode menu (MU27): no button here
        if false && alone && editmode::editable(model, id) {
            out.push(
                Act {
                    on: self.plan.edit == Some(id),
                    ..Act::new("bar.edit", "Edit outline", "edit-outline")
                }
                .key("Enter")
                .hint("Edit mode: all its grips in every view, corners to add and take away. Esc finishes."),
            );
        }
        // a view not shown in the drawing area opens from here (A4-1)
        if alone
            // (views have their own Open view below)
            && matches!(e.kind, ElementKind::Sheet(_))
            && self.active_view != Some(id)
        {
            out.push(
                Act::new("bar.view.open", "Open this view", "open-view")
                    .hint("Show it in the drawing area"),
            );
        }
        match &e.kind {
            ElementKind::Wall(_) => {
                out.push(
                    Act::new("wall.flip", "Flip", "flip")
                        .key("Space")
                        .hint("Swap its inside and outside faces"),
                );
                let overhead = !walls::attach_candidates(model, id).is_empty();
                out.push(
                    Act::new("wall.attach", "Attach top", "attach-top")
                        .enabled(overhead)
                        .hint(if overhead {
                            "The top follows the underside of the roof or slab above"
                        } else {
                            "There is no roof or slab above this wall"
                        }),
                );
                if alone {
                    out.push(
                        Act::new("bar.maketype", "Make a type", "make-type")
                            .hint("A new type with its values, which it becomes an instance of"),
                    );
                }
            }
            ElementKind::Opening(o) if o.kind == OpeningKind::Door => {
                out.push(
                    Act::new("wall.flip", "Flip", "flip")
                        .key("Space")
                        .hint("Flip the door's swing"),
                );
                out.push(
                    Act::new("wall.hand", "Flip hand", "door-open")
                        .key("Shift+Space")
                        .hint("Hinges on the other side"),
                );
            }
            ElementKind::Opening(o) if o.kind == OpeningKind::Window => {
                out.push(
                    Act::new("wall.flip", "Flip", "flip")
                        .key("Space")
                        .hint("Swap the window's inside and outside"),
                );
            }
            ElementKind::View(v) => {
                if alone {
                    out.push(
                        Act::new(format!("view:{}", id.0), "Open view", "open-view")
                            .hint("Show this view in the 2D pane (double-clicking its mark does the same)"),
                    );
                }
                if matches!(v.kind, ViewKind::Section { .. }) {
                    out.push(
                        Act::new("wall.flip", "Flip", "flip")
                            .key("Space")
                            .hint("Look the other way along the same line"),
                    );
                }
            }
            ElementKind::Tag(t) => {
                out.push(
                    Act::new("bar.tagged", "Select tagged", "select-tagged")
                        .hint("Select the element this tag names"),
                );
                out.push(
                    Act::new("bar.straighten", "Straighten leader", "straighten-leader")
                        .enabled(t.elbow.is_some())
                        .hint(if t.elbow.is_some() {
                            "Take the elbow out of its leader"
                        } else {
                            "Its leader is straight already"
                        }),
                );
            }
            _ => {}
        }
        out
    }

    /// The bar for the selection: its own actions (those every selected
    /// element has), then the common ones. Delete and More are added by
    /// [`Self::action_bar`].
    pub(super) fn element_actions(&self) -> (Vec<Act>, Vec<Act>) {
        let ids = self.selection.all();
        let model = self.doc.model();
        let alone = ids.len() == 1;
        let mut own: Vec<Act> = vec![];
        for (i, id) in ids.iter().enumerate() {
            let mine = self.own_actions(*id, alone);
            if i == 0 {
                own = mine;
            } else {
                own.retain(|a| mine.iter().any(|m| m.id == a.id));
                // one kind's "Flip" can't say what it does for the other
                for a in &mut own {
                    if let Some(m) = mine.iter().find(|m| m.id == a.id) {
                        if m.hint != a.hint {
                            a.hint = "Flip each of them".into();
                        }
                        a.enabled &= m.enabled;
                    }
                }
            }
        }
        let typed = ids
            .iter()
            .filter_map(|id| model.get(*id))
            .any(|e| super::selectsimilar::type_key(model, e).is_some());
        let all_annotation = ids
            .iter()
            .filter_map(|id| model.get(*id))
            .all(|e| annotation(&e.kind));
        let cmd = |c: Command, icon: &'static str, hint: &str| {
            Act::new(format!("bar.cmd:{}", c as usize), c.label(), icon)
                .key(c.shortcut())
                .hint(hint)
        };
        let mut common = vec![
            cmd(
                Command::Duplicate,
                "duplicate",
                "A copy that follows the pointer; click to place it",
            ),
            if typed {
                Act::new("similar:type.view", "Select similar", "select-similar")
                    .key("Shift+A")
                    .hint("Every element of the same type that this view shows")
            } else {
                Act::new("similar:cat.view", "Select similar", "select-similar")
                    .hint("Every element of the same category that this view shows")
            },
            cmd(
                Command::Hide,
                "eye-off",
                "Hide it until Alt+H shows everything again",
            ),
        ];
        if !all_annotation {
            common.push(cmd(
                Command::Isolate,
                "isolate",
                "Hide everything else until Alt+H shows everything again",
            ));
        }
        common.push(
            Act::new("nav.fitsel", "Frame", "frame-selection")
                .key(".")
                .hint("Zoom the 3D view to it"),
        );
        (own, common)
    }

    /// The bar itself, under the element's name.
    pub(super) fn action_bar(&mut self, ui: &mut egui::Ui) {
        let (own, common) = self.element_actions();
        // what doesn't fit goes in More, the common ones first
        let room = ui.available_width();
        // Sets, More and Delete, and the gaps between the groups
        let fixed = 3.0 * BUTTON + 4.0 * GAP + 3.0 * SPLIT;
        // one button's room kept spare, so Delete and More always show at
        // the right however the panel is sized (MU23: an extrusion's
        // actions pushed Delete out of sight)
        let fit = (((room - fixed) / (BUTTON + GAP)).floor().max(1.0) - 1.0) as usize;
        let mut shown_own = own.clone();
        let mut shown_common = common.clone();
        let mut more: Vec<Act> = vec![];
        while shown_own.len() + shown_common.len() > fit {
            match shown_common.pop() {
                Some(a) => more.insert(0, a),
                None => {
                    if let Some(a) = shown_own.pop() {
                        more.insert(0, a);
                    } else {
                        break;
                    }
                }
            }
        }
        let accent = ui.visuals().selection.bg_fill;
        let text = ui.visuals().text_color();
        let danger = egui::Color32::from_rgb(0xd0, 0x45, 0x3a);
        let mut picked: Option<String> = None;
        let mut set_asks = vec![];
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for a in &shown_own {
                if icon_button(ui, a, Some(accent), text) {
                    picked = Some(a.id.clone());
                }
            }
            if !shown_own.is_empty() && !shown_common.is_empty() {
                ui.add_space(SPLIT);
            }
            for a in &shown_common {
                if icon_button(ui, a, None, text) {
                    picked = Some(a.id.clone());
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                let n = self.selection.len();
                let deps: usize = self
                    .selection
                    .all()
                    .iter()
                    .map(|id| self.doc.model().dependents(*id).len())
                    .sum();
                let del = Act::new("bar.cmd:delete", "Delete", "trash-2")
                    .key("Del")
                    .hint(match (n, deps) {
                        (_, 0) => "Delete it".to_string(),
                        (_, d) => format!("Delete it and the {d} attached to it"),
                    });
                if icon_button(ui, &del, None, danger) {
                    picked = Some(del.id.clone());
                }
                ui.add_space(SPLIT);
                let m = Act::new("bar.more", "More", "ellipsis")
                    .hint("Move, rotate, copy and the rest of the right-click menu");
                let r = icon_response(ui, &m, None, text);
                egui::Popup::menu(&r).show(|ui| {
                    ui.set_min_width(190.0);
                    for a in &more {
                        if ui
                            .add_enabled(
                                a.enabled,
                                egui::Button::new(&a.label).shortcut_text(a.key),
                            )
                            .on_hover_text(&a.hint)
                            .clicked()
                        {
                            picked = Some(a.id.clone());
                        }
                    }
                    if !more.is_empty() {
                        ui.separator();
                    }
                    for c in [
                        Command::Move,
                        Command::Rotate,
                        Command::Scale,
                        Command::Copy,
                        Command::Cut,
                    ] {
                        if ui
                            .add(egui::Button::new(c.label()).shortcut_text(c.shortcut()))
                            .clicked()
                        {
                            picked = Some(format!("bar.cmd:{}", c as usize));
                        }
                    }
                    ui.separator();
                    for (id, label) in [
                        ("similar:type.project", "All of this type in the project"),
                        ("similar:cat.project", "All of this category in the project"),
                    ] {
                        if ui.button(label).clicked() {
                            picked = Some(id.into());
                        }
                    }
                });
                // the user's sets, left of More
                ui.add_space(SPLIT);
                let sets = Act::new("bar.sets", "Sets", "sets")
                    .hint("Put it in your sets or take it out, or make a new set from it");
                let r = icon_response(ui, &sets, None, text);
                egui::Popup::menu(&r)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| set_asks = self.sets_menu(ui));
            });
        });
        for a in set_asks {
            self.run_set_ask(a);
        }
        if let Some(id) = picked {
            let ctx = ui.ctx().clone();
            self.run_bar_action(&ctx, &id);
        }
    }

    /// Run a button of the bar.
    pub(super) fn run_bar_action(&mut self, ctx: &egui::Context, id: &str) {
        let model = self.doc.model();
        match id {
            "bar.edit" => self.toggle_edit_mode(),
            "bar.cmd:delete" => self.delete_selected(),
            "bar.view.open" => {
                if let Some(id) = self.selection.active {
                    self.active_view = Some(id);
                }
            }
            "bar.maketype" => {
                if let Some(id) = self.selection.active {
                    self.run_types(super::typesui::TypeAsk::FromElement(id));
                }
            }
            "bar.tagged" => {
                let hosts: Vec<ElementId> = self
                    .selection
                    .all()
                    .iter()
                    .filter_map(|id| match &model.get(*id)?.kind {
                        ElementKind::Tag(t) => Some(t.host),
                        _ => None,
                    })
                    .filter(|h| model.get(*h).is_some())
                    .collect();
                if hosts.is_empty() {
                    self.status = "What it tags is gone.".into();
                } else {
                    let n = hosts.len();
                    self.selection.extend(hosts, false);
                    self.status = match n {
                        1 => "Selected what it tags.".into(),
                        n => format!("Selected the {n} elements they tag."),
                    };
                }
            }
            "bar.straighten" => {
                let tags: Vec<ElementId> = self
                    .selection
                    .all()
                    .into_iter()
                    .filter(|id| {
                        matches!(model.get(*id).map(|e| &e.kind), Some(ElementKind::Tag(t)) if t.elbow.is_some())
                    })
                    .collect();
                let mut tx = Transaction::new("Straighten leader");
                for id in &tags {
                    if let Some(before) = model.get(*id).cloned() {
                        let mut after = before.clone();
                        if let ElementKind::Tag(t) = &mut after.kind {
                            t.elbow = None;
                        }
                        tx = tx.replace(before, after);
                    }
                }
                let r = self.doc.commit(tx);
                self.report(r, "Leader straightened.");
            }
            _ => {
                if let Some(i) = id
                    .strip_prefix("bar.cmd:")
                    .and_then(|i| i.parse::<usize>().ok())
                {
                    if let Some(c) = Command::ALL.iter().find(|c| **c as usize == i) {
                        self.run_command(ctx, *c, None);
                    }
                } else {
                    self.run_action(ctx, id);
                }
            }
        }
    }
}

fn icon_response(
    ui: &mut egui::Ui,
    a: &Act,
    tint: Option<egui::Color32>,
    color: egui::Color32,
) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(
        egui::vec2(BUTTON, BUTTON),
        if a.enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let v = ui.visuals();
    let painter = ui.painter();
    let fill = if a.on {
        tint.unwrap_or(v.selection.bg_fill).gamma_multiply(0.45)
    } else if resp.hovered() && a.enabled {
        v.widgets.hovered.weak_bg_fill
    } else if let Some(t) = tint {
        t.gamma_multiply(0.16)
    } else {
        egui::Color32::TRANSPARENT
    };
    painter.rect_filled(r, 6.0, fill);
    if a.on {
        painter.rect_stroke(
            r,
            6.0,
            egui::Stroke::new(1.0, tint.unwrap_or(v.selection.bg_fill)),
            egui::StrokeKind::Inside,
        );
    }
    let mut c = if tint.is_some() && !a.on {
        // the element's own: in the accent's ink
        v.selection.stroke.color.lerp_to_gamma(color, 0.35)
    } else {
        color
    };
    if !a.enabled {
        c = c.gamma_multiply(0.35);
    }
    crate::svgicon::paint(painter, r.shrink(3.5), a.icon, c);
    resp.on_hover_text(a.tip())
}

fn icon_button(
    ui: &mut egui::Ui,
    a: &Act,
    tint: Option<egui::Color32>,
    color: egui::Color32,
) -> bool {
    icon_response(ui, a, tint, color).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(acts: &[Act]) -> Vec<&str> {
        acts.iter().map(|a| a.id.as_str()).collect()
    }

    #[test]
    fn the_bar_shows_what_applies_to_all_of_the_selection() {
        let mut app = crate::app::test_app();
        let model = app.doc.model();
        let first = |f: &dyn Fn(&ElementKind) -> bool| {
            model.iter().find(|e| f(&e.kind)).map(|e| e.id).unwrap()
        };
        let wall = first(&|k| matches!(k, ElementKind::Wall(_)));
        let door = first(&|k| matches!(k, ElementKind::Opening(o) if o.kind == OpeningKind::Door));
        // a wall alone: its own first, then the common ones
        app.selection.only(wall);
        let (own, common) = app.element_actions();
        assert_eq!(ids(&own), ["wall.flip", "wall.attach", "bar.maketype"]);
        assert_eq!(common[0].label, "Duplicate");
        assert!(common.iter().any(|a| a.label == "Show only selected"));
        assert_eq!(common.last().unwrap().id, "nav.fitsel");
        // walls and a door: only Flip is theirs in common
        app.selection.extend([door], true);
        let (own, _) = app.element_actions();
        assert_eq!(ids(&own), ["wall.flip"]);
        assert!(own[0].tip().contains("Space"));
    }

    #[test]
    fn every_action_has_an_icon() {
        for icon in [
            "edit-outline",
            "flip",
            "attach-top",
            "make-type",
            "door-open",
            "open-view",
            "select-tagged",
            "straighten-leader",
            "duplicate",
            "select-similar",
            "eye-off",
            "isolate",
            "frame-selection",
            "ellipsis",
            "trash-2",
        ] {
            assert!(crate::svgicon::has(icon), "{icon}");
        }
    }
}
