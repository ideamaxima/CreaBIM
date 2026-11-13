//! The Sets manager (Manage tab): make, rename and delete the user's sets,
//! put the selection in them, and switch them on or off in the active view.
//! Every change is one transaction from [`creabim_core::sets`], so it undoes
//! like any edit.

use super::*;
use creabim_core::sets;

/// The window's own state, for the session.
#[derive(Default)]
pub(super) struct SetsUi {
    pub open: bool,
    /// The name typed for a new set.
    new_name: String,
    search: String,
    /// Names being typed, committed when the field loses focus.
    names: HashMap<ElementId, String>,
    /// A set asked to be deleted, waiting for a yes.
    delete: Option<ElementId>,
}

/// What a click in the manager asks for, done once the window is drawn.
pub(super) enum Ask {
    New(String),
    /// A new set holding the selection.
    NewFromSelection(String),
    Manage,
    Rename(ElementId, String),
    Assign(ElementId, bool),
    Show(ElementId, bool),
    Delete(ElementId),
    ConfirmDelete(ElementId),
    CancelDelete,
}

impl CreaBimApp {
    pub(super) fn sets_window(&mut self, ctx: &egui::Context) {
        if !self.sets.open {
            return;
        }
        let mut open = true;
        let mut asks = vec![];
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        crate::app::shell::window("Sets")
            .open(&mut open)
            .default_pos(area.min + Vec2::new(16.0, 56.0))
            .constrain_to(area)
            .min_width(520.0)
            .collapsible(false)
            .show(ctx, |ui| self.sets_contents(ui, &mut asks));
        self.sets.open = open;
        if !open {
            self.sets.delete = None;
        }
        for ask in asks {
            self.run_set_ask(ask);
        }
    }

    fn sets_contents(&mut self, ui: &mut egui::Ui, asks: &mut Vec<Ask>) {
        let model = self.doc.model();
        let target = self.vis_target(false);
        let hidden = target.and_then(|t| self.view_hidden(t)).unwrap_or_default();
        let view_name = match target {
            Some(gridvis::VisTarget::View(id)) => model.get(id).map(|e| e.name.clone()),
            _ => None,
        };
        crate::props::flow(ui, |ui| {
            let r = ui.add_sized(
                [220.0, 20.0],
                egui::TextEdit::singleline(&mut self.sets.new_name)
                    .hint_text("New set name")
                    .vertical_align(egui::Align::Center),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let name = self.sets.new_name.trim().to_string();
            if (ui
                .add_enabled(!name.is_empty(), egui::Button::new("Add set"))
                .on_hover_text("To put it in a folder, start the name with the folder and a \"/\": Phase 1/Temporary")
                .clicked()
                || enter)
                && !name.is_empty()
            {
                asks.push(Ask::New(name));
            }
        });
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(match &view_name {
                Some(v) => format!(
                    "Shown: switch a set off to leave its elements out of {v}. An element in several sets is left out when any of them is off."
                ),
                None => "Open a plan, section or 3D view to switch sets on and off there.".into(),
            })
            .weak(),
        );
        let list = sets::sets(model);
        if list.is_empty() {
            ui.separator();
            ui.label("No sets yet.");
            return;
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.sets.search)
                .hint_text("Search")
                .desired_width(220.0),
        );
        ui.separator();
        let q = self.sets.search.trim().to_lowercase();
        egui::ScrollArea::vertical()
            .max_height(360.0)
            .show(ui, |ui| {
                egui::Grid::new("sets")
                    .striped(true)
                    .num_columns(4)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        for h in ["Shown", "Name", "Elements", ""] {
                            ui.label(egui::RichText::new(h).strong());
                        }
                        ui.end_row();
                        for (id, name) in list {
                            if !q.is_empty() && !name.to_lowercase().contains(&q) {
                                continue;
                            }
                            let mut on = !hidden.sets.contains(&id);
                            if ui
                                .add_enabled(
                                    target.is_some(),
                                    egui::Checkbox::without_text(&mut on),
                                )
                                .changed()
                            {
                                asks.push(Ask::Show(id, on));
                            }
                            let draft = self
                                .sets
                                .names
                                .entry(id)
                                .or_insert_with(|| name.to_string());
                            let r = ui.add_sized(
                                [200.0, 20.0],
                                egui::TextEdit::singleline(draft)
                                    .vertical_align(egui::Align::Center),
                            );
                            if r.lost_focus() && draft.trim() != name && !draft.trim().is_empty() {
                                asks.push(Ask::Rename(id, draft.trim().to_string()));
                            } else if !r.has_focus() && draft != name {
                                *draft = name.to_string();
                            }
                            ui.label(sets::uses(model, id).0.to_string());
                            if self.sets.delete == Some(id) {
                                let (n, v) = sets::uses(model, id);
                                ui.horizontal(|ui| {
                                    ui.label(format!("{n} elements, {v} views."));
                                    if ui.small_button("Delete").clicked() {
                                        asks.push(Ask::ConfirmDelete(id));
                                    }
                                    if ui.small_button("Cancel").clicked() {
                                        asks.push(Ask::CancelDelete);
                                    }
                                });
                            } else if ui
                                .small_button("Delete")
                                .on_hover_text("Delete the set; its elements stay")
                                .clicked()
                            {
                                asks.push(Ask::Delete(id));
                            }
                            ui.end_row();
                        }
                    });
            });
    }

    /// The Sets menu of the element action bar: which sets the selection
    /// is in (a click puts it in or takes it out), a new set from it, and
    /// the manager.
    pub(super) fn sets_menu(&mut self, ui: &mut egui::Ui) -> Vec<Ask> {
        let mut asks = vec![];
        let model = self.doc.model();
        let ids = self.selection.all();
        let list = sets::sets(model);
        ui.set_min_width(220.0);
        for (id, name) in &list {
            let n = ids
                .iter()
                .filter(|e| model.get(**e).is_some_and(|e| e.sets.contains(id)))
                .count();
            let all = n == ids.len() && n > 0;
            let mut on = all;
            let r = ui.add(egui::Checkbox::new(&mut on, *name).indeterminate(n > 0 && !all));
            if r.clicked() {
                asks.push(Ask::Assign(*id, !all));
            }
        }
        if !list.is_empty() {
            ui.separator();
        }
        ui.label("New set from selection");
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.sets.new_name)
                    .hint_text("Name")
                    .desired_width(150.0),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let name = self.sets.new_name.trim().to_string();
            if (ui
                .add_enabled(!name.is_empty(), egui::Button::new("Make"))
                .clicked()
                || enter)
                && !name.is_empty()
            {
                asks.push(Ask::NewFromSelection(name));
            }
        });
        ui.separator();
        if ui.button("Manage sets…").clicked() {
            asks.push(Ask::Manage);
        }
        asks
    }

    pub(super) fn run_set_ask(&mut self, ask: Ask) {
        match ask {
            Ask::New(name) => {
                let e = sets::new_set(&name);
                let r = self
                    .doc
                    .commit(creabim_core::Transaction::new("New set").insert(e));
                self.sets.new_name.clear();
                self.report(r, format!("Set {name} made."));
            }
            Ask::NewFromSelection(name) => {
                let e = sets::new_set(&name);
                let id = e.id;
                let tx = creabim_core::Transaction::new("New set from selection").insert(e);
                let mut m = self.doc.model().clone();
                let _ = tx.apply(&mut m);
                let mut tx = tx;
                tx.ops
                    .extend(sets::assign(&m, id, &self.selection.all(), true).ops);
                let n = tx.ops.len() - 1;
                let r = self.doc.commit(tx);
                self.sets.new_name.clear();
                self.report(r, format!("Set {name} made with {n} elements."));
            }
            Ask::Manage => self.sets.open = true,
            Ask::Rename(id, name) => {
                let tx = sets::rename(self.doc.model(), id, &name);
                let r = self.doc.commit(tx);
                self.report(r, format!("Set renamed {name}."));
            }
            Ask::Assign(id, add) => {
                let ids = self.selection.all();
                let tx = sets::assign(self.doc.model(), id, &ids, add);
                let n = tx.ops.len();
                let r = self.doc.commit(tx);
                self.hidden_rev += 1;
                self.report(
                    r,
                    if add {
                        format!("{n} elements put in the set.")
                    } else {
                        format!("{n} elements taken out of the set.")
                    },
                );
            }
            Ask::Show(id, on) => {
                if let Some(t) = self.vis_target(false) {
                    let label = if on { "Set shown" } else { "Set hidden" };
                    self.set_view_hidden(t, label, |h| h.set_set(id, !on));
                }
            }
            Ask::Delete(id) => self.sets.delete = Some(id),
            Ask::CancelDelete => self.sets.delete = None,
            Ask::ConfirmDelete(id) => {
                self.sets.delete = None;
                let name = self
                    .doc
                    .model()
                    .get(id)
                    .map(|e| e.name.clone())
                    .unwrap_or_default();
                let tx = sets::delete(self.doc.model(), id);
                let r = self.doc.commit(tx);
                self.sets.names.remove(&id);
                self.report(r, format!("Set {name} deleted; its elements stay."));
            }
        }
    }
}
