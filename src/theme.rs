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
    pub gold: Color32,
    pub green: Color32,
    pub orange: Color32,
    pub purple: Color32,
    pub red: Color32,
}

// A quiet night-academy palette: teal carries interactive state, while gold
// is reserved for hierarchy and decorative accents. Both themes keep WCAG-like
// contrast rather than placing text directly on decorative artwork.
const LIGHT: Palette = Palette {
    background: Color32::from_rgb(239, 238, 233),
    surface: Color32::from_rgb(253, 252, 248),
    surface_muted: Color32::from_rgb(246, 244, 238),
    label: Color32::from_rgb(31, 35, 43),
    secondary_label: Color32::from_rgb(91, 98, 108),
    tertiary_label: Color32::from_rgb(128, 133, 140),
    separator: Color32::from_rgb(217, 211, 198),
    blue: Color32::from_rgb(31, 126, 126),
    gold: Color32::from_rgb(162, 119, 48),
    green: Color32::from_rgb(42, 145, 92),
    orange: Color32::from_rgb(190, 112, 34),
    purple: Color32::from_rgb(99, 83, 146),
    red: Color32::from_rgb(191, 64, 62),
};

const DARK: Palette = Palette {
    background: Color32::from_rgb(11, 15, 26),
    surface: Color32::from_rgb(20, 26, 40),
    surface_muted: Color32::from_rgb(16, 21, 34),
    label: Color32::from_rgb(238, 240, 244),
    secondary_label: Color32::from_rgb(172, 180, 194),
    tertiary_label: Color32::from_rgb(111, 122, 142),
    separator: Color32::from_rgb(45, 55, 74),
    blue: Color32::from_rgb(36, 130, 130),
    gold: Color32::from_rgb(202, 164, 91),
    green: Color32::from_rgb(66, 184, 120),
    orange: Color32::from_rgb(220, 145, 67),
    purple: Color32::from_rgb(139, 119, 190),
    red: Color32::from_rgb(224, 91, 88),
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

pub fn gold() -> Color32 {
    palette().gold
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

/// Paints a very low-contrast observatory motif behind opaque content cards.
/// The geometry is original and intentionally subtle so it never competes
/// with labels, controls, or recognition previews.
pub fn paint_background(painter: &egui::Painter, rect: egui::Rect) {
    let center = rect.right_top() + egui::vec2(-42.0, 38.0);
    let gold = gold().gamma_multiply(0.055);
    let teal = blue().gamma_multiply(0.045);
    for radius in [54.0, 88.0, 128.0] {
        painter.circle_stroke(center, radius, Stroke::new(1.0, gold));
    }
    painter.line_segment(
        [
            center + egui::vec2(-128.0, 0.0),
            center + egui::vec2(128.0, 0.0),
        ],
        Stroke::new(1.0, gold),
    );
    painter.line_segment(
        [
            center + egui::vec2(0.0, -128.0),
            center + egui::vec2(0.0, 128.0),
        ],
        Stroke::new(1.0, gold),
    );

    let origin = rect.left_bottom() + egui::vec2(28.0, -22.0);
    let stars = [
        egui::vec2(0.0, 0.0),
        egui::vec2(46.0, -24.0),
        egui::vec2(91.0, -8.0),
        egui::vec2(132.0, -54.0),
        egui::vec2(181.0, -39.0),
    ];
    for segment in stars.windows(2) {
        painter.line_segment(
            [origin + segment[0], origin + segment[1]],
            Stroke::new(1.0, teal),
        );
    }
    for point in stars {
        painter.circle_filled(origin + point, 2.0, teal);
    }
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
        Color32::from_rgb(28, 42, 55)
    } else {
        Color32::from_rgb(233, 243, 239)
    };
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.blue.gamma_multiply(0.55));
    style.visuals.widgets.active.bg_fill = if dark {
        Color32::from_rgb(25, 61, 66)
    } else {
        Color32::from_rgb(219, 237, 232)
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
    // Bundle a small OFL-licensed Noto Sans CJK subset containing every glyph
    // used by the application. This prevents tofu boxes on Windows systems
    // without a Chinese language pack. System faces remain later fallbacks for
    // user-entered names containing glyphs outside the bundled subset.
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "embedded-cjk-ui".to_owned(),
        FontData::from_static(include_bytes!("../assets/fonts/NotoSansCJKsc-UI.otf")).into(),
    );
    let mut loaded_names = vec!["embedded-cjk-ui".to_owned()];

    let font_groups: [(&str, &[&str]); 2] = [
        (
            "system-ui",
            &[
                r"C:\Windows\Fonts\msyh.ttc",
                r"C:\Windows\Fonts\msyh.ttf",
                r"C:\Windows\Fonts\msjh.ttc",
                r"C:\Windows\Fonts\YuGothM.ttc",
                r"C:\Windows\Fonts\msgothic.ttc",
                r"C:\Windows\Fonts\simsun.ttc",
                r"C:\Windows\Fonts\simhei.ttf",
                r"C:\Windows\Fonts\Deng.ttf",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            ],
        ),
        (
            "system-symbols",
            &[
                r"C:\Windows\Fonts\seguisym.ttf",
                r"C:\Windows\Fonts\segoeui.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            ],
        ),
    ];

    for (name, candidates) in font_groups {
        if let Some(bytes) = candidates.iter().find_map(|path| std::fs::read(path).ok()) {
            fonts
                .font_data
                .insert(name.to_owned(), FontData::from_owned(bytes).into());
            loaded_names.push(name.to_owned());
        }
    }
    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    for (index, name) in loaded_names.iter().enumerate() {
        proportional.insert(index, name.clone());
    }
    // Preserve the built-in monospaced ASCII face, then fall back to the
    // system fonts only for CJK/symbol glyphs it cannot render.
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .extend(loaded_names);
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

pub fn secondary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.into()).color(label()))
        .fill(surface_muted())
        .stroke(Stroke::new(1.0, separator()))
        .corner_radius(CornerRadius::same(10))
}

pub fn small_danger_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.into()).color(red()).size(12.0))
        .fill(red().gamma_multiply(0.08))
        .stroke(Stroke::new(1.0, red().gamma_multiply(0.30)))
        .corner_radius(CornerRadius::same(8))
}

pub fn section_card() -> egui::Frame {
    egui::Frame::new()
        .fill(surface_muted())
        .stroke(Stroke::new(1.0, separator()))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(12, 10))
}

#[cfg(test)]
mod tests {
    use super::{DARK, LIGHT, Palette};
    use eframe::egui::Color32;

    fn relative_luminance(color: Color32) -> f32 {
        let linear = |channel: u8| {
            let value = f32::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }

    fn contrast(first: Color32, second: Color32) -> f32 {
        let first = relative_luminance(first);
        let second = relative_luminance(second);
        let (lighter, darker) = if first >= second {
            (first, second)
        } else {
            (second, first)
        };
        (lighter + 0.05) / (darker + 0.05)
    }

    fn assert_readable(palette: Palette) {
        assert!(contrast(palette.label, palette.surface) >= 7.0);
        assert!(contrast(palette.secondary_label, palette.surface) >= 4.5);
        assert!(contrast(Color32::WHITE, palette.blue) >= 4.5);
    }

    #[test]
    fn themed_palettes_keep_primary_text_and_actions_readable() {
        assert_readable(LIGHT);
        assert_readable(DARK);
    }

    #[test]
    fn embedded_font_covers_core_chinese_ui_labels() {
        let ctx = eframe::egui::Context::default();
        super::install_system_font(&ctx);
        let _ = ctx.run_ui(eframe::egui::RawInput::default(), |_| {});
        let font_id = eframe::egui::FontId::proportional(14.0);
        ctx.fonts_mut(|fonts| {
            assert!(fonts.has_glyphs(
                &font_id,
                "流程工作台 识别目标 搜索范围 点击动作 等待策略 运行前检查"
            ));
        });
    }
}
