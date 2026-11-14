//! The colour swatch every colour field shows. CreaBIM keeps colours in
//! sRGB (0..1, as `#rrggbb`), and egui's `color_edit_button_rgb` reads
//! and writes linear RGB: a colour picked there drew darker than its
//! swatch. This one hands the picker sRGB.

use eframe::egui::{self, ecolor::Hsva};

fn to_bytes(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn from_bytes(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| v as f32 / 255.0)
}

/// A swatch for sRGB colour `c` that opens the picker; `c` changes only
/// when the picker is used, so it never drifts by rounding. The picker's
/// hue and saturation are kept between frames while the colour is the
/// one it gave: rebuilt from the rounded colour every frame, a grey has no
/// hue and the drag jumped when it left it.
pub fn srgb_button(ui: &mut egui::Ui, c: &mut [f32; 3]) -> egui::Response {
    let bytes = to_bytes(*c);
    let id = ui.next_auto_id().with("srgb");
    let kept: Option<(Hsva, [u8; 3])> = ui.data(|d| d.get_temp(id));
    let mut hsva = kept
        .filter(|(_, b)| *b == bytes)
        .map_or_else(|| Hsva::from_srgb(bytes), |(h, _)| h);
    let r = ui.color_edit_button_hsva(&mut hsva);
    let now = hsva.to_srgb();
    ui.data_mut(|d| d.insert_temp(id, (hsva, now)));
    if r.changed() && now != bytes {
        *c = from_bytes(now);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_swatch_shows_the_colour_drawn() {
        // a mid grey drawn as 128: the old linear swatch showed it as 188
        let grey = [0.5f32; 3];
        assert_eq!(to_bytes(grey), [128; 3]);
        assert_eq!(
            egui::Color32::from(egui::Rgba::from_rgb(0.5, 0.5, 0.5)).r(),
            188,
            "egui reads a linear 0.5 as sRGB 188: the bug this fixes"
        );
        let back = from_bytes(to_bytes(grey));
        assert!((back[0] - 0.5).abs() < 0.003);
        // the picker's own colour space keeps the grey
        assert_eq!(Hsva::from_srgb([128; 3]).to_srgb(), [128; 3]);
    }
}
