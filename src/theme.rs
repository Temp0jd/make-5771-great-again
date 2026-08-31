use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke};

pub const BACKGROUND: Color32 = Color32::from_rgb(242, 242, 247);
pub const SURFACE: Color32 = Color32::from_rgb(255, 255, 255);
pub const SURFACE_MUTED: Color32 = Color32::from_rgb(248, 248, 250);
pub const LABEL: Color32 = Color32::from_rgb(28, 28, 30);
pub const SECONDARY_LABEL: Color32 = Color32::from_rgb(99, 99, 102);
pub const TERTIARY_LABEL: Color32 = Color32::from_rgb(142, 142, 147);
pub const SEPARATOR: Color32 = Color32::from_rgb(225, 225, 230);
pub const BLUE: Color32 = Color32::from_rgb(0, 122, 255);
pub const GREEN: Color32 = Color32::from_rgb(52, 199, 89);
pub const ORANGE: Color32 = Color32::from_rgb(255, 149, 0);

pub fn install(ctx: &egui::Context) {
    install_system_font(ctx);

    ctx.set_theme(egui::Theme::Light);
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(16.0, 9.0);
    style.spacing.interact_size.y = 36.0;
    style.spacing.scroll.floating = false;
    style.spacing.scroll.bar_width = 10.0;
    style.spacing.scroll.handle_min_length = 32.0;
    style.spacing.scroll.bar_inner_margin = 2.0;
    style.spacing.scroll.bar_outer_margin = 2.0;
    style.visuals.dark_mode = false;
    style.visuals.panel_fill = BACKGROUND;
    style.visuals.window_fill = SURFACE;
    style.visuals.extreme_bg_color = SURFACE_MUTED;
    style.visuals.faint_bg_color = SURFACE_MUTED;
    style.visuals.override_text_color = Some(LABEL);
    style.visuals.selection.bg_fill = BLUE.gamma_multiply(0.20);
    style.visuals.selection.stroke = Stroke::new(1.0, BLUE);
    style.visuals.widgets.noninteractive.bg_fill = SURFACE;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, SEPARATOR);
    style.visuals.widgets.inactive.bg_fill = SURFACE_MUTED;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, SEPARATOR);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(238, 244, 252);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BLUE.gamma_multiply(0.55));
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(224, 237, 252);
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, BLUE);
    style.visuals.widgets.open.bg_fill = SURFACE;
    style.visuals.window_corner_radius = CornerRadius::same(16);
    style.visuals.menu_corner_radius = CornerRadius::same(12);

    for visuals in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        visuals.corner_radius = CornerRadius::same(10);
    }

    ctx.set_global_style(style);
}

fn install_system_font(ctx: &egui::Context) {
    let candidates = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyh.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ];

    let Some(bytes) = candidates.iter().find_map(|path| std::fs::read(path).ok()) else {
        return;
    };

    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("system-ui".to_owned(), FontData::from_owned(bytes).into());
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "system-ui".to_owned());
    ctx.set_fonts(fonts);
}

pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, SEPARATOR))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(egui::Margin::same(18))
}

pub fn subtle_card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE_MUTED)
        .stroke(Stroke::new(1.0, SEPARATOR))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::same(12))
}

pub fn primary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .color(Color32::WHITE)
            .strong(),
    )
    .fill(BLUE)
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(12))
    .min_size(egui::vec2(180.0, 44.0))
}
