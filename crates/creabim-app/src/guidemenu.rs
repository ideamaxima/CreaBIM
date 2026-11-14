//! Guides from the right-click menu: delete every guide on the level of
//! the selected one, in one step.

use super::*;
use crate::ui_model::Item;

impl CreaBimApp {
    /// The right-click item for a selected guide: every guide on its level
    /// goes in one step.
    pub(super) fn guide_menu_items(&self) -> Vec<Item> {
        let level = self.selection.all().into_iter().find_map(|id| {
            match self.doc.model().get(id).map(|e| &e.kind) {
                Some(ElementKind::Guide(g)) => Some(g.level),
                _ => None,
            }
        });
        let Some(level) = level else {
            return vec![];
        };
        let n = guides_on(self.doc.model(), level).len();
        vec![
            Item::new(
                "guides.delete_level",
                format!("Delete all guides on this level ({n})"),
            )
            .icon("trash-2")
            .hint("Every guide of this level, in one step: Ctrl+Z brings them back"),
            Item::Separator,
        ]
    }

    /// Delete every guide on the level of the selected guide.
    pub(super) fn delete_level_guides(&mut self) {
        let level = self.selection.all().into_iter().find_map(|id| {
            match self.doc.model().get(id).map(|e| &e.kind) {
                Some(ElementKind::Guide(g)) => Some(g.level),
                _ => None,
            }
        });
        let Some(level) = level else {
            self.status = "Select a guide first.".into();
            return;
        };
        let ids = guides_on(self.doc.model(), level);
        let n = ids.len();
        let tx = Transaction::new("Delete all guides on the level")
            .remove_all_cascade(self.doc.model(), &ids);
        let r = self.doc.commit(tx);
        self.selection.retain(|id| !ids.contains(&id));
        self.report(r, format!("Deleted {n} guides. Ctrl+Z brings them back."));
    }
}

/// The guides on `level` (`None`: on no level).
pub(super) fn guides_on(model: &Model, level: Option<ElementId>) -> Vec<ElementId> {
    model
        .iter()
        .filter(|e| matches!(&e.kind, ElementKind::Guide(g) if g.level == level))
        .map(|e| e.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::gridvis::guide_ids;
    use super::*;
    use serde_json::json;

    /// The app round the sample house, with a guide on no level.
    fn app() -> CreaBimApp {
        let mut app = CreaBimApp::blank(None);
        app.load_model(sample_house());
        let guide: creabim_core::Guide = serde_json::from_value(json!({
            "level": null, "a": [0.0, -2.0, 0.0], "b": [4.0, -2.0, 0.0],
        }))
        .unwrap();
        let e = Element::new("Guide", ElementKind::Guide(guide));
        app.doc.commit(Transaction::new("Guide").insert(e)).unwrap();
        app
    }

    #[test]
    fn delete_all_guides_takes_the_level_s_guides_only_in_one_step() {
        let mut app = app();
        let ctx = egui::Context::default();
        let level = app
            .doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Level(_)))
            .map(|e| e.id);
        let on_level = |a: f64| {
            let g: creabim_core::Guide = serde_json::from_value(json!({
                "level": level, "a": [a, 0.0, 0.0], "b": [a, 1.0, 0.0],
            }))
            .unwrap();
            Element::new("Guide", ElementKind::Guide(g))
        };
        let (g1, g2) = (on_level(1.0), on_level(2.0));
        let first = g1.id;
        app.doc
            .commit(Transaction::new("Guides").insert(g1).insert(g2))
            .unwrap();
        assert_eq!(guide_ids(app.doc.model()).len(), 3);
        app.selection.only(first);
        let items = app.guide_menu_items();
        assert!(items.iter().any(|i| matches!(i,
            Item::Action { id, .. } if id == "guides.delete_level")));
        app.run_action(&ctx, "guides.delete_level");
        // the guide on no level stays
        assert_eq!(guide_ids(app.doc.model()).len(), 1);
        assert!(app.selection.is_empty());
        app.doc.undo();
        assert_eq!(guide_ids(app.doc.model()).len(), 3);
        // nothing for a selection with no guide in it
        app.selection.clear();
        assert!(app.guide_menu_items().is_empty());
    }
}
