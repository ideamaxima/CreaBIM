//! Starting a project: the empty project, and the New project window shown
//! when the app opens.

use super::*;

impl CreaBimApp {
    /// An empty project: two levels, in the units in use.
    pub(super) fn load_empty_project(&mut self) {
        let units = self.doc.model().units();
        let mut m = Document::new();
        let mut tx = Transaction::new("New project");
        for (name, z) in [("Level 0", 0.0), ("Level 1", 3.0)] {
            tx = tx.insert(Element::new(
                name,
                ElementKind::Level(creabim_core::Level {
                    elevation: z,
                    height: 3.0,
                }),
            ));
        }
        let _ = m.commit(tx);
        let tx = creabim_core::units::set_units(m.model(), units);
        let _ = m.commit(tx);
        self.load_model(m.model().clone());
        self.restore_areas(None);
        self.path = shell::new_path("Untitled");
        self.status = "New project with two levels. Pick the Wall tool to start drawing.".into();
    }

    /// Open the New project window once, on the first frame the panels
    /// are ready, when the app started without a file.
    pub(super) fn startup_dialog(&mut self, ctx: &egui::Context) {
        if !self.start_dialog {
            return;
        }
        let classic = self.shell.classic || self.shell.failed.is_some();
        if self.shell.web.is_none() && !classic {
            return;
        }
        self.start_dialog = false;
        self.open_new_project(ctx);
    }
}
