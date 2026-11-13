//! Sets: groups the user makes and names, across every category (a wall, a
//! slab and a chair can share one). An element can be in several; a view
//! leaves it out when any of its sets is switched off there. A "/" in a
//! set's name puts it in a folder ("Phase 1/Temporary").

use crate::{Element, ElementId, ElementKind, Model};
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
