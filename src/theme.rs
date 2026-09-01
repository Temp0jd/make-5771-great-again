use std::cell::Cell;

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke};

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    pub surface: Color32,
    pub surface_muted: Color32,
    pub label: Color32,
    pub secondary_label: Color32,
    pub tertiary_label: Color32,
    pub separator: Color32,
    pub blue: Color32,
    pub green: Color32,
    pub orange: Color32,
    pub purple: Color32,
    pub red: Color32,
}

const LIGHT: Palette = Palette {
    background: Color32::from_rgb(242, 242, 247),
    surface: Color32::from_rgb(255, 255, 255),
    surface_muted: Color32::from_rgb(248, 248, 250),
    label: Color32::from_rgb(28, 28, 30),
    secondary_label: Color32::from_rgb(99, 99, 102),
    tertiary_label: Color32::from_rgb(142, 142, 147),
    separator: Color32::from_rgb(225, 225, 230),
    blue: Color32::from_rgb(0, 122, 255),
    green: Color32::from_rgb(52, 199, 89),
    orange: Color32::from_rgb(255, 149, 0),
    purple: Color32::from_rgb(175, 82, 222),
    red: Color32::from_rgb(255, 59, 48),
};

const DARK: Palette = Palette {
    background: Color32::from_rgb(24, 24, 27),
    surface: Color32::from_rgb(38, 38, 42),
    surface_muted: Color32::from_rgb(30, 30, 34),
    label: Color32::from_rgb(242, 242, 247),
    secondary_label: Color32::from_rgb(174, 174, 178),
    tertiary_label: Color32::from_rgb(110, 110, 115),
    separator: Color32::from_rgb(58, 58, 62),
    blue: Color32::from_rgb(10, 132, 255),
    green: Color32::from_rgb(48, 209, 88),
    orange: Color32::from_rgb(255, 159, 10),
    purple: Color32::from_rgb(191, 90, 242),
    red: Color32::from_rgb(255, 69, 58),
};

thread_local! {
    static PALETTE: Cell<Palette> = const { Cell::new(LIGHT) };
}

fn palette() -> Palette {
    PALETTE.with(Cell::get)
}

pub fn background() -> Color32 {
    palette().background
}

pub fn surface() -> Color32 {
    palette().surface
}

pub fn surface_muted() -> Color32 {
    palette().surface_muted
}

pub fn label() -> Color32 {
    palette().label
}

pub fn secondary_label() -> Color32 {
    palette().secondary_label
}

pub fn tertiary_label() -> Color32 {
    palette().tertiary_label
}

pub fn separator() -> Color32 {
    palette().separator
}

pub fn blue() -> Color32 {
    palette().blue
}

pub fn green() -> Color32 {
    palette().green
}

pub fn orange() -> Color32 {
    palette().orange
}

pub fn purple() -> Color32 {
    palette().purple
}

pub fn red() -> Color32 {
    palette().red
}

pub fn install(ctx: &egui::Context, dark: bool) {
    install_system_font(ctx);
    apply(ctx, dark);
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    let palette = if dark { DARK } else { LIGHT };
    PALETTE.with(|current| current.set(palette));

    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 7.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.interact_size.y = 32.0;
    style.spacing.scroll.floating = false;
    style.spacing.scroll.bar_width = 10.0;
    style.spacing.scroll.handle_min_length = 32.0;
    style.spacing.scroll.bar_inner_margin = 2.0;
    style.spacing.scroll.bar_outer_margin = 2.0;
    style.visuals.dark_mode = dark;
    style.visuals.panel_fill = palette.background;
    style.visuals.window_fill = palette.surface;
    style.visuals.extreme_bg_color = palette.surface_muted;
    style.visuals.faint_bg_color = palette.surface_muted;
    style.visuals.override_text_color = Some(palette.label);
    style.visuals.selection.bg_fill = palette.blue.gamma_multiply(0.25);
    style.visuals.selection.stroke = Stroke::new(1.0, palette.blue);
    style.visuals.widgets.noninteractive.bg_fill = palette.surface;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.separator);
    style.visuals.widgets.inactive.bg_fill = palette.surface_muted;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.separator);
    style.visuals.widgets.hovered.bg_fill = if dark {
        Color32::from_rgb(46, 50, 58)
    } else {
        Color32::from_rgb(238, 244, 252)
    };
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.blue.gamma_multiply(0.55));
    style.visuals.widgets.active.bg_fill = if dark {
        Color32::from_rgb(30, 58, 88)
    } else {
        Color32::from_rgb(224, 237, 252)
    };
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.blue);
    style.visuals.widgets.open.bg_fill = palette.surface;
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
        .fill(surface())
        .stroke(Stroke::new(1.0, separator()))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::same(14))
}

pub fn subtle_card() -> egui::Frame {
    egui::Frame::new()
        .fill(surface_muted())
        .stroke(Stroke::new(1.0, separator()))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(10))
}

pub fn primary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .color(Color32::WHITE)
            .strong(),
    )
    .fill(blue())
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(12))
    .min_size(egui::vec2(160.0, 40.0))
}
