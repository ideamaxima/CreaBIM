//! Doors with one leaf or two, opening by their swing angle: the arcs in
//! plan, the leaves in 3D.

use creabim_core::*;
use creabim_geom::opening::{opening_boxes, Part};
use creabim_geom::{plan_view, PlanItem};
use glam::DVec2;

fn door(frame: OpeningFrame, swing_angle: f64) -> (Model, ElementId, Opening, Wall) {
    let mut doc = Document::new();
    let l = Element::new(
        "L0",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let wall = Wall::new(l.id, DVec2::ZERO, DVec2::new(6.0, 0.0), 0.2, 3.0);
    let w = Element::new("W", ElementKind::Wall(wall.clone()));
    let o = Opening {
        host: w.id,
        kind: OpeningKind::Door,
        offset: 2.0,
        sill: 0.0,
        width: 1.4,
        height: 2.1,
        material: None,
        frame,
        flip_hand: false,
        flip_facing: false,
        swing_angle,
    };
    let d = Element::new("D", ElementKind::Opening(o.clone()));
    let lid = l.id;
    doc.commit(Transaction::new("t").insert(l).insert(w).insert(d))
        .unwrap();
    (doc.model().clone(), lid, o, wall)
}

/// (radius, sweep) of each door arc in the plan.
fn arcs(m: &Model, level: ElementId) -> Vec<(f64, f64)> {
    plan_view(m, level, 1.2)
        .items
        .iter()
        .filter_map(|i| match i {
            PlanItem::Arc {
                radius, start, end, ..
            } => Some((*radius, end - start)),
            _ => None,
        })
        .collect()
}

fn two(main_leaf: f64) -> OpeningFrame {
    OpeningFrame {
        leaves: 2,
        main_leaf,
        ..Default::default()
    }
}

#[test]
fn one_leaf_at_a_right_angle_is_a_quarter_circle() {
    let (m, l, o, w) = door(OpeningFrame::default(), 90.0);
    let a = arcs(&m, l);
    assert_eq!(a.len(), 1);
    assert!((a[0].1 - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    let leaves = opening_boxes(&o, &w)
        .iter()
        .filter(|b| b.part == Part::Leaf)
        .count();
    assert_eq!(leaves, 1);
}

#[test]
fn two_leaves_the_same_sweep_half_the_width_each() {
    let (m, l, o, w) = door(two(0.0), 90.0);
    let a = arcs(&m, l);
    assert_eq!(a.len(), 2);
    assert!((a[0].0 - a[1].0).abs() < 1e-9, "{a:?}");
    assert!(a[0].0 < 0.7, "{a:?}");
    let leaves = opening_boxes(&o, &w)
        .iter()
        .filter(|b| b.part == Part::Leaf)
        .count();
    assert_eq!(leaves, 2);
}

#[test]
fn two_leaves_can_differ() {
    let (m, l, _, _) = door(two(0.9), 90.0);
    let mut r: Vec<f64> = arcs(&m, l).iter().map(|a| a.0).collect();
    r.sort_by(f64::total_cmp);
    assert!(r[1] - r[0] > 0.3, "{r:?}");
}

#[test]
fn a_main_leaf_of_half_the_door_is_two_the_same() {
    let r = |main| {
        let (m, l, _, _) = door(two(main), 90.0);
        let mut r: Vec<f64> = arcs(&m, l).iter().map(|a| a.0).collect();
        r.sort_by(f64::total_cmp);
        r
    };
    let (half, even) = (r(0.7), r(0.0));
    for (a, b) in half.iter().zip(&even) {
        assert!((a - b).abs() < 1e-9, "{half:?} {even:?}");
    }
}

#[test]
fn the_swing_angle_sets_each_arc() {
    for frame in [OpeningFrame::default(), two(0.0)] {
        let (m, l, _, _) = door(frame, 45.0);
        for (_, sweep) in arcs(&m, l) {
            assert!((sweep - std::f64::consts::FRAC_PI_4).abs() < 1e-9);
        }
    }
}

#[test]
fn a_door_saved_before_keeps_one_leaf_and_a_right_angle() {
    let (_, _, o, _) = door(OpeningFrame::default(), 90.0);
    let mut j = serde_json::to_value(&o).unwrap();
    j.as_object_mut().unwrap().remove("swing_angle");
    let back: Opening = serde_json::from_value(j).unwrap();
    assert_eq!(back.swing_angle, 90.0);
    assert_eq!(back.frame.leaves, 1);
}

#[test]
fn a_shut_door_has_no_arc_and_past_a_right_angle_stops_there() {
    for frame in [OpeningFrame::default(), two(0.0)] {
        let (m, l, _, _) = door(frame, 0.0);
        assert!(arcs(&m, l).is_empty());
        let (m, l, _, _) = door(frame, 120.0);
        for (_, sweep) in arcs(&m, l) {
            assert!((sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        }
    }
}

#[test]
fn the_main_leaf_and_its_handle_go_to_the_side_chosen() {
    use creabim_geom::opening::{door_hand, main_leaf_left, set_main_leaf_left};
    let handle_x = |o: &Opening, w: &Wall| {
        let hs: Vec<_> = opening_boxes(o, w)
            .into_iter()
            .filter(|b| b.part == Part::Handle)
            .collect();
        hs.iter().map(|b| b.lo.x + b.hi.x).sum::<f64>() / (2.0 * hs.len() as f64)
    };
    // two leaves the same: the side can still be chosen
    let (_, _, mut o, w) = door(two(0.0), 90.0);
    let left = main_leaf_left(&o, &w);
    let before = handle_x(&o, &w);
    set_main_leaf_left(&mut o, &w, !left);
    assert_eq!(main_leaf_left(&o, &w), !left);
    let after = handle_x(&o, &w);
    // by the meeting edge either way, on the other leaf
    assert!((before - after).abs() > 0.1, "{before} {after}");
    let side = if left { "right" } else { "left" };
    assert_eq!(door_hand(&o, &w), format!("Double, main {side}, in"));
    // choosing the side it is on already changes nothing
    set_main_leaf_left(&mut o, &w, !left);
    assert_eq!(main_leaf_left(&o, &w), !left);
}
