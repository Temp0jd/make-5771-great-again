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

// A restrained game-themed palette: teal carries interactive state, while
// gold is reserved for hierarchy and decorative accents. Both themes keep
// WCAG-like contrast rather than placing text on decorative elements.
const LIGHT: Palette = Palette {
    background: Color32::from_rgb(239, 238, 233),
    surface: Color32::from_rgb(253, 252, 248),
    surface_muted: Color32::from_rgb(246, 244, 238),
    label: Color32::from_rgb(31, 35, 43),
    secondary_label: Color32::from_rgb(91, 98, 108),
    tertiary_label: Color32::from_rgb(109, 115, 124),
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
    tertiary_label: Color32::from_rgb(131, 141, 160),
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
    static DARK_MODE: Cell<bool> = const { Cell::new(false) };
}

fn palette() -> Palette {
    PALETTE.with(Cell::get)
}

pub fn is_dark() -> bool {
    DARK_MODE.with(Cell::get)
}

/// Translucent card surface. The alpha stays high enough that labels keep
/// their contrast even when a background ornament shows through the card.
pub fn glass_surface() -> Color32 {
    if is_dark() {
        Color32::from_rgba_unmultiplied(24, 31, 47, 236)
    } else {
        Color32::from_rgba_unmultiplied(253, 252, 248, 242)
    }
}

pub fn glass_muted() -> Color32 {
    if is_dark() {
        Color32::from_rgba_unmultiplied(18, 24, 38, 232)
    } else {
        Color32::from_rgba_unmultiplied(246, 244, 238, 238)
    }
}

/// Soft elevation used by content cards. Dark themes need a stronger shadow to
/// read against the near-black background.
pub fn card_shadow() -> egui::epaint::Shadow {
    if is_dark() {
        egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(96),
        }
    } else {
        egui::epaint::Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(26),
        }
    }
}

pub fn subtle_shadow() -> egui::epaint::Shadow {
    if is_dark() {
        egui::epaint::Shadow {
            offset: [0, 5],
            blur: 14,
            spread: 0,
            color: Color32::from_black_alpha(64),
        }
    } else {
        egui::epaint::Shadow {
            offset: [0, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(16),
        }
    }
}

pub fn window_shadow() -> egui::epaint::Shadow {
    if is_dark() {
        egui::epaint::Shadow {
            offset: [0, 10],
            blur: 30,
            spread: 0,
            color: Color32::from_black_alpha(150),
        }
    } else {
        egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(36),
        }
    }
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

/// Fill for framed controls (buttons, combo boxes, switches). The muted surface
/// was too close to the glass cards, so controls such as toggles disappeared.
pub fn control_fill() -> Color32 {
    if is_dark() {
        Color32::from_rgb(45, 55, 73)
    } else {
        Color32::from_rgb(231, 229, 221)
    }
}

/// Outline for framed controls. Keeps at least 3:1 contrast against the card
/// surface so the control boundary stays visible (WCAG 1.4.11 non-text).
pub fn control_border() -> Color32 {
    if is_dark() {
        Color32::from_rgb(101, 114, 138)
    } else {
        Color32::from_rgb(146, 139, 124)
    }
}

fn switch_track_off() -> Color32 {
    if is_dark() {
        Color32::from_rgb(52, 63, 84)
    } else {
        Color32::from_rgb(200, 195, 182)
    }
}

/// Always-framed on/off switch.
///
/// egui's `toggle_value` paints no frame at all while the value is off, which
/// made switches such as “深色模式” read as plain text. This variant always
/// paints a track, an outline and the current state text, and toggles on click.
fn blend(from: Color32, to: Color32, t: f32) -> Color32 {
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
    Color32::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

pub fn switch(ui: &mut egui::Ui, value: &mut bool) -> egui::Response {
    let on = *value;
    let state_text = if on { "开启" } else { "关闭" };
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(state_text.to_owned(), font, label());
    let track = egui::vec2(40.0, 22.0);
    let size = egui::vec2(
        galley.size().x + 10.0 + track.x,
        track.y.max(galley.size().y),
    );
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let track_rect = egui::Rect::from_center_size(
            egui::pos2(rect.right() - track.x / 2.0, rect.center().y),
            track,
        );
        let radius = CornerRadius::same(11);
        let progress = ui
            .ctx()
            .animate_bool_with_time(response.id.with("switch-anim"), on, 0.10);
        ui.painter().rect_filled(
            track_rect,
            radius,
            blend(switch_track_off(), blue(), progress),
        );
        ui.painter().rect_stroke(
            track_rect,
            radius,
            Stroke::new(
                1.0,
                if on {
                    blue().gamma_multiply(0.7)
                } else {
                    control_border()
                },
            ),
            egui::StrokeKind::Inside,
        );
        let knob_x = track_rect.left() + 11.0 + (track_rect.width() - 22.0) * progress;
        ui.painter().circle_filled(
            egui::pos2(knob_x, track_rect.center().y),
            7.5,
            Color32::WHITE,
        );
        ui.painter().galley(
            egui::pos2(rect.left(), rect.center().y - galley.size().y / 2.0),
            galley,
            label(),
        );
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response
}

/// Paints a very low-contrast circular and constellation ornament behind
/// opaque content cards. It stays subtle so it never competes with labels,
/// controls, or recognition previews.
pub fn paint_background(painter: &egui::Painter, rect: egui::Rect) {
    // Very faint brand-colour glows sit under the existing ornaments and the
    // translucent cards, which is what makes the glass surfaces read as layered.
    painter.circle_filled(
        rect.right_top() + egui::vec2(-150.0, 40.0),
        300.0,
        blue().gamma_multiply(0.055),
    );
    painter.circle_filled(
        rect.left_bottom() + egui::vec2(120.0, -50.0),
        260.0,
        gold().gamma_multiply(0.05),
    );

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
    DARK_MODE.with(|current| current.set(dark));

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
    // Also used as the keyboard focus ring, so keep it clearly visible.
    style.visuals.selection.stroke = Stroke::new(2.0, palette.blue);
    style.visuals.widgets.noninteractive.bg_fill = palette.surface;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.separator);
    style.visuals.widgets.inactive.bg_fill = control_fill();
    style.visuals.widgets.inactive.weak_bg_fill = control_fill();
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, control_border());
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
    style.visuals.window_corner_radius = CornerRadius::same(18);
    style.visuals.menu_corner_radius = CornerRadius::same(14);
    style.visuals.window_shadow = window_shadow();
    style.visuals.popup_shadow = subtle_shadow();

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
        .fill(glass_surface())
        .stroke(Stroke::new(1.0, separator()))
        .corner_radius(CornerRadius::same(18))
        .shadow(card_shadow())
        .inner_margin(egui::Margin::same(16))
}

pub fn subtle_card() -> egui::Frame {
    egui::Frame::new()
        .fill(glass_muted())
        .stroke(Stroke::new(1.0, separator()))
        .corner_radius(CornerRadius::same(12))
        .shadow(subtle_shadow())
        .inner_margin(egui::Margin::same(11))
}

pub fn primary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(text.into())
            .color(Color32::WHITE)
            .strong(),
    )
    .fill(blue())
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(14))
    .min_size(egui::vec2(160.0, 40.0))
}

pub fn secondary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.into()).color(label()))
        .fill(glass_muted())
        .stroke(Stroke::new(1.0, separator()))
        .corner_radius(CornerRadius::same(12))
}

pub fn small_danger_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.into()).color(red()).size(12.0))
        .fill(red().gamma_multiply(0.08))
        .stroke(Stroke::new(1.0, red().gamma_multiply(0.30)))
        .corner_radius(CornerRadius::same(10))
}

pub fn section_card() -> egui::Frame {
    egui::Frame::new()
        .fill(glass_muted())
        .stroke(Stroke::new(1.0, separator()))
        .corner_radius(CornerRadius::same(13))
        .shadow(subtle_shadow())
        .inner_margin(egui::Margin::symmetric(13, 11))
}

#[cfg(test)]
mod tests {
    use super::{DARK, LIGHT, Palette};
    use eframe::egui;
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
        assert!(contrast(palette.tertiary_label, palette.surface) >= 4.5);
        assert!(contrast(Color32::WHITE, palette.blue) >= 4.5);
    }

    #[test]
    fn themed_palettes_keep_primary_text_and_actions_readable() {
        assert_readable(LIGHT);
        assert_readable(DARK);
    }

    /// Alpha-blends a (premultiplied) `Color32` over an opaque background the
    /// same way the renderer does, so colour assertions match what users see.
    fn composite(foreground: Color32, background: Color32) -> Color32 {
        let alpha = f32::from(foreground.a()) / 255.0;
        let straight = |channel: u8| {
            if alpha <= 0.0 {
                0.0
            } else {
                (f32::from(channel) / alpha).min(255.0)
            }
        };
        let mix = |front: u8, back: u8| {
            (straight(front) * alpha + f32::from(back) * (1.0 - alpha)).round() as u8
        };
        Color32::from_rgb(
            mix(foreground.r(), background.r()),
            mix(foreground.g(), background.g()),
            mix(foreground.b(), background.b()),
        )
    }

    #[test]
    fn translucent_cards_keep_labels_readable_over_background_ornaments() {
        for dark in [false, true] {
            let palette = if dark { DARK } else { LIGHT };
            super::PALETTE.with(|current| current.set(palette));
            super::DARK_MODE.with(|current| current.set(dark));
            // Worst case: a card sits on top of the strongest ornament colour.
            let ornament = composite(palette.blue.gamma_multiply(0.06), palette.background);
            let card = composite(super::glass_surface(), ornament);
            assert!(contrast(palette.label, card) >= 7.0, "dark={dark}");
            assert!(
                contrast(palette.secondary_label, card) >= 4.5,
                "dark={dark}"
            );
            let frame = super::card();
            assert_eq!(frame.corner_radius, eframe::egui::CornerRadius::same(18));
            assert_ne!(frame.shadow, eframe::egui::epaint::Shadow::NONE);
            assert!(frame.shadow.blur > 0);
        }
    }

    #[test]
    fn framed_controls_stay_visible_on_glass_cards() {
        for dark in [false, true] {
            let palette = if dark { DARK } else { LIGHT };
            super::PALETTE.with(|current| current.set(palette));
            super::DARK_MODE.with(|current| current.set(dark));
            let ornament = composite(palette.blue.gamma_multiply(0.06), palette.background);
            let card = composite(super::glass_surface(), ornament);
            assert!(
                contrast(super::control_border(), card) >= 3.0,
                "border contrast too low (dark={dark})"
            );
            assert!(
                contrast(super::control_fill(), card) >= 1.10,
                "control fill does not stand out (dark={dark})"
            );
        }
    }

    #[test]
    fn switch_paints_a_framed_track_and_toggles_by_click() {
        let ctx = egui::Context::default();
        let mut value = false;
        let mut center = eframe::egui::Pos2::ZERO;
        let mut track = 0;
        let mut knob = 0;
        let frames = vec![
            Vec::new(),
            vec![
                egui::Event::PointerMoved(center),
                egui::Event::PointerButton {
                    pos: center,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            vec![egui::Event::PointerButton {
                pos: center,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ];
        for events in frames {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(240.0, 60.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let response = super::switch(ui, &mut value);
                    center = response.rect.center();
                },
            );
            for clipped in &output.shapes {
                match &clipped.shape {
                    egui::Shape::Rect(rect)
                        if rect.corner_radius == egui::CornerRadius::same(11)
                            && rect.fill.a() == 255 =>
                    {
                        track += 1;
                    }
                    egui::Shape::Circle(circle)
                        if (circle.radius - 7.5).abs() < 0.01
                            && circle.fill == egui::Color32::WHITE =>
                    {
                        knob += 1;
                    }
                    _ => {}
                }
            }
        }
        assert!(track >= 3, "switch track must be painted every frame");
        assert!(knob >= 3, "switch knob must be painted every frame");
        assert!(value, "clicking the switch must toggle it on");
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
                "守密人行动终端 准备执行视觉流程 工作台 识别目标 搜索范围 点击动作 等待策略 运行前检查 拖动左侧手柄排序 点击窗口比例位置 横向 纵向 客户区 简洁模式 专家模式 主流程 子流程库 调用 物品 优先级 禁止 允许随机 候选区域 数据不足 风险 售罄 嵌套 阶段标志"
            ));
        });
    }
}
