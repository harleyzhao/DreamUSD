use std::collections::BTreeMap;

use egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Frame, Margin, RichText, Stroke,
    TextStyle,
};

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.menu_margin = Margin::symmetric(8, 6);
    style.spacing.window_margin = Margin::same(12);
    style.spacing.interact_size = egui::vec2(32.0, 22.0);
    style.spacing.combo_width = 140.0;
    style.spacing.text_edit_width = 160.0;
    style.spacing.indent = 14.0;
    style.spacing.icon_width = 14.0;
    style.spacing.icon_spacing = 4.0;
    style.visuals = visuals();
    style.text_styles = text_styles();
    ctx.set_style(style);
}

fn text_styles() -> BTreeMap<TextStyle, FontId> {
    use TextStyle::*;

    [
        (Heading, FontId::new(18.0, FontFamily::Proportional)),
        (
            TextStyle::Name("PanelTitle".into()),
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Name("Eyebrow".into()),
            FontId::new(10.0, FontFamily::Proportional),
        ),
        (Body, FontId::new(12.5, FontFamily::Proportional)),
        (Button, FontId::new(12.5, FontFamily::Proportional)),
        (Monospace, FontId::new(11.5, FontFamily::Monospace)),
        (Small, FontId::new(11.0, FontFamily::Proportional)),
    ]
    .into()
}

fn visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(text_color());
    visuals.hyperlink_color = accent_color();
    visuals.panel_fill = app_background();
    visuals.window_fill = chrome_surface();
    visuals.window_stroke = Stroke::new(1.0, border_color());
    visuals.window_corner_radius = 8.into();
    visuals.menu_corner_radius = 6.into();
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(80),
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(60),
    };
    visuals.faint_bg_color = Color32::from_rgb(20, 26, 34);
    visuals.extreme_bg_color = Color32::from_rgb(10, 14, 20);
    visuals.code_bg_color = Color32::from_rgb(19, 24, 31);
    visuals.warn_fg_color = warning_color();
    visuals.error_fg_color = danger_color();
    visuals.selection.bg_fill = accent_color().gamma_multiply(0.20);
    visuals.selection.stroke = Stroke::new(1.0, accent_color());
    visuals.button_frame = true;
    visuals.collapsing_header_frame = false;
    visuals.indent_has_left_vline = false;
    visuals.striped = false;
    visuals.slider_trailing_fill = true;

    visuals.widgets.noninteractive.bg_fill = panel_surface();
    visuals.widgets.noninteractive.weak_bg_fill = panel_surface();
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, subtle_border_color());
    visuals.widgets.noninteractive.corner_radius = 4.into();
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, muted_text_color());

    visuals.widgets.inactive.bg_fill = card_surface();
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(28, 35, 44);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, subtle_border_color());
    visuals.widgets.inactive.corner_radius = 4.into();
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, text_color());

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(38, 48, 60);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(42, 52, 64);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, accent_color().gamma_multiply(0.5));
    visuals.widgets.hovered.corner_radius = 4.into();
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, text_color());
    visuals.widgets.hovered.expansion = 0.0;

    visuals.widgets.active.bg_fill = accent_color().gamma_multiply(0.18);
    visuals.widgets.active.weak_bg_fill = accent_color().gamma_multiply(0.18);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, accent_color());
    visuals.widgets.active.corner_radius = 4.into();
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);

    visuals.widgets.open = visuals.widgets.active;
    visuals
}

// ── Color palette ──────────────────────────────────────────────────

pub fn app_background() -> Color32 {
    Color32::from_rgb(18, 18, 22)
}

pub fn chrome_surface() -> Color32 {
    Color32::from_rgb(24, 24, 30)
}

pub fn panel_surface() -> Color32 {
    Color32::from_rgb(28, 28, 34)
}

pub fn card_surface() -> Color32 {
    Color32::from_rgb(34, 34, 42)
}

pub fn card_surface_alt() -> Color32 {
    Color32::from_rgb(40, 40, 48)
}

pub fn accent_color() -> Color32 {
    Color32::from_rgb(86, 156, 238)
}

pub fn success_color() -> Color32 {
    Color32::from_rgb(82, 196, 132)
}

pub fn warning_color() -> Color32 {
    Color32::from_rgb(230, 180, 80)
}

pub fn danger_color() -> Color32 {
    Color32::from_rgb(220, 90, 90)
}

pub fn muted_text_color() -> Color32 {
    Color32::from_rgb(130, 136, 150)
}

pub fn text_color() -> Color32 {
    Color32::from_rgb(220, 224, 232)
}

pub fn border_color() -> Color32 {
    Color32::from_rgb(50, 50, 62)
}

pub fn subtle_border_color() -> Color32 {
    Color32::from_rgb(38, 38, 48)
}

// ── Prim icon colors ───────────────────────────────────────────────

pub fn icon_color_mesh() -> Color32 {
    Color32::from_rgb(100, 180, 255)
}

pub fn icon_color_xform() -> Color32 {
    Color32::from_rgb(180, 160, 110)
}

pub fn icon_color_light() -> Color32 {
    Color32::from_rgb(255, 210, 80)
}

pub fn icon_color_camera() -> Color32 {
    Color32::from_rgb(160, 120, 220)
}

pub fn icon_color_material() -> Color32 {
    Color32::from_rgb(220, 100, 140)
}

pub fn icon_color_scope() -> Color32 {
    Color32::from_rgb(130, 200, 160)
}

pub fn icon_color_default() -> Color32 {
    muted_text_color()
}

// ── Frame helpers ──────────────────────────────────────────────────

pub fn chrome_frame() -> Frame {
    Frame::new()
        .fill(chrome_surface())
        .stroke(Stroke::new(1.0, subtle_border_color()))
        .inner_margin(Margin::symmetric(8, 6))
}

pub fn sidebar_frame() -> Frame {
    Frame::new()
        .fill(panel_surface())
        .inner_margin(Margin::symmetric(8, 8))
}

pub fn panel_card_frame() -> Frame {
    Frame::new()
        .fill(card_surface())
        .stroke(Stroke::new(1.0, subtle_border_color()))
        .corner_radius(6)
        .inner_margin(Margin::same(8))
}

pub fn section_frame() -> Frame {
    Frame::new()
        .fill(card_surface_alt())
        .stroke(Stroke::new(1.0, subtle_border_color()))
        .corner_radius(4)
        .inner_margin(Margin::same(8))
}

pub fn viewport_toolbar_frame() -> Frame {
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(28, 28, 34, 230))
        .stroke(Stroke::new(1.0, subtle_border_color()))
        .corner_radius(6)
        .inner_margin(Margin::symmetric(8, 4))
}

pub fn toolbar_frame() -> Frame {
    Frame::new()
        .fill(chrome_surface())
        .stroke(Stroke::new(1.0, border_color()))
        .inner_margin(Margin::symmetric(8, 4))
}

pub fn status_bar_frame() -> Frame {
    Frame::new()
        .fill(Color32::from_rgb(20, 20, 26))
        .stroke(Stroke::new(1.0, subtle_border_color()))
        .inner_margin(Margin::symmetric(10, 4))
}

// ── Rich text helpers ──────────────────────────────────────────────

pub fn panel_title(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(13.0, FontFamily::Proportional))
        .strong()
        .color(text_color())
        .extra_letter_spacing(0.3)
}

pub fn eyebrow(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(10.0, FontFamily::Proportional))
        .color(muted_text_color())
        .extra_letter_spacing(1.0)
}

pub fn section_title(text: &str) -> RichText {
    RichText::new(text).strong().color(text_color())
}

pub fn subdued(text: &str) -> RichText {
    RichText::new(text).color(muted_text_color())
}

pub fn chip_frame(fill: Color32) -> Frame {
    Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(8, 3))
}
