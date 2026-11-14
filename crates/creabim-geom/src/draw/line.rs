//! A drawn line ([`creabim_core::draw::DraftLine`]) as drawing items: the
//! line, and its end marks as the dimensions and notes draw theirs. Its
//! own pen, pattern and colour are applied by [`crate::looks`], as a
//! view's override for one element is.

use crate::annotation::{end_marks, END_SIZE};
use crate::plan::{Pen, PlanItem};
use creabim_core::draw::DraftLine;
use creabim_core::{ElementId, Terminator};

/// The items of line `l`; `mm` is drawing units per paper millimetre.
pub fn line_items(id: ElementId, l: &DraftLine, mm: f64) -> Vec<PlanItem> {
    let mut out = vec![PlanItem::Line {
        id,
        pen: Pen::Projection,
        a: l.a,
        b: l.b,
    }];
    let dir = l.b - l.a;
    for (t, tip, d) in [(l.start, l.a, -dir), (l.end, l.b, dir)] {
        if t == Terminator::None {
            continue;
        }
        let ends = end_marks(t, tip, d, END_SIZE, mm);
        for run in ends.thin.iter().chain(&ends.heavy) {
            for w in run.windows(2) {
                out.push(PlanItem::Line {
                    id,
                    pen: Pen::Projection,
                    a: w[0],
                    b: w[1],
                });
            }
        }
        for f in ends.fills {
            out.push(PlanItem::Polygon {
                id,
                pen: Pen::Cut,
                points: f,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::draw::line::LineLook;
    use glam::DVec2;

    #[test]
    fn a_plain_line_is_one_item_and_marks_add_theirs() {
        let mut l = DraftLine::new(
            ElementId::new(),
            DVec2::ZERO,
            DVec2::new(2.0, 0.0),
            &LineLook::default(),
        );
        assert_eq!(line_items(ElementId::new(), &l, 0.01).len(), 1);
        l.end = Terminator::Arrow;
        let items = line_items(ElementId::new(), &l, 0.01);
        assert!(items.iter().any(|i| matches!(i, PlanItem::Polygon { .. })));
        l.start = Terminator::Slash;
        assert!(line_items(ElementId::new(), &l, 0.01).len() >= 3);
    }
}
