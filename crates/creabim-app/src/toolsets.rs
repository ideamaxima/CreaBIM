//! The set each drawing tool puts what it draws in (Features 21): picked
//! in the top bar after Type, kept per tool and saved with the project
//! (outside the model, so not undone). While a tool draws, the document
//! stamps the new elements with it ([`creabim_core::Document::stamp_sets`]).
//! A set switched off in the view still takes them, with a warning: hidden
//! in the right set beats shown in none.

use crate::planview::{PlanState, Tool};
use crate::ui_model::Item;
use creabim_core::{ElementId, ElementKind, Model};
use serde_json::{json, Value};
use std::collections::HashMap;

/// The set each tool draws into, by the tool's name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolSets(HashMap<String, ElementId>);

/// The meta key the project file keeps them under.
pub const META_KEY: &str = "ui_tool_sets";

fn key(t: Tool) -> String {
    format!("{t:?}")
}

/// Does `tool` draw anything to put in a set?
pub fn draws(tool: Tool) -> bool {
    !matches!(tool, Tool::Select | Tool::Align | Tool::Measure)
}

impl ToolSets {
    /// `tool`'s set, while it still is one.
    pub fn of(&self, model: &Model, tool: Tool) -> Option<ElementId> {
        let id = *self.0.get(&key(tool))?;
        matches!(model.get(id).map(|e| &e.kind), Some(ElementKind::Set(_))).then_some(id)
    }

    pub fn pick(&mut self, tool: Tool, set: Option<ElementId>) {
        match set {
            Some(s) => self.0.insert(key(tool), s),
            None => self.0.remove(&key(tool)),
        };
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.0).unwrap_or_default()
    }

    /// Read them back; a file without them has none.
    pub fn from_json(s: &str) -> Self {
        Self(serde_json::from_str(s).unwrap_or_default())
    }
}

/// The set what the active tool draws goes in, if it draws.
pub fn stamp(state: &PlanState, model: &Model) -> Option<ElementId> {
    draws(state.tool)
        .then(|| state.tool_sets.of(model, state.tool))
        .flatten()
}

/// Is `set` switched off in `view`?
pub fn off_in(model: &Model, view: Option<ElementId>, set: ElementId) -> bool {
    match view.and_then(|v| model.get(v)).map(|e| &e.kind) {
        Some(ElementKind::View(v)) => v.hidden.sets.contains(&set),
        _ => false,
    }
}

fn name(model: &Model, set: Option<ElementId>) -> String {
    set.map_or("None".into(), |s| creabim_core::sets::path(model, s))
}

/// The top bar's Set menu, for the active tool in `view`.
pub fn option(state: &PlanState, model: &Model, view: Option<ElementId>) -> Option<Value> {
    if !draws(state.tool) {
        return None;
    }
    let set = state.tool_sets.of(model, state.tool);
    let off = set.is_some_and(|s| off_in(model, view, s));
    let value = if off {
        format!("⚠ {}", name(model, set))
    } else {
        name(model, set)
    };
    let hint = if off {
        "This set is off in this view: what you draw goes in it and stays hidden until you switch it on"
    } else {
        "The set what this tool draws goes in; each tool keeps its own"
    };
    Some(json!({"kind": "menu", "key": "toolset", "label": "Set", "value": value, "hint": hint}))
}

/// The Set menu's items: None, then every set, those off in the view
/// greyed out.
pub fn menu(state: &PlanState, model: &Model, view: Option<ElementId>) -> Vec<Item> {
    let now = state.tool_sets.of(model, state.tool);
    let mut items = vec![Item::new("toolset:none", "None").checked(now.is_none())];
    // as the Sets window shows them: each folder's name over its sets,
    // folders first, then the sets in no folder
    let mut item = |id: ElementId, label: String| {
        let off = off_in(model, view, id);
        let mut it = Item::new(format!("toolset:{}", id.0), label)
            .checked(now == Some(id))
            .enabled(!off);
        if off {
            it = it.hint("Off in this view");
        }
        it
    };
    fn walk(
        model: &Model,
        folder: ElementId,
        items: &mut Vec<Item>,
        item: &mut dyn FnMut(ElementId, String) -> Item,
    ) {
        let (folders, sets) = creabim_core::sets::children(model, Some(folder));
        items.push(Item::Header(creabim_core::sets::path(model, folder)));
        for s in sets {
            let name = model.get(s).map_or(String::new(), |e| e.name.clone());
            items.push(item(s, name));
        }
        for f in folders {
            walk(model, f, items, item);
        }
    }
    let (folders, loose) = creabim_core::sets::children(model, None);
    for f in folders {
        walk(model, f, &mut items, &mut item);
    }
    if !loose.is_empty() {
        items.push(Item::Separator);
    }
    for s in loose {
        let name = model.get(s).map_or(String::new(), |e| e.name.clone());
        items.push(item(s, name));
    }
    items
}

/// The set picked in the menu, for the active tool: `arg` is a set's id
/// or "none". The status line it leaves.
pub fn action(state: &mut PlanState, model: &Model, arg: &str) -> String {
    let set = model
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Set(_)) && e.id.0.to_string() == arg)
        .map(|e| e.id);
    let tool = state.tool;
    state.tool_sets.pick(tool, set);
    match set {
        Some(s) => format!(
            "{} draws into {} from now on.",
            tool.label(),
            name(model, Some(s))
        ),
        None => format!("{} draws into no set.", tool.label()),
    }
}

/// After a tool drew into `set` in `view`: the warning when that set is
/// off there.
pub fn warning(model: &Model, view: ElementId, set: ElementId) -> Option<String> {
    off_in(model, Some(view), set).then(|| {
        format!(
            "Drawn in {}, which is off in this view: switch it on to see it.",
            name(model, Some(set))
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{Element, Transaction, View};

    #[test]
    fn each_tool_keeps_its_set_and_they_save_and_read_back() {
        let mut doc = creabim_core::Document::new();
        let s = creabim_core::sets::new_set("Sketches");
        let set = s.id;
        doc.commit(Transaction::new("set").insert(s)).unwrap();
        let m = doc.model();
        let mut state = PlanState::default();
        state.tool = Tool::Line;
        action(&mut state, m, &set.0.to_string());
        assert_eq!(state.tool_sets.of(m, Tool::Line), Some(set));
        assert_eq!(state.tool_sets.of(m, Tool::Wall), None);
        assert_eq!(stamp(&state, m), Some(set));
        state.tool = Tool::Select;
        assert_eq!(stamp(&state, m), None, "Select draws nothing");
        let back = ToolSets::from_json(&state.tool_sets.to_json());
        assert_eq!(back, state.tool_sets);
        // a set deleted leaves the tool in none
        assert_eq!(back.of(&Model::new(), Tool::Line), None);
    }

    #[test]
    fn a_set_off_in_the_view_is_greyed_and_warned_about() {
        let mut doc = creabim_core::Document::new();
        let s = creabim_core::sets::new_set("Old");
        let set = s.id;
        let mut v = View::section(glam::DVec2::ZERO, glam::DVec2::X, 3.0, true);
        v.hidden.sets.push(set);
        let ve = Element::new("Section", ElementKind::View(v));
        let view = ve.id;
        doc.commit(Transaction::new("setup").insert(s).insert(ve))
            .unwrap();
        let m = doc.model();
        let mut state = PlanState::default();
        state.tool = Tool::Line;
        let items = menu(&state, m, Some(view));
        assert!(items
            .iter()
            .any(|i| matches!(i, Item::Action { enabled: false, .. })));
        state.tool_sets.pick(Tool::Line, Some(set));
        let o = option(&state, m, Some(view)).unwrap();
        assert!(o["value"].as_str().unwrap().starts_with('⚠'));
        assert!(warning(m, view, set).is_some());
    }

    #[test]
    fn the_menu_lists_sets_under_their_folders() {
        let mut doc = creabim_core::Document::new();
        let f = creabim_core::sets::new_folder("Structural", None);
        let mut wall = creabim_core::sets::new_set("Wall External");
        if let ElementKind::Set(s) = &mut wall.kind {
            s.folder = Some(f.id);
        }
        let loose = creabim_core::sets::new_set("Hidden");
        doc.commit(
            Transaction::new("setup")
                .insert(f)
                .insert(wall)
                .insert(loose),
        )
        .unwrap();
        let labels: Vec<String> = menu(&PlanState::default(), doc.model(), None)
            .into_iter()
            .filter_map(|i| match i {
                Item::Header(h) => Some(format!("# {h}")),
                Item::Action { label, .. } => Some(label),
                _ => None,
            })
            .collect();
        assert_eq!(labels, ["None", "# Structural", "Wall External", "Hidden"]);
    }
}
