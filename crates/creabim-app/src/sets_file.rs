//! Portable set and folder structure, independent of project element IDs.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SetFile {
    version: u32,
    folders: Vec<Folder>,
    sets: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Folder {
    name: String,
    folders: Vec<Folder>,
    sets: Vec<String>,
}

fn contents(model: &Model, parent: Option<ElementId>) -> (Vec<Folder>, Vec<String>) {
    let (folders, sets_) = sets::children(model, parent);
    let folders = folders
        .into_iter()
        .filter_map(|id| {
            let name = model.get(id)?.name.clone();
            let (folders, sets) = contents(model, Some(id));
            Some(Folder {
                name,
                folders,
                sets,
            })
        })
        .collect();
    let sets = sets_
        .into_iter()
        .filter_map(|id| model.get(id).map(|e| e.name.clone()))
        .collect();
    (folders, sets)
}

fn validate(file: &SetFile) -> Result<(), String> {
    fn walk(
        folders: &[Folder],
        sets: &[String],
        depth: usize,
        names: &mut [HashSet<String>; 2],
    ) -> Result<(), String> {
        if depth > 64 {
            return Err("Folders are nested too deeply".into());
        }
        for (items, kind) in [
            (folders.iter().map(|f| &f.name).collect::<Vec<_>>(), 0),
            (sets.iter().collect(), 1),
        ] {
            for name in items {
                if name.trim().is_empty() || !names[kind].insert(name.trim().to_lowercase()) {
                    return Err("Empty or duplicate names in the file".into());
                }
            }
        }
        for folder in folders {
            walk(&folder.folders, &folder.sets, depth + 1, names)?;
        }
        Ok(())
    }
    if file.version != 1 {
        return Err("Unsupported Sets file version".into());
    }
    walk(
        &file.folders,
        &file.sets,
        0,
        &mut [HashSet::new(), HashSet::new()],
    )
}

fn insert_tree(
    tx: &mut Transaction,
    folders: &[Folder],
    names: &[String],
    parent: Option<ElementId>,
) {
    for name in names {
        let mut e = sets::new_set(name);
        if let ElementKind::Set(s) = &mut e.kind {
            s.folder = parent;
        }
        tx.ops.extend(Transaction::new("set").insert(e).ops);
    }
    for folder in folders {
        let e = sets::new_folder(&folder.name, parent);
        let id = e.id;
        tx.ops.extend(Transaction::new("folder").insert(e).ops);
        insert_tree(tx, &folder.folders, &folder.sets, Some(id));
    }
}

impl CreaBimApp {
    pub(super) fn save_sets_file(&mut self) {
        let name = self.sets.save_name.trim();
        let filename = if name.is_empty() { "sets" } else { name };
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save sets")
            .set_file_name(format!("{filename}.json"))
            .add_filter("Sets", &["json"])
            .save_file()
        else {
            return;
        };
        let (folders, sets) = contents(self.doc.model(), None);
        let file = SetFile {
            version: 1,
            folders,
            sets,
        };
        let result = serde_json::to_string_pretty(&file)
            .map_err(|e| e.to_string())
            .and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string()));
        self.report(result, format!("Saved {}.", path.display()));
    }

    pub(super) fn load_sets_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Load sets")
            .add_filter("Sets", &["json"])
            .pick_file()
        else {
            return;
        };
        let result = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<SetFile>(&s).map_err(|e| e.to_string()))
            .and_then(|file| {
                validate(&file)?;
                Ok(file)
            });
        match result {
            Ok(file) => {
                self.sets.pending_load = Some(file);
                if !self.doc.model().iter().any(|e| {
                    matches!(
                        e.kind,
                        ElementKind::Set(_)
                            | ElementKind::SetFolder(_)
                            | ElementKind::Combination(_)
                    )
                }) {
                    self.replace_sets_file();
                }
            }
            Err(e) => self.status = format!("Couldn't read {}: {e}", path.display()),
        }
    }

    pub(super) fn replace_sets_file(&mut self) {
        let Some(file) = self.sets.pending_load.take() else {
            return;
        };
        let mut model = self.doc.model().clone();
        let mut tx = Transaction::new("Load sets");
        // Apply each existing deletion to a temporary model so later operations
        // see the updated references; commit the combined transaction once.
        for kind in 0..3 {
            let ids: Vec<_> = model
                .iter()
                .filter(|e| match kind {
                    0 => matches!(e.kind, ElementKind::Combination(_)),
                    1 => matches!(e.kind, ElementKind::Set(_)),
                    _ => matches!(e.kind, ElementKind::SetFolder(_)),
                })
                .map(|e| e.id)
                .collect();
            for id in ids {
                let part = match kind {
                    0 => sets::delete_combination(&model, id),
                    1 => sets::delete(&model, id),
                    _ => sets::delete_folder(&model, id),
                };
                if let Err(e) = part.apply(&mut model) {
                    self.status = format!("Couldn't load sets: {e}");
                    return;
                }
                tx.ops.extend(part.ops);
            }
        }
        insert_tree(&mut tx, &file.folders, &file.sets, None);
        let r = self.doc.commit(tx);
        self.report(r, "Sets loaded.");
    }
}
