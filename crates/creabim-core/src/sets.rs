//! Sets: groups the user makes and names, across every category (a wall, a
//! slab and a chair can share one). An element can be in several; a view
//! leaves it out when any of its sets is switched off there. Sets sit in
//! folders, which can sit in folders and can be empty.

use crate::{Element, ElementId, ElementKind, Model, Transaction, ViewHidden};
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

/// A combination: a named state of the sets, as ArchiCAD's layer
/// combinations. Its name is the element's name. Views that follow it
/// switch off just its sets; change it and they all change.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Combination {
    /// The sets it switches off; the others are on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_sets: Vec<ElementId>,
}

/// The project's combinations, by name.
pub fn combinations(model: &Model) -> Vec<(ElementId, &str)> {
    let mut out: Vec<_> = model
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Combination(_)))
        .map(|e| (e.id, e.name.as_str()))
        .collect();
    out.sort_by(|a, b| a.1.cmp(b.1));
    out
}

/// A name not used yet: `base`, else `base 2`, `base 3`...
fn free_name(model: &Model, base: &str) -> String {
    let taken = |n: &str| model.iter().any(|e| e.name == n);
    let mut name = base.to_string();
    let mut n = 2;
    while taken(&name) {
        name = format!("{base} {n}");
        n += 1;
    }
    name
}

/// A new combination named `name` holding `view`'s sets as they are now;
/// `view` then follows it. Returns the transaction and the combination.
pub fn new_combination(model: &Model, name: &str, view: ElementId) -> (Transaction, ElementId) {
    let hidden_sets = match model.get(view).map(|e| &e.kind) {
        Some(ElementKind::View(v)) => v.hidden.sets.clone(),
        _ => vec![],
    };
    let c = Element::new(
        free_name(model, name),
        ElementKind::Combination(Combination { hidden_sets }),
    );
    let id = c.id;
    let tx = Transaction::new("New combination").insert(c);
    let mut next = model.clone();
    let mut tx = tx;
    if tx.apply(&mut next).is_ok() {
        tx.ops.extend(apply_combination(&next, id, view).ops);
    }
    tx.label = "New combination".into();
    (tx, id)
}

/// Make `view` follow `combination`: its sets become the combination's.
pub fn apply_combination(model: &Model, combination: ElementId, view: ElementId) -> Transaction {
    let tx = Transaction::new("Apply combination");
    let Some(ElementKind::Combination(c)) = model.get(combination).map(|e| &e.kind) else {
        return tx;
    };
    let Some(e) = model.get(view) else { return tx };
    let mut after = e.clone();
    let ElementKind::View(v) = &mut after.kind else {
        return tx;
    };
    v.hidden.sets = c.hidden_sets.clone();
    v.hidden.combination = Some(combination);
    if after == *e {
        return tx;
    }
    tx.replace(e.clone(), after)
}

/// The same sets, whatever their order.
pub fn same_sets(a: &[ElementId], b: &[ElementId]) -> bool {
    a.len() == b.len() && a.iter().all(|s| b.contains(s))
}

/// `view` follows a combination but its sets were switched by hand since.
pub fn modified(model: &Model, view: &ViewHidden) -> bool {
    match view.combination.and_then(|c| model.get(c)).map(|e| &e.kind) {
        Some(ElementKind::Combination(c)) => !same_sets(&c.hidden_sets, &view.sets),
        _ => false,
    }
}

/// A copy of `combination`, named "<name> copy". Returns the transaction
/// and the copy.
pub fn duplicate_combination(
    model: &Model,
    combination: ElementId,
) -> Option<(Transaction, ElementId)> {
    let e = model.get(combination)?;
    let ElementKind::Combination(c) = &e.kind else {
        return None;
    };
    let copy = Element::new(
        free_name(model, &format!("{} copy", e.name)),
        ElementKind::Combination(c.clone()),
    );
    let id = copy.id;
    Some((Transaction::new("Duplicate combination").insert(copy), id))
}

/// Store `view`'s sets as they are now in `combination`; `view` and the
/// views following it unmodified take them, the modified ones are left
/// as they are.
pub fn update_combination(model: &Model, combination: ElementId, view: ElementId) -> Transaction {
    let tx = Transaction::new("Update combination");
    let Some(ElementKind::View(v)) = model.get(view).map(|e| &e.kind) else {
        return tx;
    };
    let hidden_sets = v.hidden.sets.clone();
    let Some(e) = model.get(combination) else {
        return tx;
    };
    let ElementKind::Combination(c) = &e.kind else {
        return tx;
    };
    let old = c.hidden_sets.clone();
    let mut tx = tx;
    if c.hidden_sets != hidden_sets {
        let mut after = e.clone();
        after.kind = ElementKind::Combination(Combination {
            hidden_sets: hidden_sets.clone(),
        });
        tx = tx.replace(e.clone(), after);
    }
    for m in model.iter() {
        let ElementKind::View(mv) = &m.kind else {
            continue;
        };
        let follows =
            mv.hidden.combination == Some(combination) && same_sets(&mv.hidden.sets, &old);
        if m.id != view && !follows {
            continue;
        }
        let mut after = m.clone();
        if let ElementKind::View(av) = &mut after.kind {
            av.hidden.sets = hidden_sets.clone();
            av.hidden.combination = Some(combination);
        }
        if after != *m {
            tx = tx.replace(m.clone(), after);
        }
    }
    tx
}

/// The views following `combination`, by name.
pub fn combination_views(model: &Model, combination: ElementId) -> Vec<String> {
    let mut out: Vec<String> = model
        .iter()
        .filter(|e| {
            matches!(&e.kind, ElementKind::View(v) if v.hidden.combination == Some(combination))
        })
        .map(|e| e.name.clone())
        .collect();
    out.sort();
    out
}

/// Give a combination a new name.
pub fn rename_combination(model: &Model, combination: ElementId, name: &str) -> Transaction {
    let tx = Transaction::new("Rename combination");
    match model.get(combination) {
        Some(e) if matches!(e.kind, ElementKind::Combination(_)) && e.name != name => {
            let mut after = e.clone();
            after.name = name.to_string();
            tx.replace(e.clone(), after)
        }
        _ => tx,
    }
}

/// Delete `combination`: views following it keep its sets as their own.
pub fn delete_combination(model: &Model, combination: ElementId) -> Transaction {
    let mut tx = Transaction::new("Delete combination");
    if !matches!(
        model.get(combination).map(|e| &e.kind),
        Some(ElementKind::Combination(_))
    ) {
        return tx;
    }
    for e in model.iter() {
        if let ElementKind::View(v) = &e.kind {
            if v.hidden.combination == Some(combination) {
                let mut after = e.clone();
                if let ElementKind::View(av) = &mut after.kind {
                    av.hidden.combination = None;
                }
                tx = tx.replace(e.clone(), after);
            }
        }
    }
    tx.remove_cascade(model, combination)
}

/// A copy of `view` named "<view> – <combination>", following
/// `combination`; `view` itself is left as it is. Returns the transaction
/// and the new view.
pub fn view_from_combination(
    model: &Model,
    combination: ElementId,
    view: ElementId,
) -> Option<(Transaction, ElementId)> {
    let c = model.get(combination)?;
    let ElementKind::Combination(k) = &c.kind else {
        return None;
    };
    let v = model.get(view)?;
    let ElementKind::View(_) = &v.kind else {
        return None;
    };
    // the view with what belongs to it (crop box, notes...), as Duplicate
    let elements: Vec<Element> = crate::transform::copy_set(model, &[view])
        .iter()
        .filter_map(|id| model.get(*id))
        .cloned()
        .collect();
    let at = elements.iter().position(|e| e.id == view)?;
    let (mut copies, _) = crate::transform::fresh_copies(model, &elements);
    let copy = &mut copies[at];
    copy.name = free_name(model, &format!("{} – {}", v.name, c.name));
    if let ElementKind::View(cv) = &mut copy.kind {
        cv.hidden.sets = k.hidden_sets.clone();
        cv.hidden.combination = Some(combination);
    }
    let id = copy.id;
    let mut tx = Transaction::new("New view from combination");
    for c in copies {
        tx = tx.insert(c);
    }
    Some((tx, id))
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
        match &mut after.kind {
            ElementKind::View(v) if v.hidden.sets.contains(&set) => v.hidden.sets.push(id),
            ElementKind::Combination(c) if c.hidden_sets.contains(&set) => c.hidden_sets.push(id),
            _ => {}
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
            ElementKind::Combination(c) => c.hidden_sets.retain(|s| *s != set),
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

    #[test]
    fn combinations_keep_the_working_view_whole_and_drive_their_views() {
        let a = new_set("Dimensions");
        let b = new_set("Furniture");
        let mut doc = crate::Document::from_model(sample_house());
        let plan = doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::View(_)))
            .unwrap()
            .clone();
        let mut whole = plan.clone();
        if let ElementKind::View(v) = &mut whole.kind {
            v.hidden = ViewHidden::default();
        }
        doc.commit(
            Transaction::new("x")
                .insert(a.clone())
                .insert(b.clone())
                .replace(plan.clone(), whole),
        )
        .unwrap();
        let view = |doc: &crate::Document, id| match &doc.model().get(id).unwrap().kind {
            ElementKind::View(v) => v.hidden.clone(),
            _ => unreachable!(),
        };
        // switch Dimensions off, save it as a combination
        let mut e = doc.model().get(plan.id).unwrap().clone();
        if let ElementKind::View(v) = &mut e.kind {
            v.hidden.set_set(a.id, true);
        }
        doc.commit(Transaction::new("off").replace(doc.model().get(plan.id).unwrap().clone(), e))
            .unwrap();
        let (tx, c) = new_combination(doc.model(), "Presentation", plan.id);
        doc.commit(tx).unwrap();
        assert_eq!(view(&doc, plan.id).combination, Some(c));
        // a switch by hand leaves it modified, still on the combination
        let mut e = doc.model().get(plan.id).unwrap().clone();
        if let ElementKind::View(v) = &mut e.kind {
            v.hidden.set_set(a.id, false);
        }
        doc.commit(Transaction::new("on").replace(doc.model().get(plan.id).unwrap().clone(), e))
            .unwrap();
        assert!(view(&doc, plan.id).sets.is_empty(), "whole again");
        assert_eq!(view(&doc, plan.id).combination, Some(c));
        assert!(modified(doc.model(), &view(&doc, plan.id)));
        // a new view from it; the working view stays whole
        let (tx, copy) = view_from_combination(doc.model(), c, plan.id).unwrap();
        doc.commit(tx).unwrap();
        assert_eq!(view(&doc, copy).sets, vec![a.id]);
        assert_eq!(view(&doc, copy).combination, Some(c));
        assert!(view(&doc, plan.id).sets.is_empty());
        assert!(doc
            .model()
            .get(copy)
            .unwrap()
            .name
            .ends_with("– Presentation"));
        assert_eq!(combination_views(doc.model(), c).len(), 2);
        // change the combination from a view: its views follow
        let mut e = doc.model().get(plan.id).unwrap().clone();
        if let ElementKind::View(v) = &mut e.kind {
            v.hidden.set_set(b.id, true);
        }
        doc.commit(Transaction::new("b").replace(doc.model().get(plan.id).unwrap().clone(), e))
            .unwrap();
        doc.commit(update_combination(doc.model(), c, plan.id))
            .unwrap();
        assert_eq!(view(&doc, copy).sets, vec![b.id]);
        assert!(!modified(doc.model(), &view(&doc, plan.id)));
        // a view switched by hand is left alone by an update
        let mut e = doc.model().get(copy).unwrap().clone();
        if let ElementKind::View(v) = &mut e.kind {
            v.hidden.set_set(b.id, false);
        }
        doc.commit(Transaction::new("b on").replace(doc.model().get(copy).unwrap().clone(), e))
            .unwrap();
        let mut e = doc.model().get(plan.id).unwrap().clone();
        if let ElementKind::View(v) = &mut e.kind {
            v.hidden.set_set(a.id, true);
        }
        doc.commit(Transaction::new("a").replace(doc.model().get(plan.id).unwrap().clone(), e))
            .unwrap();
        doc.commit(update_combination(doc.model(), c, plan.id))
            .unwrap();
        assert!(view(&doc, copy).sets.is_empty());
        assert!(modified(doc.model(), &view(&doc, copy)));
        let (tx, twin) = duplicate_combination(doc.model(), c).unwrap();
        doc.commit(tx).unwrap();
        assert_eq!(doc.model().get(twin).unwrap().name, "Presentation copy");
        doc.commit(delete_combination(doc.model(), twin)).unwrap();
        // a deleted set leaves the combination
        doc.commit(delete(doc.model(), b.id)).unwrap();
        let ElementKind::Combination(k) = &doc.model().get(c).unwrap().kind else {
            unreachable!()
        };
        assert_eq!(k.hidden_sets, vec![a.id]);
        // deleting the combination: views keep their sets, free
        doc.commit(rename_combination(doc.model(), c, "Print"))
            .unwrap();
        doc.commit(delete_combination(doc.model(), c)).unwrap();
        assert!(doc.model().get(c).is_none());
        assert_eq!(view(&doc, copy).combination, None);
        assert!(combinations(doc.model()).is_empty());
    }
}
