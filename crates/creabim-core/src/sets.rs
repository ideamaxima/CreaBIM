//! Sets: groups the user makes and names, across every category (a wall, a
//! slab and a chair can share one). An element can be in several; a view
//! leaves it out when any of its sets is switched off there. A "/" in a
//! set's name puts it in a folder ("Phase 1/Temporary").

use crate::{Element, ElementId, ElementKind, Model, Transaction};
use serde::{Deserialize, Serialize};

/// A set: its name is the element's name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Set {}

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

/// The folder part of a set's name ("Phase 1" for "Phase 1/Temporary").
pub fn folder(name: &str) -> Option<&str> {
    name.rsplit_once('/').map(|(f, _)| f)
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

/// Give `set` a new name; what is in it and the views stay as they are.
pub fn rename(model: &Model, set: ElementId, name: &str) -> Transaction {
    let tx = Transaction::new("Rename set");
    match model.get(set) {
        Some(e) if matches!(e.kind, ElementKind::Set(_)) && e.name != name => {
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
        assert_eq!(folder(&a.name), Some("Phase 1"));
        assert_eq!(folder(&b.name), None);
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
