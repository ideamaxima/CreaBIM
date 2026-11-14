//! The Guide tool's own geometry: where a guide goes from its first
//! click, an edge or a point, and a typed distance. Plans and the 3D view
//! on the level both use it.

use glam::DVec2;

/// The guide the Guide tool makes from its first click `a` to the pointer
/// `p`, as two points on it. From an edge (`edge`) it is parallel to the
/// edge, through the pointer, or `typed` metres from the edge on the
/// pointer's side. From a point with a typed distance it is square to the
/// way the pointer went (across: a plumb guide; up or down: a level one),
/// that far from the point. From a point without one it runs through the
/// two points. `None` when the two points are one.
pub fn guide_line(
    a: DVec2,
    p: DVec2,
    edge: Option<(DVec2, DVec2)>,
    typed: Option<f64>,
) -> Option<(DVec2, DVec2)> {
    match (edge, typed) {
        (Some((e0, e1)), typed) => {
            let d = (e1 - e0).try_normalize()?;
            let through = match typed {
                Some(t) => {
                    let n = d.perp();
                    let side = if (p - e0).dot(n) < 0.0 { -1.0 } else { 1.0 };
                    e0 + n * side * t
                }
                None => p,
            };
            Some((through, through + d))
        }
        (None, Some(t)) => {
            let w = p - a;
            let side = |v: f64| if v < 0.0 { -1.0 } else { 1.0 };
            Some(if w.x.abs() >= w.y.abs() {
                let x = a.x + side(w.x) * t;
                (DVec2::new(x, a.y), DVec2::new(x, a.y + 1.0))
            } else {
                let y = a.y + side(w.y) * t;
                (DVec2::new(a.x, y), DVec2::new(a.x + 1.0, y))
            })
        }
        (None, None) => (a.distance(p) > 0.01).then_some((a, p)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_guide_distance_goes_to_the_pointer_s_side() {
        let v = DVec2::new;
        let edge = Some((v(0.0, 0.0), v(4.0, 0.0)));
        // from an edge: parallel, that far, on the pointer's side
        let (p, q) = guide_line(v(1.0, 0.0), v(2.0, 3.0), edge, Some(1.2)).unwrap();
        assert!((p.y - 1.2).abs() < 1e-9 && (q.y - 1.2).abs() < 1e-9);
        let (p, _) = guide_line(v(1.0, 0.0), v(2.0, -3.0), edge, Some(1.2)).unwrap();
        assert!((p.y + 1.2).abs() < 1e-9);
        // nothing typed: through the pointer, as before
        let (p, q) = guide_line(v(1.0, 0.0), v(2.0, 0.7), edge, None).unwrap();
        assert!((p.y - 0.7).abs() < 1e-9 && (q.y - 0.7).abs() < 1e-9);
        // from a point: pointing across gives a plumb guide that far
        let a = v(1.0, 1.0);
        let (p, q) = guide_line(a, v(-3.0, 1.5), None, Some(0.5)).unwrap();
        assert!((p.x - 0.5).abs() < 1e-9 && (q.x - 0.5).abs() < 1e-9);
        // pointing up gives a level one
        let (p, q) = guide_line(a, v(1.2, 4.0), None, Some(2.0)).unwrap();
        assert!((p.y - 3.0).abs() < 1e-9 && (q.y - 3.0).abs() < 1e-9);
        // two points, nothing typed: through both; one point: none
        assert_eq!(
            guide_line(a, v(3.0, 2.0), None, None),
            Some((a, v(3.0, 2.0)))
        );
        assert_eq!(guide_line(a, a, None, None), None);
    }
}
