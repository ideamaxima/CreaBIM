//! Section line and extent grips, sharing the standard drag constraints.

use super::*;

pub(super) fn grips(start: DVec2, end: DVec2, depth: f64) -> Vec<Handle> {
    let along = (end - start).normalize_or_zero();
    let look = along.perp();
    let mut out = Vec::new();
    for (which, at) in [(false, start), (true, end)] {
        out.push(Handle {
            kind: HandleKind::ViewEnd { end: which },
            at: at.extend(0.0),
            constraint: Constraint::Plane {
                point: at.extend(0.0),
                normal: DVec3::Z,
            },
            nudge: 0.0,
        });
        let side = (at + look * depth / 2.0).extend(0.0);
        out.push(Handle {
            kind: HandleKind::ViewSide { end: which },
            at: side,
            constraint: Constraint::Line {
                point: side,
                dir: along.extend(0.0),
            },
            nudge: 0.0,
        });
    }
    let at = ((start + end) / 2.0 + look * depth).extend(0.0);
    out.push(Handle {
        kind: HandleKind::ViewDepth,
        at,
        constraint: Constraint::Line {
            point: at,
            dir: look.extend(0.0),
        },
        nudge: 0.0,
    });
    out
}

pub(super) fn resize(kind: &mut ViewKind, which: bool, by: DVec3) -> Result<(), String> {
    if let ViewKind::Section { start, end, .. } = kind {
        let along = (*end - *start).normalize_or_zero();
        let delta = along * by.truncate().dot(along);
        let (a, b) = if which {
            (*start, *end + delta)
        } else {
            (*start + delta, *end)
        };
        if (b - a).dot(along) < 0.2 {
            return Err("a view line needs some length".into());
        }
        (*start, *end) = (a, b);
    }
    Ok(())
}

pub(super) fn orthogonal(
    kind: Option<HandleKind>,
    original: Option<&Element>,
    now: DVec3,
    shift: bool,
    look: DVec3,
) -> DVec3 {
    if !shift || look.z.abs() < 0.99 {
        return now;
    }
    if let (
        Some(HandleKind::ViewEnd { end: which }),
        Some(Element {
            kind: ElementKind::View(v),
            ..
        }),
    ) = (kind, original)
    {
        if let ViewKind::Section { start, end, .. } = v.kind {
            let base = if which { start } else { end }.extend(now.z);
            return base
                + crate::modal::move_delta(
                    base,
                    now,
                    None,
                    Some([DVec3::X, DVec3::Y]),
                    None,
                    DVec3::X,
                    true,
                );
        }
    }
    now
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{Transaction, View};

    #[test]
    fn either_side_resizes_straight_and_diagonal_sections_and_undoes() {
        for end in [DVec2::new(8.0, 0.0), DVec2::new(6.0, 8.0)] {
            for which in [false, true] {
                let mut doc = Document::new();
                let element = Element::new(
                    "Section",
                    ElementKind::View(View::section(DVec2::ZERO, end, 4.0, true)),
                );
                let id = element.id;
                doc.commit(Transaction::new("Setup").insert(element.clone()))
                    .unwrap();
                let hs = handles(doc.model(), id, None);
                assert_eq!(hs.len(), 5);
                let grip = hs
                    .iter()
                    .find(|h| h.kind == HandleKind::ViewSide { end: which })
                    .unwrap();
                assert!(grip.kind.round());
                let along = end.normalize().extend(0.0);
                let Constraint::Line { dir, .. } = grip.constraint else {
                    panic!("a constrained side")
                };
                assert!((dir - along).length() < 1e-9);
                let delta = along * if which { -2.0 } else { 2.0 };
                let changed = apply(
                    doc.model(),
                    std::slice::from_ref(&element),
                    &[],
                    grip.kind,
                    grip.at,
                    grip.at + delta,
                    0.0,
                )
                .unwrap();
                let ElementKind::View(v) = &changed[0].kind else {
                    panic!()
                };
                let ViewKind::Section {
                    start: a,
                    end: b,
                    depth,
                    ..
                } = v.kind
                else {
                    panic!()
                };
                assert_eq!(depth, 4.0);
                assert!(((b - a).normalize() - end.normalize()).length() < 1e-9);
                assert!((a.distance(b) - (end.length() - 2.0)).abs() < 1e-9);
                if which {
                    assert_eq!(a, DVec2::ZERO);
                } else {
                    assert_eq!(b, end);
                }
                doc.edit(id, "Resize section", |e| *e = changed[0].clone())
                    .unwrap();
                assert!(doc.undo());
                assert_eq!(doc.model().get(id), Some(&element));
            }
        }
    }

    #[test]
    fn a_side_cannot_cross_the_other_or_collapse_the_section() {
        let mut v = View::section(DVec2::ZERO, DVec2::new(5.0, 0.0), 3.0, true);
        assert!(resize(&mut v.kind, false, DVec3::new(4.9, 0.0, 0.0)).is_err());
        assert!(resize(&mut v.kind, true, DVec3::new(-6.0, 0.0, 0.0)).is_err());
    }

    #[test]
    fn shift_uses_the_standard_orthogonal_move_from_the_opposite_end() {
        let e = Element::new(
            "Section",
            ElementKind::View(View::section(
                DVec2::new(2.0, 3.0),
                DVec2::new(8.0, 3.0),
                4.0,
                true,
            )),
        );
        let p = DVec3::new(5.0, 3.8, 0.0);
        for which in [false, true] {
            let k = Some(HandleKind::ViewEnd { end: which });
            assert_eq!(
                orthogonal(k, Some(&e), p, true, DVec3::Z),
                DVec3::new(5.0, 3.0, 0.0)
            );
            assert_eq!(orthogonal(k, Some(&e), p, false, DVec3::Z), p);
        }
        assert_eq!(
            orthogonal(Some(HandleKind::ViewDepth), Some(&e), p, true, DVec3::Z),
            p
        );
    }
}
