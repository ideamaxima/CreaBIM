//! Starting a project: the empty project, the New project window shown
//! when the app opens, and its recent files.

use super::*;
use serde_json::{json, Value};

/// Recent files shown before "Show all".
const RECENT_SHOWN: usize = 6;

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

    /// The recent files as New project cards: the project's name, its
    /// full path, and whether it is still there.
    pub(super) fn recent_cards(&self) -> Vec<Value> {
        self.recent
            .files
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let p = std::path::Path::new(f);
                let title = p
                    .file_stem()
                    .map_or(f.clone(), |n| n.to_string_lossy().into_owned());
                let folder = p
                    .parent()
                    .map_or(String::new(), |d| short_folder(&d.to_string_lossy()));
                json!({"id": format!("recent:{i}"), "title": title, "line": folder,
                       "path": f, "icon": "house", "gone": !p.exists()})
            })
            .collect()
    }

    /// The New project window's height: taller with the recent files.
    pub(super) fn new_project_height(&self) -> f32 {
        match self.recent.files.len() {
            0 => 520.0,
            n if n <= RECENT_SHOWN / 2 => 640.0,
            _ => 720.0,
        }
    }

    /// Open… in New project: pick a file; nothing picked brings the window
    /// back.
    pub(super) fn open_from_new_project(&mut self, ctx: &egui::Context) {
        match crate::files::pick_open() {
            Some(path) => self.open_path(&path),
            None => self.open_new_project(ctx),
        }
    }
}

/// Folders longer than this lose their middle.
const FOLDER_CHARS: usize = 34;

/// A folder on one line: the home folder as `~`, and a long path cut in the
/// middle so its first folder and its last folders stay, `/mnt/…/Disegni/CAD`.
fn short_folder(dir: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = match dir.strip_prefix(home.as_str()) {
        Some(rest) if !home.is_empty() && (rest.is_empty() || rest.starts_with('/')) => {
            format!("~{rest}")
        }
        _ => dir.to_string(),
    };
    if dir.chars().count() <= FOLDER_CHARS {
        return dir;
    }
    let parts: Vec<&str> = dir.split('/').collect();
    // the first folder, then the last folders that fit
    let head = match parts.first() {
        Some(&"") => format!("/{}", parts.get(1).unwrap_or(&"")),
        Some(first) => first.to_string(),
        None => String::new(),
    };
    let room = FOLDER_CHARS.saturating_sub(head.chars().count() + 1);
    let mut tail = String::new();
    for part in parts.iter().skip(2).rev() {
        let next = format!("/{part}{tail}");
        if next.chars().count() > room && !tail.is_empty() {
            break;
        }
        tail = next;
    }
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::short_folder;

    #[test]
    fn long_folders_keep_their_start_and_last_folders() {
        assert_eq!(short_folder("/tmp/a"), "/tmp/a");
        let s = short_folder(
            "/mnt/20DC8641DC8610E8/Archivio/Clienti/Zambaldo Mario/Pratiche/Via Tiro a Segno/Disegni/CAD",
        );
        assert!(s.starts_with("/mnt…/"), "{s}");
        assert!(s.ends_with("/Disegni/CAD"), "{s}");
        assert!(s.contains('…'), "{s}");
        assert!(s.chars().count() <= 34, "{s}");
    }
}
