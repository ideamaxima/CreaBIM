//! Sets: groups the user makes and names, across every category (a wall, a
//! slab and a chair can share one). An element can be in several; a view
//! leaves it out when any of its sets is switched off there. Sets sit in
//! folders, which can sit in folders and can be empty.

use crate::{Element, ElementId, ElementKind, Model, Transaction};
use serde::{Deserialize, Serialize};

/// A set: its name is the element's name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Set {
    /// The folder it is in; none: at the top.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<ElementId>,
}

/// A folder of sets: its name is the element's name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SetFolder {
    /// The folder it is in; none: at the top.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<ElementId>,
}

/// A new folder named `name` in `parent`, to insert into the model.
pub fn new_folder(name: &str, parent: Option<ElementId>) -> Element {
    Element::new(name, ElementKind::SetFolder(SetFolder { parent }))
}

/// The folder a set or folder sits in.
pub fn parent_of(model: &Model, id: ElementId) -> Option<ElementId> {
    match &model.get(id)?.kind {
        ElementKind::Set(s) => s.folder,
        ElementKind::SetFolder(f) => f.parent,
        _ => None,
    }
}

/// What sits directly in `folder` (none: the top): its folders, then its
/// sets, each by name.
pub fn children(model: &Model, folder: Option<ElementId>) -> (Vec<ElementId>, Vec<ElementId>) {
    let mut folders: Vec<&Element> = vec![];
    let mut sets: Vec<&Element> = vec![];
    for e in model.iter() {
        match &e.kind {
            ElementKind::SetFolder(f) if f.parent == folder => folders.push(e),
            ElementKind::Set(s) if s.folder == folder => sets.push(e),
            _ => {}
        }
    }
    folders.sort_by(|a, b| a.name.cmp(&b.name));
    sets.sort_by(|a, b| a.name.cmp(&b.name));
    (
        folders.iter().map(|e| e.id).collect(),
        sets.iter().map(|e| e.id).collect(),
    )
}

/// Every set in `folder` and its folders, all the way down.
pub fn sets_under(model: &Model, folder: ElementId) -> Vec<ElementId> {
    let (folders, mut out) = children(model, Some(folder));
    for f in folders {
        out.extend(sets_under(model, f));
    }
    out
}

/// The folder path of a set or folder, outermost first ("Phase 1/Temporary").
pub fn path(model: &Model, id: ElementId) -> String {
    let mut names = vec![];
    let mut at = Some(id);
    let mut guard = 0;
    while let Some(i) = at {
        let Some(e) = model.get(i) else { break };
        names.push(e.name.as_str());
        at = parent_of(model, i);
        guard += 1;
        if guard > 64 {
            break;
        }
    }
    names.reverse();
    names.join("/")
}

/// The elements in `set`.
pub fn members(model: &Model, set: ElementId) -> Vec<ElementId> {
    model
        .iter()
        .filter(|e| e.sets.contains(&set))
        .map(|e| e.id)
        .collect()
}

/// The views that switch `set` off, by name.
pub fn views_off(model: &Model, set: ElementId) -> Vec<String> {
    let mut out: Vec<String> = model
        .iter()
        .filter(|e| matches!(&e.kind, ElementKind::View(v) if v.hidden.sets.contains(&set)))
        .map(|e| e.name.clone())
        .collect();
    out.sort();
    out
}

/// Put the set or folder `id` in `folder` (none: the top). Refused (empty)
/// when `folder` is `id` or inside it.
pub fn move_to(model: &Model, id: ElementId, folder: Option<ElementId>) -> Transaction {
    let tx = Transaction::new("Move to folder");
    // no folder inside itself
    let mut at = folder;
    let mut guard = 0;
    while let Some(f) = at {
        if f == id || guard > 64 {
            return tx;
        }
        at = parent_of(model, f);
        guard += 1;
    }
    let Some(e) = model.get(id) else { return tx };
    let mut after = e.clone();
    match &mut after.kind {
        ElementKind::Set(s) => s.folder = folder,
        ElementKind::SetFolder(f) => f.parent = folder,
        _ => return tx,
    }
    if after == *e {
        return tx;
    }
    tx.replace(e.clone(), after)
}

/// Delete `folder`: what is in it moves up to its own folder.
pub fn delete_folder(model: &Model, folder: ElementId) -> Transaction {
    let mut tx = Transaction::new("Delete folder");
    let Some(ElementKind::SetFolder(f)) = model.get(folder).map(|e| &e.kind) else {
        return tx;
    };
    let up = f.parent;
    let (folders, sets) = children(model, Some(folder));
    for id in folders.into_iter().chain(sets) {
        tx.ops.extend(move_to(model, id, up).ops);
    }
    tx.remove_cascade(model, folder)
}

/// A copy of `set` holding the same elements, in the same folder, switched
/// off where it is. Returns the transaction and the new set.
pub fn duplicate(model: &Model, set: ElementId) -> Option<(Transaction, ElementId)> {
    let e = model.get(set)?;
    let ElementKind::Set(s) = &e.kind else {
        return None;
    };
    let names: Vec<&str> = model.iter().map(|e| e.name.as_str()).collect();
    let mut name = format!("{} copy", e.name);
    let mut n = 2;
    while names.contains(&name.as_str()) {
        name = format!("{} copy {n}", e.name);
        n += 1;
    }
    let copy = Element::new(name, ElementKind::Set(s.clone()));
    let id = copy.id;
    let mut tx = Transaction::new("Duplicate set").insert(copy);
    for m in model.iter() {
        let mut after = m.clone();
        if m.sets.contains(&set) {
            after.sets.push(id);
        }
        if let ElementKind::View(v) = &mut after.kind {
            if v.hidden.sets.contains(&set) {
                v.hidden.sets.push(id);
            }
        }
        if after != *m {
            tx = tx.replace(m.clone(), after);
        }
    }
    Some((tx, id))
}

/// Merge `from` into `into`: its elements join `into`, and `from` goes.
pub fn merge(model: &Model, from: ElementId, into: ElementId) -> Transaction {
    let tx = Transaction::new("Merge sets");
    let ok = |id| matches!(model.get(id).map(|e| &e.kind), Some(ElementKind::Set(_)));
    if from == into || !ok(from) || !ok(into) {
        return tx;
    }
    let mut tx = tx;
    for m in model.iter() {
        if m.sets.contains(&from) && !m.sets.contains(&into) {
            let mut after = m.clone();
            after.sets.push(into);
            tx = tx.replace(m.clone(), after);
        }
    }
    // the rest of the cleanup is deleting it, on the model as it will be
    let mut next = model.clone();
    if tx.apply(&mut next).is_err() {
        return Transaction::new("Merge sets");
    }
    tx.ops.extend(delete(&next, from).ops);
    tx.label = "Merge sets".into();
    tx
}

/// A new set named `name`, to insert into the model.
pub fn new_set(name: &str) -> Element {
    Element::new(name, ElementKind::Set(Set::default()))
}

/// The project's sets, by name.
pub fn sets(model: &Model) -> Vec<(ElementId, &str)> {
    let mut out: Vec<_> = model
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Set(_)))
        .map(|e| (e.id, e.name.as_str()))
        .collect();
    out.sort_by(|a, b| a.1.cmp(b.1));
    out
}

/// How many elements are in `set` and how many views switch it off: what
/// deleting it would touch.
pub fn uses(model: &Model, set: ElementId) -> (usize, usize) {
    let mut elements = 0;
    let mut views = 0;
    for e in model.iter() {
        if e.sets.contains(&set) {
            elements += 1;
        }
        if let ElementKind::View(v) = &e.kind {
            if v.hidden.sets.contains(&set) {
                views += 1;
            }
        }
    }
    (elements, views)
}

/// Put `ids` in `set` (`add`) or take them out of it. Elements already
/// where they should be are left alone.
pub fn assign(model: &Model, set: ElementId, ids: &[ElementId], add: bool) -> Transaction {
    let mut tx = Transaction::new(if add { "Add to set" } else { "Remove from set" });
    for id in ids {
        let Some(e) = model.get(*id) else { continue };
        if matches!(e.kind, ElementKind::Set(_)) || e.sets.contains(&set) == add {
            continue;
        }
        let mut after = e.clone();
        after.sets.retain(|s| *s != set);
        if add {
            after.sets.push(set);
        }
        tx = tx.replace(e.clone(), after);
    }
    tx
}

/// Give a set or folder a new name; what is in it and the views stay as
/// they are.
pub fn rename(model: &Model, set: ElementId, name: &str) -> Transaction {
    let tx = Transaction::new("Rename set");
    match model.get(set) {
        Some(e)
            if matches!(e.kind, ElementKind::Set(_) | ElementKind::SetFolder(_))
                && e.name != name =>
        {
            let mut after = e.clone();
            after.name = name.to_string();
            tx.replace(e.clone(), after)
        }
        _ => tx,
    }
}

/// Delete `set`: its elements stay, out of it, and views and view
/// templates forget it.
pub fn delete(model: &Model, set: ElementId) -> Transaction {
    let mut tx = Transaction::new("Delete set");
    if !matches!(model.get(set).map(|e| &e.kind), Some(ElementKind::Set(_))) {
        return tx;
    }
    for e in model.iter() {
        let mut after = e.clone();
        after.sets.retain(|s| *s != set);
        match &mut after.kind {
            ElementKind::View(v) => v.hidden.sets.retain(|s| *s != set),
            ElementKind::StylePack(p) => {
                for t in &mut p.templates {
                    t.hidden_sets.retain(|s| *s != set);
                }
            }
            _ => {}
        }
        if after != *e {
            tx = tx.replace(e.clone(), after);
        }
    }
    tx.remove_cascade(model, set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::sample_house;
    use crate::visibility::ViewHidden;

    #[test]
    fn a_set_holds_elements_of_any_category() {
        let mut m = sample_house();
        let s = new_set("Temporary");
        m.insert(s.clone()).unwrap();
        let wall = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Wall(_)))
            .unwrap()
            .id;
        let slab = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Slab(_)))
            .unwrap()
            .id;
        for id in [wall, slab] {
            let mut e = m.get(id).unwrap().clone();
            e.sets.push(s.id);
            m.replace(e).unwrap();
        }
        let mut h = ViewHidden::default();
        assert!(h.hidden_set(&m).is_empty());
        h.set_set(s.id, true);
        let hidden = h.hidden_set(&m);
        assert!(hidden.contains(&wall) && hidden.contains(&slab));
        // the wall's doors and windows go with it, other walls stay
        assert!(m.dependents(wall).iter().all(|d| {
            !matches!(m.get(*d).unwrap().kind, ElementKind::Opening(_)) || hidden.contains(d)
        }));
        let other = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Wall(_)) && e.id != wall)
            .unwrap();
        assert!(!hidden.contains(&other.id));
        assert_eq!(uses(&m, s.id), (2, 0));
        h.set_set(s.id, false);
        assert!(h.is_empty());
    }

    #[test]
    fn an_element_in_several_sets_hides_when_any_is_off() {
        let mut m = sample_house();
        let a = new_set("Phase 1/Temporary");
        let b = new_set("Furniture to move");
        m.insert(a.clone()).unwrap();
        m.insert(b.clone()).unwrap();
        let mut wall = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Wall(_)))
            .unwrap()
            .clone();
        wall.sets = vec![a.id, b.id];
        m.replace(wall.clone()).unwrap();
        let mut h = ViewHidden::default();
        h.set_set(b.id, true);
        assert!(h.hidden_set(&m).contains(&wall.id));
        h.set_set(b.id, false);
        h.set_set(a.id, true);
        assert!(h.hidden_set(&m).contains(&wall.id));
        assert_eq!(sets(&m).len(), 2);
    }

    #[test]
    fn deleting_a_set_frees_its_elements_and_views() {
        let mut doc = crate::Document::new();
        let lvl = Element::new(
            "L0",
            ElementKind::Level(crate::Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        doc.commit(Transaction::new("l").insert(lvl.clone()))
            .unwrap();
        let s = new_set("Temporary");
        doc.commit(Transaction::new("s").insert(s.clone())).unwrap();
        let tx = assign(doc.model(), s.id, &[lvl.id], true);
        doc.commit(tx).unwrap();
        assert_eq!(doc.model().get(lvl.id).unwrap().sets, vec![s.id]);
        // adding twice changes nothing
        assert!(assign(doc.model(), s.id, &[lvl.id], true).ops.is_empty());
        doc.commit(rename(doc.model(), s.id, "Phase 1/Temporary"))
            .unwrap();
        assert_eq!(doc.model().get(s.id).unwrap().name, "Phase 1/Temporary");
        doc.commit(delete(doc.model(), s.id)).unwrap();
        assert!(doc.model().get(s.id).is_none());
        assert!(doc.model().get(lvl.id).unwrap().sets.is_empty());
    }

    #[test]
    fn folders_nest_hold_sets_and_give_them_up_when_deleted() {
        let mut doc = crate::Document::new();
        let top = new_folder("Phase 1", None);
        let inner = new_folder("Temporary", Some(top.id));
        let mut s = new_set("Scaffolding");
        let empty = new_folder("Empty", None);
        doc.commit(
            Transaction::new("x")
                .insert(top.clone())
                .insert(inner.clone())
                .insert(empty.clone()),
        )
        .unwrap();
        if let ElementKind::Set(k) = &mut s.kind {
            k.folder = Some(inner.id);
        }
        doc.commit(Transaction::new("s").insert(s.clone())).unwrap();
        let m = doc.model();
        assert_eq!(path(m, s.id), "Phase 1/Temporary/Scaffolding");
        assert_eq!(sets_under(m, top.id), vec![s.id]);
        assert_eq!(children(m, None).0.len(), 2, "the empty folder stays");
        // a folder can't go inside itself
        assert!(move_to(m, top.id, Some(inner.id)).ops.is_empty());
        doc.commit(delete_folder(doc.model(), inner.id)).unwrap();
        assert_eq!(parent_of(doc.model(), s.id), Some(top.id));
        doc.commit(move_to(doc.model(), s.id, None)).unwrap();
        assert_eq!(path(doc.model(), s.id), "Scaffolding");
    }

    #[test]
    fn duplicating_and_merging_sets_keep_their_elements() {
        let mut doc = crate::Document::new();
        let lvl = Element::new(
            "L0",
            ElementKind::Level(crate::Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let a = new_set("A");
        let b = new_set("B");
        doc.commit(
            Transaction::new("x")
                .insert(lvl.clone())
                .insert(a.clone())
                .insert(b.clone()),
        )
        .unwrap();
        doc.commit(assign(doc.model(), a.id, &[lvl.id], true))
            .unwrap();
        let (tx, c) = duplicate(doc.model(), a.id).unwrap();
        doc.commit(tx).unwrap();
        assert_eq!(members(doc.model(), c), vec![lvl.id]);
        assert_eq!(doc.model().get(c).unwrap().name, "A copy");
        doc.commit(merge(doc.model(), a.id, b.id)).unwrap();
        assert!(doc.model().get(a.id).is_none());
        assert_eq!(members(doc.model(), b.id), vec![lvl.id]);
        assert_eq!(doc.model().get(lvl.id).unwrap().sets.len(), 2);
    }

    #[test]
    fn elements_from_older_files_read_in_no_set() {
        let mut m = sample_house();
        let e = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Wall(_)))
            .unwrap()
            .clone();
        let mut json = serde_json::to_value(&e).unwrap();
        assert!(json.get("sets").is_none(), "no sets, nothing saved");
        json.as_object_mut().unwrap().remove("sets");
        let back: Element = serde_json::from_value(json).unwrap();
        assert!(back.sets.is_empty());
        // and with a set it round-trips
        let s = new_set("A");
        m.insert(s.clone()).unwrap();
        let mut e2 = e.clone();
        e2.sets.push(s.id);
        let back: Element = serde_json::from_str(&serde_json::to_string(&e2).unwrap()).unwrap();
        assert_eq!(back.sets, vec![s.id]);
    }
}
