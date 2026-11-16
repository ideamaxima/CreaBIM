use super::*;
use creabim_core::{annotation::Note, ElementId, Model};

#[test]
fn moving_a_note_far_off_linework_respects_its_leader_setting() {
    for leader in [None, Some(DVec2::new(-110.0, -20.0))] {
        let model = Model::new();
        let note = Note::new(ElementId::NIL, DVec2::ZERO, "PLAN");
        let id = ElementId::new();
        let mut items = vec![crate::annotate::note_item(&model, id, &note, leader, 1.0)];
        for y in -10..=10 {
            items.push(PlanItem::Line {
                id: ElementId::new(),
                pen: Pen::Grid,
                a: DVec2::new(-100.0, y as f64),
                b: DVec2::new(100.0, y as f64),
            });
        }
        place_labels(&mut items, 1.0);
        let PlanItem::Annotation { item, .. } = &items[0] else {
            panic!("the note")
        };
        let Shape::Note {
            at, leader: shown, ..
        } = item.shape
        else {
            panic!("the note shape")
        };
        assert!(
            at.length() > LEADER_PAST,
            "the note must move far enough to trigger the regression: {at}"
        );
        assert_eq!(
            shown, leader,
            "automatic placement must respect the note's leader setting"
        );
        if leader.is_none() {
            let marks = item.marks(1.0);
            assert!(marks.lines.is_empty());
            assert!(marks.arrows.is_empty());
        }
    }
}
