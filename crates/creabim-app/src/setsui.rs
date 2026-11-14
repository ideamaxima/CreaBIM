//! The Sets manager (Manage tab) and the Sets menu of the element action
//! bar. The manager holds everything about sets: folders (nested, empty
//! ones too), making, renaming, moving, duplicating, merging and deleting,
//! selecting what a set holds, and switching sets on and off in the active
//! view. Every change is one transaction from [`creabim_core::sets`], so it
//! undoes like any edit.

use super::*;
use creabim_core::{sets, Model, Transaction, ViewHidden};
use std::collections::HashSet;

/// The window's own state, for the session.
#[derive(Default)]
pub(super) struct SetsUi {
    pub open: bool,
    /// The name typed for a new set or folder.
    new_name: String,
    search: String,
    /// Names being typed, committed when the field loses focus.
    names: HashMap<ElementId, String>,
    /// A set or folder asked to be deleted, waiting for a yes.
    delete: Option<ElementId>,
    /// Folders closed in the tree.
    pub closed: HashSet<ElementId>,
    /// The folder the Sets menu makes new sets in; none: the top.
    menu_folder: Option<ElementId>,
    /// The Sets menu's folder list is open.
    menu_pick: bool,
    /// The name typed for a new combination.
    new_combination: String,
}

/// What a click asks for, done once the window or menu is drawn.
pub(super) enum Ask {
    /// A new set (or folder, when true) named so, in a folder.
    New(String, bool, Option<ElementId>),
    /// A new set holding the selection.
    NewFromSelection(String, Option<ElementId>),
    Manage,
    Rename(ElementId, String),
    Assign(ElementId, bool),
    /// Switch these sets on (true) or off in the active view.
    Show(Vec<ElementId>, bool),
    /// Every set on (true) or off.
    ShowAll(bool),
    /// Only this set on.
    Isolate(ElementId),
    Select(ElementId),
    Duplicate(ElementId),
    MoveTo(ElementId, Option<ElementId>),
    Merge(ElementId, ElementId),
    Delete(ElementId),
    ConfirmDelete(ElementId),
    CancelDelete,
    /// A new combination named so, from the active view's sets.
    NewCombination(String),
    /// The active view follows this combination.
    ApplyCombination(ElementId),
    /// The combination takes the active view's sets.
    UpdateCombination(ElementId),
    /// A copy of the active view following this combination.
    ViewFromCombination(ElementId),
    DuplicateCombination(ElementId),
}

/// What the tree rows need to read, gathered before they draw.
struct Tree<'a> {
    model: &'a Model,
    hidden: &'a ViewHidden,
    /// A view to switch sets in is open.
    live: bool,
    /// Every folder, with its path, for Move to.
    folders: Vec<(ElementId, String)>,
    /// Every set, with its path, for Merge into.
    all_sets: Vec<(ElementId, String)>,
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
            // as tall as the Catalogue: down to the same margin above the
            // app's bottom edge
            .default_size([560.0, (area.height() - 56.0 - 160.0).max(360.0)])
            .min_width(560.0)
            .resizable([true, true])
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
        let target = self.vis_target(false);
        let hidden = target.and_then(|t| self.view_hidden(t)).unwrap_or_default();
        let model = self.doc.model();
        let view_id = match target {
            Some(gridvis::VisTarget::View(id)) => Some(id),
            _ => None,
        };
        let view_name = view_id.and_then(|id| model.get(id)).map(|e| e.name.clone());
        let ui_state = &mut self.sets;
        // few, and picked from: on top, always in sight
        if !sets::sets(model).is_empty() {
            combinations_section(ui, model, &hidden, view_name.as_deref(), ui_state, asks);
            ui.separator();
        }
        ui.label(egui::RichText::new("Sets").strong());
        ui.horizontal(|ui| {
            let r = ui.add_sized(
                [200.0, 20.0],
                egui::TextEdit::singleline(&mut ui_state.new_name)
                    .hint_text("Name")
                    .vertical_align(egui::Align::Center),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let name = ui_state.new_name.trim().to_string();
            let set_taken = sets::name_taken(model, &sets::new_set("").kind, &name, None);
            let folder_taken =
                sets::name_taken(model, &sets::new_folder("", None).kind, &name, None);
            let ok = !name.is_empty() && !set_taken;
            let enter = enter && ok;
            if (ui
                .add_enabled(ok, egui::Button::new("New set"))
                .on_hover_text("A new set at the top; move it into a folder from its ⋯ menu")
                .clicked()
                || enter)
                && ok
            {
                asks.push(Ask::New(name.clone(), false, None));
            }
            if ui
                .add_enabled(
                    !name.is_empty() && !folder_taken,
                    egui::Button::new("New folder"),
                )
                .on_hover_text("A folder at the top; folders can hold sets and other folders")
                .clicked()
            {
                asks.push(Ask::New(name, true, None));
            }
            if set_taken || folder_taken {
                ui.label(
                    egui::RichText::new(match (set_taken, folder_taken) {
                        (true, true) => "Already used by a set and a folder",
                        (true, false) => "Already used by a set",
                        _ => "Already used by a folder",
                    })
                    .color(ui.visuals().error_fg_color),
                );
            }
        });
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(match &view_name {
                Some(v) => format!(
                    "The tick shows a set in {v}; untick it to leave its elements out. An element in several sets is left out when any of them is off."
                ),
                None => "Open a plan, section or 3D view to switch sets on and off there.".into(),
            })
            .weak(),
        );
        let all_sets: Vec<(ElementId, String)> = sets::sets(model)
            .into_iter()
            .map(|(id, _)| (id, sets::path(model, id)))
            .collect();
        let mut folders: Vec<(ElementId, String)> = model
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::SetFolder(_)))
            .map(|e| (e.id, sets::path(model, e.id)))
            .collect();
        folders.sort_by(|a, b| a.1.cmp(&b.1));
        if all_sets.is_empty() && folders.is_empty() {
            ui.separator();
            ui.label("No sets yet.");
            return;
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut ui_state.search)
                    .hint_text("Search")
                    .desired_width(200.0),
            );
            ui.add_enabled_ui(target.is_some(), |ui| {
                if ui
                    .button("Show all")
                    .on_hover_text("Every set on in this view")
                    .clicked()
                {
                    asks.push(Ask::ShowAll(true));
                }
                if ui
                    .button("Hide all")
                    .on_hover_text("Every set off in this view")
                    .clicked()
                {
                    asks.push(Ask::ShowAll(false));
                }
            });
        });
        ui.separator();
        let tree = Tree {
            model,
            hidden: &hidden,
            live: target.is_some(),
            folders,
            all_sets,
        };
        let q = ui_state.search.trim().to_lowercase();
        // the list takes what the window has left, down to its bottom edge
        egui::ScrollArea::vertical()
            .id_salt("sets_tree")
            .max_height(ui.available_height().max(160.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if q.is_empty() {
                    folder_rows(ui, &tree, ui_state, None, 0, asks);
                } else {
                    for (id, p) in &tree.all_sets {
                        if p.to_lowercase().contains(&q) {
                            set_row(ui, &tree, ui_state, *id, 0, Some(p), asks);
                        }
                    }
                }
            });
    }

    /// The Sets menu of the element action bar: which sets the selection
    /// is in (a click puts it in or takes it out), a new set from it, and
    /// the manager.
    pub(super) fn sets_menu(&mut self, ui: &mut egui::Ui) -> Vec<Ask> {
        let mut asks = vec![];
        let model = self.doc.model();
        let ids = self.selection.all();
        let any = model
            .iter()
            .any(|e| matches!(e.kind, ElementKind::Set(_) | ElementKind::SetFolder(_)));
        ui.set_min_width(220.0);
        menu_rows(ui, model, &ids, &mut self.sets.closed, None, 0, &mut asks);
        if any {
            ui.separator();
        }
        ui.label("New set from selection");
        // the folder it goes in, kept for next time while it exists
        if self.sets.menu_folder.is_some_and(|f| {
            !matches!(
                model.get(f).map(|e| &e.kind),
                Some(ElementKind::SetFolder(_))
            )
        }) {
            self.sets.menu_folder = None;
        }
        let mut folders: Vec<(ElementId, String)> = model
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::SetFolder(_)))
            .map(|e| (e.id, sets::path(model, e.id)))
            .collect();
        folders.sort_by(|a, b| a.1.cmp(&b.1));
        let shown = match self.sets.menu_folder {
            Some(f) => sets::path(model, f),
            None => "Top (no folder)".into(),
        };
        // a list that opens inside this menu: a dropdown would be a second
        // popup, and a click in it would close this one
        ui.horizontal(|ui| {
            ui.label("In");
            let open = self.sets.menu_pick;
            let r = icon_button(
                ui,
                if open {
                    "chevron-down"
                } else {
                    "chevron-right"
                },
                "Pick the folder",
            );
            let l = ui.add(
                egui::Label::new(egui::RichText::new(shown).strong()).sense(egui::Sense::click()),
            );
            if r.clicked() || l.clicked() {
                self.sets.menu_pick = !open;
            }
        });
        if self.sets.menu_pick {
            ui.indent("sets-menu-folders", |ui| {
                let mut pick = |ui: &mut egui::Ui, f: Option<ElementId>, label: &str| {
                    if ui
                        .selectable_label(self.sets.menu_folder == f, label)
                        .clicked()
                    {
                        self.sets.menu_folder = f;
                        self.sets.menu_pick = false;
                    }
                };
                pick(ui, None, "Top (no folder)");
                for (f, p) in &folders {
                    pick(ui, Some(*f), p);
                }
            });
        }
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.sets.new_name)
                    .hint_text("Name")
                    .desired_width(150.0),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let name = self.sets.new_name.trim().to_string();
            let taken = sets::name_taken(model, &sets::new_set("").kind, &name, None);
            let ok = !name.is_empty() && !taken;
            if (ui.add_enabled(ok, egui::Button::new("New set")).clicked() || enter) && ok {
                asks.push(Ask::NewFromSelection(name, self.sets.menu_folder));
            }
            if taken {
                already_used(ui);
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
            Ask::New(name, folder, parent) => {
                let e = if folder {
                    sets::new_folder(&name, parent)
                } else {
                    let mut e = sets::new_set(&name);
                    if let ElementKind::Set(s) = &mut e.kind {
                        s.folder = parent;
                    }
                    e
                };
                let label = if folder { "New folder" } else { "New set" };
                let r = self.doc.commit(Transaction::new(label).insert(e));
                self.sets.new_name.clear();
                if let Some(p) = parent {
                    self.sets.closed.remove(&p);
                }
                self.report(r, format!("{label} {name} made."));
            }
            Ask::NewFromSelection(name, folder) => {
                let mut e = sets::new_set(&name);
                if let ElementKind::Set(s) = &mut e.kind {
                    s.folder = folder;
                }
                if let Some(f) = folder {
                    self.sets.closed.remove(&f);
                }
                let id = e.id;
                let tx = Transaction::new("New set from selection").insert(e);
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
                let model = self.doc.model();
                let tx = if matches!(
                    model.get(id).map(|e| &e.kind),
                    Some(ElementKind::Combination(_))
                ) {
                    sets::rename_combination(model, id, &name)
                } else {
                    sets::rename(model, id, &name)
                };
                let r = self.doc.commit(tx);
                self.report(r, format!("Renamed {name}."));
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
            Ask::Show(ids, on) => {
                if let Some(t) = self.vis_target(false) {
                    let label = if on { "Sets shown" } else { "Sets hidden" };
                    self.set_view_hidden(t, label, |h| {
                        for id in ids {
                            h.set_set(id, !on);
                        }
                    });
                }
            }
            Ask::ShowAll(on) => {
                let all: Vec<ElementId> = sets::sets(self.doc.model())
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();
                if let Some(t) = self.vis_target(false) {
                    let label = if on {
                        "All sets shown"
                    } else {
                        "All sets hidden"
                    };
                    self.set_view_hidden(t, label, |h| {
                        for id in all {
                            h.set_set(id, !on);
                        }
                    });
                }
            }
            Ask::Isolate(only) => {
                let all: Vec<ElementId> = sets::sets(self.doc.model())
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();
                if let Some(t) = self.vis_target(false) {
                    self.set_view_hidden(t, "Set isolated", |h| {
                        for id in all {
                            h.set_set(id, id != only);
                        }
                    });
                }
            }
            Ask::Select(id) => {
                let ids = sets::members(self.doc.model(), id);
                let n = ids.len();
                self.selection.extend(ids, false);
                self.status = format!("Selected the {n} elements of the set.");
            }
            Ask::Duplicate(id) => {
                if let Some((tx, new)) = sets::duplicate(self.doc.model(), id) {
                    let r = self.doc.commit(tx);
                    let name = self
                        .doc
                        .model()
                        .get(new)
                        .map(|e| e.name.clone())
                        .unwrap_or_default();
                    self.report(r, format!("Set duplicated as {name}."));
                }
            }
            Ask::MoveTo(id, folder) => {
                let tx = sets::move_to(self.doc.model(), id, folder);
                if tx.ops.is_empty() {
                    self.status = "A folder can't go inside itself.".into();
                    return;
                }
                let r = self.doc.commit(tx);
                if let Some(f) = folder {
                    self.sets.closed.remove(&f);
                }
                self.report(r, "Moved.");
            }
            Ask::Merge(from, into) => {
                let into_name = sets::path(self.doc.model(), into);
                let tx = sets::merge(self.doc.model(), from, into);
                let r = self.doc.commit(tx);
                self.sets.names.remove(&from);
                self.report(r, format!("Merged into {into_name}."));
            }
            Ask::Delete(id) => self.sets.delete = Some(id),
            Ask::CancelDelete => self.sets.delete = None,
            Ask::ConfirmDelete(id) => {
                self.sets.delete = None;
                let model = self.doc.model();
                let name = model.get(id).map(|e| e.name.clone()).unwrap_or_default();
                if matches!(
                    model.get(id).map(|e| &e.kind),
                    Some(ElementKind::Combination(_))
                ) {
                    let tx = sets::delete_combination(model, id);
                    let r = self.doc.commit(tx);
                    self.sets.names.remove(&id);
                    self.report(
                        r,
                        format!("Combination {name} deleted; its views keep their sets."),
                    );
                    return;
                }
                let folder = matches!(
                    model.get(id).map(|e| &e.kind),
                    Some(ElementKind::SetFolder(_))
                );
                let tx = if folder {
                    sets::delete_folder(model, id)
                } else {
                    sets::delete(model, id)
                };
                let r = self.doc.commit(tx);
                self.sets.names.remove(&id);
                self.report(
                    r,
                    if folder {
                        format!("Folder {name} deleted; what it held moved up.")
                    } else {
                        format!("Set {name} deleted; its elements stay.")
                    },
                );
            }
            Ask::NewCombination(name) => {
                let Some(gridvis::VisTarget::View(view)) = self.vis_target(false) else {
                    return;
                };
                let (tx, id) = sets::new_combination(self.doc.model(), &name, view);
                let r = self.doc.commit(tx);
                self.sets.new_combination.clear();
                let name = self
                    .doc
                    .model()
                    .get(id)
                    .map(|e| e.name.clone())
                    .unwrap_or(name);
                self.hidden_rev += 1;
                self.report(r, format!("Combination {name} made from this view."));
            }
            Ask::ApplyCombination(id) => {
                let Some(gridvis::VisTarget::View(view)) = self.vis_target(false) else {
                    return;
                };
                let tx = sets::apply_combination(self.doc.model(), id, view);
                let r = self.doc.commit(tx);
                self.hidden_rev += 1;
                self.report(r, "Combination used in this view.");
            }
            Ask::UpdateCombination(id) => {
                let Some(gridvis::VisTarget::View(view)) = self.vis_target(false) else {
                    return;
                };
                let tx = sets::update_combination(self.doc.model(), id, view);
                let r = self.doc.commit(tx);
                self.hidden_rev += 1;
                let n = sets::combination_views(self.doc.model(), id).len();
                self.report(r, format!("Combination updated; {n} views follow it."));
            }
            Ask::DuplicateCombination(id) => {
                if let Some((tx, new)) = sets::duplicate_combination(self.doc.model(), id) {
                    let r = self.doc.commit(tx);
                    let name = self
                        .doc
                        .model()
                        .get(new)
                        .map(|e| e.name.clone())
                        .unwrap_or_default();
                    self.report(r, format!("Combination duplicated as {name}."));
                }
            }
            Ask::ViewFromCombination(id) => {
                let Some(gridvis::VisTarget::View(view)) = self.vis_target(false) else {
                    return;
                };
                if let Some((tx, new)) = sets::view_from_combination(self.doc.model(), id, view) {
                    let r = self.doc.commit(tx);
                    let name = self
                        .doc
                        .model()
                        .get(new)
                        .map(|e| e.name.clone())
                        .unwrap_or_default();
                    self.report(r, format!("View {name} made; this one is unchanged."));
                }
            }
        }
    }
}

/// The Combinations: named states of the sets. One is made from the
/// active view; "New view" copies the active view to follow it, leaving
/// the view being worked on whole.
fn combinations_section(
    ui: &mut egui::Ui,
    model: &Model,
    hidden: &ViewHidden,
    view: Option<&str>,
    st: &mut SetsUi,
    asks: &mut Vec<Ask>,
) {
    let live = view.is_some();
    let modified = sets::modified(model, hidden);
    ui.label(egui::RichText::new("Combinations").strong());
    ui.add_enabled_ui(live, |ui| {
        ui.horizontal(|ui| {
            let r = ui.add_sized(
                [200.0, 20.0],
                egui::TextEdit::singleline(&mut st.new_combination)
                    .hint_text("Name")
                    .vertical_align(egui::Align::Center),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let name = st.new_combination.trim().to_string();
            let taken = sets::name_taken(
                model,
                &ElementKind::Combination(Default::default()),
                &name,
                None,
            );
            let ok = !name.is_empty() && !taken;
            if (ui
                .add_enabled(ok, egui::Button::new("New combination"))
                .on_hover_text("Remembers which sets are on and off in this view")
                .clicked()
                || enter)
                && ok
            {
                asks.push(Ask::NewCombination(name));
            }
            if taken {
                already_used(ui);
            }
        });
    });
    if !live {
        ui.label(egui::RichText::new("Open a plan or section to use combinations.").weak());
    }
    for (id, name) in sets::combinations(model) {
        let on = hidden.combination == Some(id);
        ui.horizontal(|ui| {
            ui.add_space(INDENT);
            if ui
                .add_enabled(live, egui::RadioButton::new(on, ""))
                .on_hover_text("Use it in this view")
                .clicked()
            {
                asks.push(Ask::ApplyCombination(id));
            }
            name_field(ui, st, model, id, name, asks);
            if on && modified {
                ui.label(egui::RichText::new("(modified)").italics());
            }
            let views = sets::combination_views(model, id);
            let r = ui.label(
                egui::RichText::new(match views.len() {
                    1 => "1 view".to_string(),
                    n => format!("{n} views"),
                })
                .weak(),
            );
            if !views.is_empty() {
                r.on_hover_text(views.join(", "));
            }
            ui.add_enabled_ui(live, |ui| {
                if ui
                    .add_enabled(on && modified, egui::Button::new("Update"))
                    .on_hover_text(
                        "The combination takes this view's sets; the views using it change too",
                    )
                    .clicked()
                {
                    asks.push(Ask::UpdateCombination(id));
                }
                if ui
                    .button("New view")
                    .on_hover_text(
                        "A copy of this view using the combination; this view stays as it is",
                    )
                    .clicked()
                {
                    asks.push(Ask::ViewFromCombination(id));
                }
            });
            let more = icon_button(ui, "ellipsis", "More");
            egui::Popup::menu(&more).show(|ui| {
                if ui.button("Duplicate").clicked() {
                    asks.push(Ask::DuplicateCombination(id));
                }
                if ui
                    .button("Delete")
                    .on_hover_text("The views using it keep their sets")
                    .clicked()
                {
                    asks.push(Ask::ConfirmDelete(id));
                }
            });
        });
    }
}

/// Room for one step of the tree.
const INDENT: f32 = 18.0;

/// The folders, then the sets, sitting in `folder`, and what is inside
/// each open folder below it.
fn folder_rows(
    ui: &mut egui::Ui,
    t: &Tree,
    st: &mut SetsUi,
    folder: Option<ElementId>,
    depth: usize,
    asks: &mut Vec<Ask>,
) {
    let (folders, set_ids) = sets::children(t.model, folder);
    for f in folders {
        let open = folder_row(ui, t, st, f, depth, asks);
        if open {
            folder_rows(ui, t, st, Some(f), depth + 1, asks);
        }
    }
    for s in set_ids {
        set_row(ui, t, st, s, depth, None, asks);
    }
}

/// The editable name of a set or folder; the change goes when it loses focus.
/// The name of a set, folder or combination, renamed when the field is
/// left. A name another of its kind has shows "Already used" and is not
/// taken: the old name comes back.
fn name_field(
    ui: &mut egui::Ui,
    st: &mut SetsUi,
    model: &Model,
    id: ElementId,
    name: &str,
    asks: &mut Vec<Ask>,
) {
    let draft = st.names.entry(id).or_insert_with(|| name.to_string());
    let r = ui.add_sized(
        [180.0, 20.0],
        egui::TextEdit::singleline(draft).vertical_align(egui::Align::Center),
    );
    let taken = model
        .get(id)
        .is_some_and(|e| sets::name_taken(model, &e.kind, draft, Some(id)));
    if r.lost_focus() && !taken && draft.trim() != name && !draft.trim().is_empty() {
        asks.push(Ask::Rename(id, draft.trim().to_string()));
    } else if !r.has_focus() && draft != name {
        *draft = name.to_string();
    } else if taken && r.has_focus() {
        already_used(ui);
    }
}

/// The red note under a name another of its kind already has.
fn already_used(ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("Already used").color(ui.visuals().error_fg_color));
}

/// "Move to ▸": the top and every folder but `id` itself.
fn move_menu(ui: &mut egui::Ui, t: &Tree, id: ElementId, asks: &mut Vec<Ask>) {
    ui.menu_button("Move to", |ui| {
        let here = sets::parent_of(t.model, id);
        if ui
            .add_enabled(here.is_some(), egui::Button::new("Top (no folder)"))
            .clicked()
        {
            asks.push(Ask::MoveTo(id, None));
            ui.close();
        }
        for (f, p) in &t.folders {
            if *f == id {
                continue;
            }
            if ui
                .add_enabled(here != Some(*f), egui::Button::new(p))
                .clicked()
            {
                asks.push(Ask::MoveTo(id, Some(*f)));
                ui.close();
            }
        }
    });
}

/// The yes/no after Delete was asked for this row.
fn delete_question(
    ui: &mut egui::Ui,
    st: &SetsUi,
    id: ElementId,
    what: String,
    asks: &mut Vec<Ask>,
) {
    if st.delete == Some(id) {
        ui.label(what);
        if ui.small_button("Delete").clicked() {
            asks.push(Ask::ConfirmDelete(id));
        }
        if ui.small_button("Cancel").clicked() {
            asks.push(Ask::CancelDelete);
        }
    }
}

/// One folder: open/close, its tick for all its sets, name and menu. True
/// when it is open.
fn folder_row(
    ui: &mut egui::Ui,
    t: &Tree,
    st: &mut SetsUi,
    id: ElementId,
    depth: usize,
    asks: &mut Vec<Ask>,
) -> bool {
    let Some(e) = t.model.get(id) else {
        return false;
    };
    let open = !st.closed.contains(&id);
    let under = sets::sets_under(t.model, id);
    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * INDENT);
        if icon_button(
            ui,
            if open {
                "chevron-down"
            } else {
                "chevron-right"
            },
            if open {
                "Close the folder"
            } else {
                "Open the folder"
            },
        )
        .clicked()
        {
            if open {
                st.closed.insert(id);
            } else {
                st.closed.remove(&id);
            }
        }
        let off = under.iter().filter(|s| t.hidden.sets.contains(s)).count();
        let mut on = !under.is_empty() && off == 0;
        let r = ui
            .add_enabled(
                t.live && !under.is_empty(),
                egui::Checkbox::without_text(&mut on).indeterminate(off > 0 && off < under.len()),
            )
            .on_hover_text("Every set in the folder, on or off");
        if r.clicked() {
            asks.push(Ask::Show(under.clone(), off > 0));
        }
        name_field(ui, st, t.model, id, &e.name, asks);
        ui.label(
            egui::RichText::new(match under.len() {
                1 => "1 set".to_string(),
                n => format!("{n} sets"),
            })
            .weak(),
        );
        let more = icon_button(ui, "ellipsis", "More");
        egui::Popup::menu(&more).show(|ui| {
            if ui.button("New set here").clicked() {
                asks.push(Ask::New("New set".into(), false, Some(id)));
                ui.close();
            }
            if ui.button("New folder here").clicked() {
                asks.push(Ask::New("New folder".into(), true, Some(id)));
                ui.close();
            }
            move_menu(ui, t, id, asks);
            ui.separator();
            if ui
                .button("Delete folder")
                .on_hover_text("What it holds moves up a level")
                .clicked()
            {
                // an empty folder goes at once; Ctrl+Z brings it back
                let (fs, ss) = sets::children(t.model, Some(id));
                asks.push(if fs.is_empty() && ss.is_empty() {
                    Ask::ConfirmDelete(id)
                } else {
                    Ask::Delete(id)
                });
                ui.close();
            }
        });
        delete_question(ui, st, id, "Its sets move up.".into(), asks);
    });
    open
}

/// One set: its tick, name, how much it holds and where it is off, and
/// what can be done with it. `path` is shown when searching.
fn set_row(
    ui: &mut egui::Ui,
    t: &Tree,
    st: &mut SetsUi,
    id: ElementId,
    depth: usize,
    path: Option<&String>,
    asks: &mut Vec<Ask>,
) {
    let Some(e) = t.model.get(id) else { return };
    ui.horizontal(|ui| {
        // lined up with the folders' names, past their arrow
        ui.add_space(depth as f32 * INDENT + INDENT);
        let mut on = !t.hidden.sets.contains(&id);
        if ui
            .add_enabled(t.live, egui::Checkbox::without_text(&mut on))
            .changed()
        {
            asks.push(Ask::Show(vec![id], on));
        }
        name_field(ui, st, t.model, id, &e.name, asks);
        let n = sets::members(t.model, id).len();
        let views = sets::views_off(t.model, id);
        let info = if views.is_empty() {
            match n {
                1 => "1 element".to_string(),
                n => format!("{n} elements"),
            }
        } else {
            let what = match n {
                1 => "1 element".to_string(),
                n => format!("{n} elements"),
            };
            format!("{what} · off in {}", views.len())
        };
        let r = ui.label(egui::RichText::new(info).weak());
        if !views.is_empty() {
            r.on_hover_text(format!("Off in: {}", views.join(", ")));
        }
        if let Some(p) = path {
            ui.label(egui::RichText::new(p).weak().italics());
        }
        if ui
            .add_enabled(n > 0, egui::Button::new("Select").small())
            .on_hover_text("Select its elements")
            .clicked()
        {
            asks.push(Ask::Select(id));
        }
        if ui
            .add_enabled(t.live, egui::Button::new("Isolate").small())
            .on_hover_text("In this view, only this set on")
            .clicked()
        {
            asks.push(Ask::Isolate(id));
        }
        let more = icon_button(ui, "ellipsis", "More");
        egui::Popup::menu(&more).show(|ui| {
            if ui
                .button("Duplicate")
                .on_hover_text("A new set with the same elements")
                .clicked()
            {
                asks.push(Ask::Duplicate(id));
                ui.close();
            }
            move_menu(ui, t, id, asks);
            ui.menu_button("Merge into", |ui| {
                for (s, p) in &t.all_sets {
                    if *s != id && ui.button(p).clicked() {
                        asks.push(Ask::Merge(id, *s));
                        ui.close();
                    }
                }
            });
            ui.separator();
            if ui
                .button("Delete set")
                .on_hover_text("Its elements stay")
                .clicked()
            {
                // an empty set goes at once; Ctrl+Z brings it back
                asks.push(if n == 0 {
                    Ask::ConfirmDelete(id)
                } else {
                    Ask::Delete(id)
                });
                ui.close();
            }
        });
        delete_question(ui, st, id, format!("{n} elements stay."), asks);
    });
}

/// A small icon button, drawn with the app's own icons (the text font has
/// no arrows or ellipsis).
pub(super) fn icon_button(ui: &mut egui::Ui, icon: &str, hint: &str) -> egui::Response {
    let size = egui::vec2(20.0, 20.0);
    let (r, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let v = ui.visuals();
    if resp.hovered() {
        ui.painter()
            .rect_filled(r, 3.0, v.widgets.hovered.weak_bg_fill);
    }
    crate::svgicon::paint(ui.painter(), r.shrink(3.0), icon, v.text_color());
    resp.on_hover_text(hint)
}

/// The Sets menu's tree: folders that open and close (sharing the
/// manager's), and a tick per set, full when every selected element is in
/// it and half when some are.
fn menu_rows(
    ui: &mut egui::Ui,
    model: &Model,
    ids: &[ElementId],
    closed: &mut HashSet<ElementId>,
    folder: Option<ElementId>,
    depth: usize,
    asks: &mut Vec<Ask>,
) {
    let (folders, set_ids) = sets::children(model, folder);
    for f in folders {
        let Some(e) = model.get(f) else { continue };
        let open = !closed.contains(&f);
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * INDENT);
            let r = icon_button(
                ui,
                if open {
                    "chevron-down"
                } else {
                    "chevron-right"
                },
                if open {
                    "Close the folder"
                } else {
                    "Open the folder"
                },
            );
            let l = ui.add(egui::Label::new(&e.name).sense(egui::Sense::click()));
            if r.clicked() || l.clicked() {
                if open {
                    closed.insert(f);
                } else {
                    closed.remove(&f);
                }
            }
        });
        if open {
            menu_rows(ui, model, ids, closed, Some(f), depth + 1, asks);
        }
    }
    for s in set_ids {
        let Some(e) = model.get(s) else { continue };
        let n = ids
            .iter()
            .filter(|i| model.get(**i).is_some_and(|x| x.sets.contains(&s)))
            .count();
        let all = n == ids.len() && n > 0;
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * INDENT + INDENT);
            let mut on = all;
            let r =
                ui.add(egui::Checkbox::new(&mut on, e.name.as_str()).indeterminate(n > 0 && !all));
            if r.clicked() {
                asks.push(Ask::Assign(s, !all));
            }
        });
    }
}
