//! One rule for every row of Properties and the settings windows: the
//! label on the left, the control on the right, every control lined up in
//! one column. On and off settings are switches; choices of four or more
//! are drop-downs; section headers are full-width rows with a chevron at
//! the right.

use eframe::egui::{self, Color32, Response, Sense, Ui, Vec2};

/// The label column's width for a row `width` wide: wide enough for most
/// labels, never more than 40% of the row.
pub fn label_width(width: f32) -> f32 {
    (width * 0.38).clamp(44.0, 160.0)
}

/// Narrower than this, a row stacks: its label above, its control the
/// full width below (A4-1). As Blender, rows keep one line down to a
/// narrow panel and cut their labels instead (MU22).
pub const STACK_BELOW: f32 = 110.0;

/// A section open on this layer: where its box starts, filled once the
/// next section starts or [`close_box`] is called (MU22, as Blender's
/// panels: each section a gentle box, a gap between boxes).
#[derive(Clone, Copy)]
struct OpenBox {
    shape: egui::layers::ShapeIdx,
    left: f32,
    right: f32,
    top: f32,
    header_bottom: f32,
    pass: u64,
}

fn box_id(ui: &Ui) -> egui::Id {
    egui::Id::new(("props.box", ui.layer_id()))
}

/// A small icon button, 20 px, the icon in the text colour, a tint when
/// hovered: for row tools (copy, browse, remove, reload), as Blender's.
pub fn icon_button(ui: &mut Ui, icon: &str, tip: &str) -> Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(r, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let ink = ui.visuals().text_color();
    crate::svgicon::paint(ui.painter(), r.shrink(3.0), icon, ink);
    resp.on_hover_text(tip)
}

/// A sub-section inside a section's box, folding as Blender's
/// (MU22): true while open. `key` keeps it folded or open.
pub fn subsection(ui: &mut Ui, key: &str, title: &str, default_open: bool) -> bool {
    let id = egui::Id::new(("props.sub", key));
    let open = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(id))
        .unwrap_or(default_open);
    let (resp, _) = header(ui, title, open, true);
    if resp.clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(id, !open));
        return !open;
    }
    open
}

/// A drop-down's arrow: the sections' thin chevron, pointing down (up
/// while open), not a filled triangle (MU22).
pub fn combo_icon(ui: &Ui, rect: egui::Rect, visuals: &egui::style::WidgetVisuals, open: bool) {
    let c = rect.center();
    let s = if open { -1.0 } else { 1.0 };
    let pts = [
        c + Vec2::new(-3.6, -1.8 * s),
        c + Vec2::new(0.0, 1.8 * s),
        c + Vec2::new(3.6, -1.8 * s),
    ];
    let stroke = egui::Stroke::new(1.3, visuals.fg_stroke.color);
    ui.painter().line_segment([pts[0], pts[1]], stroke);
    ui.painter().line_segment([pts[1], pts[2]], stroke);
}

/// The fill of a section's box: a tone off the panel.
pub fn box_fill(v: &egui::Visuals) -> Color32 {
    let p = v.panel_fill;
    let (to, t) = if v.dark_mode {
        (255.0, 0.045)
    } else {
        (0.0, 0.035)
    };
    let m = |c: u8| (c as f32 + (to - c as f32) * t).round() as u8;
    Color32::from_rgb(m(p.r()), m(p.g()), m(p.b()))
}

/// Draw sections as boxes on this layer from now on (`true`), or stop
/// (Properties turns them on round its sections).
pub fn boxes(ui: &mut Ui, on: bool) {
    let id = box_id(ui).with("on");
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data_mut(|d| {
        if on {
            d.insert_temp(id, pass);
        } else {
            d.remove::<u64>(id);
        }
    });
}

/// Is a section's box open to put a sub-section in? (A sub-section with
/// none above it is a section of its own: MU40.)
pub fn box_open(ui: &Ui) -> bool {
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx()
        .data(|d| d.get_temp::<OpenBox>(box_id(ui)))
        .is_some_and(|b| b.pass == pass)
}

fn boxes_on(ui: &Ui) -> bool {
    let id = box_id(ui).with("on");
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data(|d| d.get_temp::<u64>(id)) == Some(pass)
}

/// Close the open section's box, if any: it ends under its last row.
pub fn close_box(ui: &mut Ui) {
    let id = box_id(ui);
    let Some(b) = ui.ctx().data_mut(|d| {
        let got = d.get_temp::<OpenBox>(id);
        d.remove::<OpenBox>(id);
        got
    }) else {
        return;
    };
    if b.pass != ui.ctx().cumulative_pass_nr() {
        return;
    }
    let y = ui.cursor().top();
    // (the few pixels a header leaves under itself are not content)
    let content = y > b.header_bottom + ui.spacing().item_spacing.y + 1.0;
    if content {
        ui.add_space(4.0);
    }
    let bottom = if content { y + 4.0 } else { b.header_bottom };
    let v = ui.visuals();
    let rect = egui::Rect::from_min_max(
        egui::pos2(b.left - PAD, b.top),
        egui::pos2(b.right + PAD, bottom),
    );
    ui.painter()
        .set(b.shape, egui::Shape::rect_filled(rect, 4.0, box_fill(v)));
    ui.add_space(4.0);
}

/// A box reaches this far past the content column on each side, which
/// is the margin its rows keep inside it.
const PAD: f32 = 6.0;

/// (Rows needed an inset before boxes took a margin; none now.)
fn inset(_ui: &Ui) -> f32 {
    0.0
}

/// A header row (MU24): a short `label` from the far left, its control
/// taking the rest, as Blender's name field at the top of Properties.
pub fn head_row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width();
    let h = ui.spacing().interact_size.y;
    ui.allocate_ui_with_layout(
        Vec2::new(w, h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            let muted = ui.visuals().widgets.noninteractive.fg_stroke.color;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(58.0, h), Sense::hover());
            let mut job = egui::text::LayoutJob::single_section(
                label.to_string(),
                egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), muted),
            );
            job.wrap = egui::text::TextWrapping::truncate_at_width(54.0);
            let g = ui.painter().layout_job(job);
            ui.painter().galley(
                rect.left_center() - Vec2::new(0.0, g.size().y / 2.0),
                g,
                muted,
            );
            add(ui)
        },
    )
    .inner
}

/// A row: `label` at the left, in the label column, then `add`'s controls
/// filling the rest. A label too long for its column is cut with … and
/// shows whole when hovered.
pub fn row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    // a control's own text never wraps: a value too long is cut with …
    // (A4-1)
    let add = |ui: &mut Ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        add(ui)
    };
    if ui.available_width() < STACK_BELOW {
        return ui
            .vertical(|ui| {
                let muted = ui.visuals().widgets.noninteractive.fg_stroke.color;
                ui.add(egui::Label::new(egui::RichText::new(label).color(muted)).truncate());
                ui.horizontal(add).inner
            })
            .inner;
    }
    let w = ui.available_width() - inset(ui);
    let lw = label_width(w);
    let h0 = ui.spacing().interact_size.y;
    ui.allocate_ui_with_layout(
        Vec2::new(w, h0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            let h = ui.spacing().interact_size.y;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(lw, h), Sense::hover());
            let muted = ui.visuals().widgets.noninteractive.fg_stroke.color;
            let galley = ui.painter().layout(
                label.to_string(),
                egui::TextStyle::Body.resolve(ui.style()),
                muted,
                f32::INFINITY,
            );
            if galley.size().x > lw - 6.0 {
                // cut to fit, with the whole label on hover
                let mut job = egui::text::LayoutJob::single_section(
                    label.to_string(),
                    egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), muted),
                );
                job.wrap = egui::text::TextWrapping::truncate_at_width(lw - 6.0);
                let g = ui.painter().layout_job(job);
                ui.painter().galley(
                    rect.right_center() - Vec2::new(g.size().x + 6.0, g.size().y / 2.0),
                    g,
                    muted,
                );
                resp.on_hover_text(label);
            } else {
                // against its control, as Blender's
                ui.painter().galley(
                    rect.right_center() - Vec2::new(galley.size().x + 6.0, galley.size().y / 2.0),
                    galley,
                    muted,
                );
            }
            add(ui)
        },
    )
    .inner
}

/// Add a control at the full width left in the row, as fields and boxes
/// line up in one column.
pub fn fill(ui: &mut Ui, w: impl egui::Widget) -> Response {
    let h = ui.spacing().interact_size.y;
    ui.add_sized([ui.available_width().max(40.0), h], w)
}

/// A row of controls that moves whole controls to the next line when the
/// panel is narrow, never splitting a button's or a label's words (MU34:
/// "+ Picture" broke onto two lines).
pub fn flow<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    ui.horizontal_wrapped(|ui| {
        // cut with … rather than growing: Extend let one long label widen
        // the whole panel and push every field past its edge (MU36)
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        add(ui)
    })
}

/// A choice of a few as one row of equal joined buttons filling the field
/// column, the chosen one lit: Blender's expanded list (MU31). Returns
/// true when it changed.
/// One segment of a row of joined buttons: rounded at the row's ends.
fn segment_radius(i: usize, n: usize) -> egui::CornerRadius {
    let (first, last) = (i == 0, i + 1 == n);
    egui::CornerRadius {
        nw: if first { 4 } else { 0 },
        sw: if first { 4 } else { 0 },
        ne: if last { 4 } else { 0 },
        se: if last { 4 } else { 0 },
    }
}

/// As [`choice`], each option a picture rather than a word (MU39: words
/// cut to … in a narrow panel): `(value, icon, name, what it does)`.
pub fn choice_icons<T: PartialEq + Copy>(
    ui: &mut Ui,
    label: &str,
    value: &mut T,
    options: &[(T, &str, &str, &str)],
) -> bool {
    let mut changed = false;
    row(ui, label, |ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        let n = options.len().max(1);
        let w = ((ui.available_width() - (n as f32 - 1.0)) / n as f32).max(22.0);
        let h = ui.spacing().interact_size.y;
        for (i, (v, icon, name, tip)) in options.iter().enumerate() {
            let on = *value == *v;
            let mut b = egui::Button::selectable(on, "").corner_radius(segment_radius(i, n));
            if !on {
                b = b.fill(box_fill(ui.visuals()));
            }
            let resp = ui.add_sized([w, h], b);
            let ink = if on {
                ui.visuals().selection.stroke.color
            } else {
                crate::theme::Palette::of(ui.ctx()).muted
            };
            let r = egui::Rect::from_center_size(resp.rect.center(), Vec2::splat(15.0));
            crate::svgicon::paint(ui.painter(), r, icon, ink);
            if resp.on_hover_text(format!("{name}: {tip}")).clicked() && !on {
                *value = *v;
                changed = true;
            }
        }
    });
    changed
}

/// A row of joined on/off buttons, any number on at once (the sides a
/// modifier works on). True when one changed.
pub fn toggles(ui: &mut Ui, label: &str, on: &mut [bool], names: &[(&str, &str)]) -> bool {
    let mut changed = false;
    row(ui, label, |ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        let n = names.len().max(1);
        let w = ((ui.available_width() - (n as f32 - 1.0)) / n as f32).max(20.0);
        let h = ui.spacing().interact_size.y;
        for (i, (text, tip)) in names.iter().enumerate() {
            let Some(v) = on.get_mut(i) else { break };
            let mut b = egui::Button::selectable(*v, *text).corner_radius(segment_radius(i, n));
            if !*v {
                b = b.fill(box_fill(ui.visuals()));
            }
            if ui.add_sized([w, h], b).on_hover_text(*tip).clicked() {
                *v = !*v;
                changed = true;
            }
        }
    });
    changed
}

pub fn choice<T: PartialEq + Copy>(
    ui: &mut Ui,
    label: &str,
    value: &mut T,
    options: &[(T, &str, &str)],
) -> bool {
    let mut changed = false;
    row(ui, label, |ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        let n = options.len().max(1) as f32;
        let w = ((ui.available_width() - (n - 1.0)) / n).max(24.0);
        let h = ui.spacing().interact_size.y;
        for (i, (v, text, tip)) in options.iter().enumerate() {
            let on = *value == *v;
            let (first, last) = (i == 0, i + 1 == options.len());
            let r = egui::CornerRadius {
                nw: if first { 4 } else { 0 },
                sw: if first { 4 } else { 0 },
                ne: if last { 4 } else { 0 },
                se: if last { 4 } else { 0 },
            };
            // a label too long for its button is cut with …, its whole name
            // and what it does on hover, as Blender's (MU34)
            let mut b = egui::Button::selectable(on, *text)
                .corner_radius(r)
                .truncate();
            if !on {
                b = b.fill(box_fill(ui.visuals()));
            }
            let resp = ui.add_sized([w, h], b);
            let resp = if tip.is_empty() {
                resp.on_hover_text(*text)
            } else {
                resp.on_hover_text(format!("{text}: {tip}"))
            };
            if resp.clicked() && !on {
                *value = *v;
                changed = true;
            }
        }
    });
    changed
}

/// A button filling the field column after `label` (MU31: buttons stood
/// at the left, under the labels).
pub fn action(ui: &mut Ui, label: &str, text: &str, tip: &str, enabled: bool) -> Response {
    row(ui, label, |ui| {
        let h = ui.spacing().interact_size.y;
        let w = ui.available_width().max(40.0);
        let r = ui
            .add_enabled_ui(enabled, |ui| ui.add_sized([w, h], egui::Button::new(text)))
            .inner;
        if tip.is_empty() {
            r
        } else {
            r.on_hover_text(tip)
        }
    })
}

/// Two buttons sharing the field column, as Blender's paired operators.
/// Each is (text, tip, enabled); returns which was clicked.
pub fn actions2(
    ui: &mut Ui,
    label: &str,
    a: (&str, &str, bool),
    b: (&str, &str, bool),
) -> (bool, bool) {
    row(ui, label, |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let h = ui.spacing().interact_size.y;
        let w = ((ui.available_width() - 4.0) / 2.0).max(30.0);
        let go = |ui: &mut Ui, (t, tip, on): (&str, &str, bool)| {
            let r = ui
                .add_enabled_ui(on, |ui| ui.add_sized([w, h], egui::Button::new(t)))
                .inner;
            if tip.is_empty() {
                r.clicked()
            } else {
                r.on_hover_text(tip).clicked()
            }
        };
        let x = go(ui, a);
        (x, go(ui, b))
    })
}

/// A quiet line of explanation, starting in the field column under the
/// controls it explains (MU31).
pub fn note(ui: &mut Ui, text: impl Into<String>) {
    let w = ui.available_width() - inset(ui);
    let lw = if w < STACK_BELOW { 0.0 } else { label_width(w) };
    ui.horizontal(|ui| {
        ui.add_space(lw);
        ui.add(egui::Label::new(egui::RichText::new(text.into()).weak().small()).wrap());
    });
}

/// A drop-down list entry with a small picture before its label, painted
/// by `paint` in a square of the text's height.
pub fn picture_option(
    ui: &mut Ui,
    selected: bool,
    label: &str,
    paint: impl FnOnce(&egui::Painter, egui::Rect, egui::Stroke),
) -> Response {
    let h = ui.spacing().interact_size.y + 4.0;
    let w = ui.available_width().max(140.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        if selected || resp.hovered() {
            let fill = if selected {
                v.selection.bg_fill
            } else {
                v.widgets.hovered.weak_bg_fill
            };
            ui.painter().rect_filled(rect, 3.0, fill);
        }
        let ink = v.text_color();
        let pic = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 6.0, rect.top() + 3.0),
            Vec2::splat(h - 6.0),
        );
        paint(ui.painter(), pic, egui::Stroke::new(1.5, ink));
        ui.painter().text(
            egui::pos2(pic.right() + 8.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::TextStyle::Body.resolve(ui.style()),
            ink,
        );
    }
    resp
}

/// A plain heading inside a window, for grouping rows.
pub fn section(ui: &mut Ui, title: &str) {
    if boxes_on(ui) {
        // in Properties, a box of its own with the title as its band (MU22)
        close_box(ui);
        let shape = ui.painter().add(egui::Shape::Noop);
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::hover());
        let ink = ui.visuals().strong_text_color();
        let mut job = egui::text::LayoutJob::single_section(
            title.to_string(),
            egui::TextFormat::simple(egui::FontId::proportional(12.5), ink),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((w - 24.0).max(10.0));
        let g = ui.painter().layout_job(job);
        ui.painter().galley(
            egui::pos2(rect.left() + 1.0, rect.center().y - g.size().y / 2.0),
            g,
            ink,
        );
        let b = OpenBox {
            shape,
            left: rect.left(),
            right: rect.right(),
            top: rect.top(),
            header_bottom: rect.bottom(),
            pass: ui.ctx().cumulative_pass_nr(),
        };
        ui.ctx().data_mut(|d| d.insert_temp(box_id(ui), b));
        return;
    }
    ui.add_space(4.0);
    ui.add(egui::Label::new(egui::RichText::new(title).strong()).truncate());
    ui.add_space(1.0);
}

/// An on/off switch in the theme's accent colour, the label at the left
/// and the switch at the right. It answers like a checkbox: `.changed()`
/// when clicked.
pub fn switch(ui: &mut Ui, on: &mut bool, label: impl Into<egui::WidgetText>) -> Response {
    check(ui, "", on, label)
}

/// A small check box and its text, sized to its text: for grids of them
/// (a view's categories), as Blender's (MU22).
pub fn tick(ui: &mut Ui, on: &mut bool, label: &str, width: f32) -> Response {
    let h = 19.0;
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(width, h), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let enabled = ui.is_enabled();
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, *on, label)
    });
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let accent = v.selection.bg_fill;
        let ink = if enabled {
            v.text_color()
        } else {
            v.weak_text_color()
        };
        let b = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 7.0, rect.center().y),
            Vec2::splat(13.0),
        );
        if *on {
            ui.painter().rect_filled(
                b,
                3.0,
                if enabled {
                    accent
                } else {
                    accent.gamma_multiply(0.5)
                },
            );
            let t = egui::Stroke::new(1.7, Color32::WHITE);
            ui.painter().line_segment(
                [
                    b.left_center() + Vec2::new(3.0, 0.5),
                    b.center() + Vec2::new(-1.0, 3.0),
                ],
                t,
            );
            ui.painter().line_segment(
                [
                    b.center() + Vec2::new(-1.0, 3.0),
                    b.right_top() + Vec2::new(-3.0, 3.5),
                ],
                t,
            );
        } else {
            ui.painter()
                .rect_filled(b, 3.0, v.widgets.inactive.weak_bg_fill);
            ui.painter().rect_stroke(
                b,
                3.0,
                v.widgets.inactive.bg_stroke,
                egui::StrokeKind::Inside,
            );
        }
        if resp.hovered() && enabled {
            ui.painter().rect_stroke(
                b.expand(1.0),
                4.0,
                egui::Stroke::new(1.0, accent),
                egui::StrokeKind::Outside,
            );
        }
        let mut job = egui::text::LayoutJob::single_section(
            label.to_string(),
            egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), ink),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((width - 22.0).max(10.0));
        let g = ui.painter().layout_job(job);
        let elided = g.elided;
        ui.painter().galley(
            egui::pos2(rect.left() + 19.0, rect.center().y - g.size().y / 2.0),
            g,
            ink,
        );
        if elided {
            resp = resp.on_hover_text(label.to_string());
        }
    }
    resp
}

/// A check box with `group` in the label column, as Blender's "Show In":
/// the first of a group carries its name, the rest an empty label.
pub fn check(
    ui: &mut Ui,
    group: &str,
    on: &mut bool,
    label: impl Into<egui::WidgetText>,
) -> Response {
    let label: egui::WidgetText = label.into();
    let text = label.text().to_string();
    let w = ui.available_width() - inset(ui);
    // as Blender's (MU22): a check box in the value column, its text after
    // it; the label column left empty, so on/off settings line up under
    // the fields above them
    let h = (ui.spacing().interact_size.y - 2.0).max(20.0);
    let lw = if w < STACK_BELOW { 0.0 } else { label_width(w) };
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let enabled = ui.is_enabled();
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, *on, &text)
    });
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let ink = if enabled {
            v.text_color()
        } else {
            v.weak_text_color()
        };
        if !group.is_empty() && lw > 0.0 {
            let muted = v.widgets.noninteractive.fg_stroke.color;
            let mut job = egui::text::LayoutJob::single_section(
                group.to_string(),
                egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), muted),
            );
            job.wrap = egui::text::TextWrapping::truncate_at_width(lw - 6.0);
            let g = ui.painter().layout_job(job);
            ui.painter().galley(
                rect.left_center() + Vec2::new(lw - 6.0 - g.size().x, -g.size().y / 2.0),
                g,
                muted,
            );
        }
        let accent = v.selection.bg_fill;
        let field = v.widgets.inactive.weak_bg_fill;
        let edge = v.widgets.inactive.bg_stroke;
        let x0 = rect.left() + lw;
        let b =
            egui::Rect::from_center_size(egui::pos2(x0 + 8.0, rect.center().y), Vec2::splat(14.0));
        if *on {
            ui.painter().rect_filled(
                b,
                3.0,
                if enabled {
                    accent
                } else {
                    accent.gamma_multiply(0.5)
                },
            );
            let t = egui::Stroke::new(1.8, Color32::WHITE);
            ui.painter().line_segment(
                [
                    b.left_center() + Vec2::new(3.0, 0.5),
                    b.center() + Vec2::new(-1.0, 3.5),
                ],
                t,
            );
            ui.painter().line_segment(
                [
                    b.center() + Vec2::new(-1.0, 3.5),
                    b.right_top() + Vec2::new(-3.0, 3.5),
                ],
                t,
            );
        } else {
            ui.painter().rect_filled(b, 3.0, field);
            ui.painter()
                .rect_stroke(b, 3.0, edge, egui::StrokeKind::Inside);
        }
        if resp.hovered() && enabled {
            ui.painter().rect_stroke(
                b.expand(1.0),
                4.0,
                egui::Stroke::new(1.0, accent),
                egui::StrokeKind::Outside,
            );
        }
        let mut job = egui::text::LayoutJob::single_section(
            text.clone(),
            egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), ink),
        );
        job.wrap =
            egui::text::TextWrapping::truncate_at_width((rect.right() - x0 - 24.0).max(10.0));
        let g = ui.painter().layout_job(job);
        let elided = g.elided;
        ui.painter().galley(
            egui::pos2(x0 + 22.0, rect.center().y - g.size().y / 2.0),
            g,
            ink,
        );
        if elided {
            resp = resp.on_hover_text(text.clone());
        }
    }
    resp
}

/// A section header of Properties: a full-width row with the title in
/// semibold, a chevron at the right that turns as it opens, and a tint
/// under the pointer. `indent` draws a sub-section's header, set in.
/// Returns the row's response and the rect left for a ? button beside the
/// title.
pub fn header(ui: &mut Ui, title: &str, open: bool, indent: bool) -> (Response, egui::Rect) {
    let boxed = !indent && boxes_on(ui);
    let out = header_band(ui, title, open, indent);
    // a folded section's box ends under its band, so whatever comes next
    // never hangs under it as a lip (MU40)
    if boxed && !open {
        close_box(ui);
    }
    out
}

fn header_band(ui: &mut Ui, title: &str, open: bool, indent: bool) -> (Response, egui::Rect) {
    // as Blender's panels (MU20): a band a step darker than the panel, a
    // thin open chevron at the left that turns down when the section is
    // open, the title in bold after it, a dotted grip at the right
    let boxed = !indent && boxes_on(ui);
    if boxed {
        close_box(ui);
    }
    let w = ui.available_width() - if indent { inset(ui) } else { 0.0 };
    let h = if indent { 22.0 } else { 24.0 };
    let shape = boxed.then(|| ui.painter().add(egui::Shape::Noop));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if let Some(shape) = shape {
        let b = OpenBox {
            shape,
            left: rect.left(),
            right: rect.right(),
            top: rect.top(),
            header_bottom: rect.bottom(),
            pass: ui.ctx().cumulative_pass_nr(),
        };
        ui.ctx().data_mut(|d| d.insert_temp(box_id(ui), b));
    }
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let band = {
            let p = v.panel_fill;
            let (to, t) = if v.dark_mode {
                (255.0, 0.05)
            } else {
                (0.0, 0.045)
            };
            let m = |c: u8| (c as f32 + (to - c as f32) * t).round() as u8;
            Color32::from_rgb(m(p.r()), m(p.g()), m(p.b()))
        };
        // in a box, the box itself is the band (MU22); hovered, lighter
        if !boxed && !indent {
            ui.painter().rect_filled(rect, 4.0, band);
        }
        if resp.hovered() {
            let band = if boxed {
                rect.expand2(Vec2::new(PAD, 0.0))
            } else {
                rect
            };
            ui.painter().rect_filled(
                band,
                4.0,
                v.widgets.hovered.weak_bg_fill.gamma_multiply(0.5),
            );
        }
        let ink = v.strong_text_color();
        let soft = v.weak_text_color();
        // the chevron: › when shut, ˅ when open, turning between
        let t = ui.ctx().animate_bool_responsive(resp.id, open);
        if indent {
            // a sub-section, as Blender's: flush with its section, its
            // header on a band a shade deeper than the box (MU22)
            let deeper = if v.dark_mode {
                Color32::from_black_alpha(40)
            } else {
                Color32::from_black_alpha(14)
            };
            ui.painter().rect_filled(rect, 3.0, deeper);
        }
        let c = egui::pos2(rect.left() + 4.0, rect.center().y);
        let ang = t * std::f32::consts::FRAC_PI_2;
        let rot = |p: Vec2| {
            let (s, co) = ang.sin_cos();
            c + Vec2::new(p.x * co - p.y * s, p.x * s + p.y * co)
        };
        let pts = [
            rot(Vec2::new(-1.8, -3.6)),
            rot(Vec2::new(1.8, 0.0)),
            rot(Vec2::new(-1.8, 3.6)),
        ];
        let stroke = egui::Stroke::new(1.3, ink);
        ui.painter().line_segment([pts[0], pts[1]], stroke);
        ui.painter().line_segment([pts[1], pts[2]], stroke);
        let left = c.x + 9.0;
        let font = egui::FontId::proportional(if indent { 12.0 } else { 12.5 });
        // one line, cut with … (MU22): a band never grows taller
        let mut job = egui::text::LayoutJob::single_section(
            title.to_string(),
            egui::TextFormat::simple(font, ink),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((w - 70.0).max(10.0));
        let g = ui.painter().layout_job(job);
        let tw = g.size().x;
        ui.painter()
            .galley(egui::pos2(left, rect.center().y - g.size().y / 2.0), g, ink);
        // a dotted grip at the right, as Blender's
        if !indent {
            for row in 0..2 {
                for col in 0..3 {
                    ui.painter().circle_filled(
                        egui::pos2(
                            rect.right() - 18.0 + col as f32 * 3.5,
                            rect.center().y - 1.8 + row as f32 * 3.6,
                        ),
                        0.8,
                        soft.gamma_multiply(0.7),
                    );
                }
            }
        }
        let help = egui::Rect::from_min_size(
            egui::pos2(left + tw + 6.0, rect.center().y - 8.0),
            Vec2::splat(16.0),
        );
        return (resp, help);
    }
    (resp, egui::Rect::NOTHING)
}

/// A labelled drop-down that draws as a Properties row: the label at the
/// left, the box filling the rest. It is made like an egui `ComboBox`.
pub struct Combo {
    label: String,
    inner: egui::ComboBox,
    width: Option<f32>,
}

impl Combo {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            inner: egui::ComboBox::from_id_salt(("props.combo", label.to_string()))
                .icon(crate::props::combo_icon),
            width: None,
        }
    }

    pub fn selected_text(mut self, t: impl Into<egui::WidgetText>) -> Self {
        self.inner = self.inner.selected_text(t);
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn show_ui<R>(
        self,
        ui: &mut Ui,
        add: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        let Combo {
            label,
            inner,
            width,
        } = self;
        row(ui, &label, |ui| {
            let w = width.unwrap_or(f32::INFINITY).min(ui.available_width());
            inner.width(w).show_ui(ui, add)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_label_column_is_two_fifths_of_the_row_and_narrow_rows_stack() {
        // about 38 %, never under 44 px, no wider than 160 (MU36: a narrow
        // panel keeps one value a line, the field stretching)
        assert!((label_width(300.0) - 114.0).abs() < 1e-3);
        assert_eq!(label_width(1000.0), 160.0);
        assert_eq!(label_width(100.0), 44.0);
        // only below 110 px does the label go above its control
        assert_eq!(STACK_BELOW, 110.0);
    }
}
pub mod set_membership;
