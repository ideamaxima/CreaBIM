//! A line pattern drawn on screen as it prints: its dashes, gaps and dots
//! at their paper size, so Dotted shows dots and Centre its long and short
//! dashes (they all showed as one plain dash before).

use eframe::egui::{self, Pos2, Stroke};

/// A pattern's dashes and gaps, paper mm, kept in a small fixed array so
/// a view's overrides stay cheap to copy; a dash of 0 is a dot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dashes {
    v: [f32; 8],
    n: usize,
}

impl Dashes {
    /// From a pattern's lengths; none when solid. Longer than eight runs
    /// keep their first eight pairs' worth.
    pub fn of(lengths: Option<Vec<f64>>) -> Option<Self> {
        let l = lengths?;
        let n = (l.len().min(8) / 2) * 2;
        if n < 2 {
            return None;
        }
        let mut v = [0.0; 8];
        for (i, x) in l.iter().take(n).enumerate() {
            v[i] = x.max(0.0) as f32;
        }
        Some(Dashes { v, n })
    }

    pub fn from_pattern(p: &creabim_core::styles::LinePattern) -> Option<Self> {
        Self::of(p.dashes())
    }
}

/// More runs than this on one segment are drawn as a plain line: too
/// dense to read anyway.
const MAX_RUNS: usize = 4000;

/// The pieces of `a`→`b` in pattern `d` at `px_mm` screen points per
/// paper mm: dashes as pairs of points, dots as one point (both the same).
pub fn pieces(a: Pos2, b: Pos2, d: &Dashes, px_mm: f32) -> Option<Vec<(Pos2, Pos2)>> {
    let len = a.distance(b);
    let period: f32 = d.v[..d.n].iter().sum::<f32>() * px_mm;
    if period < 2.0 || len / period * d.n as f32 > MAX_RUNS as f32 {
        return None;
    }
    let dir = (b - a) / len.max(1e-6);
    let mut out = vec![];
    let (mut t, mut i) = (0.0f32, 0usize);
    while t < len {
        let run = d.v[i] * px_mm;
        if i % 2 == 0 {
            let end = (t + run).min(len);
            out.push((a + dir * t, a + dir * end));
        }
        t += run;
        i = (i + 1) % d.n;
    }
    Some(out)
}

/// Draw `a`→`b` with `stroke` in pattern `d` (none: solid).
pub fn segment(
    painter: &egui::Painter,
    a: Pos2,
    b: Pos2,
    stroke: Stroke,
    d: Option<&Dashes>,
    px_mm: f32,
) {
    let Some(pieces) = d.and_then(|d| pieces(a, b, d, px_mm)) else {
        painter.line_segment([a, b], stroke);
        return;
    };
    let r = (stroke.width / 2.0).max(0.8);
    for (p, q) in pieces {
        if p.distance(q) < 0.5 {
            painter.circle_filled(p, r, stroke.color);
        } else {
            painter.line_segment([p, q], stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::styles::LinePattern;

    #[test]
    fn dotted_draws_dots_and_dashed_draws_dashes() {
        let (a, b) = (Pos2::ZERO, Pos2::new(100.0, 0.0));
        let dots = Dashes::from_pattern(&LinePattern::Dotted).unwrap();
        let p = pieces(a, b, &dots, 5.0).unwrap();
        assert!(p.len() > 10 && p.iter().all(|(p, q)| p.distance(*q) < 0.5));
        let dash = Dashes::from_pattern(&LinePattern::Dashed).unwrap();
        let p = pieces(a, b, &dash, 5.0).unwrap();
        assert!(p.iter().any(|(p, q)| p.distance(*q) > 5.0));
        assert!(Dashes::from_pattern(&LinePattern::Solid).is_none());
    }

    #[test]
    fn too_fine_to_read_it_is_drawn_plain() {
        let dash = Dashes::from_pattern(&LinePattern::Dashed).unwrap();
        assert!(pieces(Pos2::ZERO, Pos2::new(100.0, 0.0), &dash, 0.1).is_none());
    }
}
