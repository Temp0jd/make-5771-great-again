//! Decorative background art (character portraits).
//!
//! Files live in `assets/art/<category>/` and are embedded by the list
//! generated in `build.rs`, so adding art means adding a PNG and rebuilding.
//! Portraits are only decoration: they are downscaled for display, drawn with a
//! low alpha so text contrast is untouched, and can be switched off.

use eframe::egui;
use serde::{Deserialize, Serialize};

/// Strongest alpha the user can select. The portrait owns a band beside the page
/// content, so nothing is drawn on top of it and this only has to look right.
pub const MAX_PORTRAIT_ALPHA: f32 = 0.65;

/// Share of the panel width reserved for the portrait, clamped so the band stays
/// useful on small windows and does not eat a wide screen.
const PORTRAIT_BAND_SHARE: f32 = 0.27;
const PORTRAIT_BAND_MIN: f32 = 170.0;
const PORTRAIT_BAND_MAX: f32 = 340.0;
/// Below this panel width the band would squeeze the page content too much, so
/// the portrait is skipped entirely.
const PORTRAIT_MIN_PANEL_WIDTH: f32 = 1000.0;
/// The figure sits on the bottom edge of the band; nothing bleeds into the
/// navigation bar or the page content.
const PORTRAIT_BOTTOM_BLEED: f32 = 0.0;
/// Only the inner edge and the very top fade, so the figure stays recognisable.
const PORTRAIT_INNER_FADE: f32 = 0.35;
const PORTRAIT_TOP_FADE: f32 = 0.10;

/// Longest edge uploaded for a portrait; keeps GPU memory small.
const PORTRAIT_MAX_SIZE: u32 = 512;

/// The fade cap is part of the readability contract, so it is checked at
/// compile time as well as in the unit tests.
const _: () = assert!(MAX_PORTRAIT_ALPHA <= 0.70);

include!(concat!(env!("OUT_DIR"), "/art_assets.rs"));

/// How strongly the background portrait shows through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PortraitStrength {
    Subtle,
    #[default]
    Medium,
    Strong,
}

impl PortraitStrength {
    pub const ALL: [Self; 3] = [Self::Subtle, Self::Medium, Self::Strong];

    pub fn alpha(self) -> f32 {
        match self {
            Self::Subtle => 0.35,
            Self::Medium => 0.50,
            Self::Strong => 0.65,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Subtle => "淡（35%）",
            Self::Medium => "中（50%）",
            Self::Strong => "强（65%）",
        }
    }
}

pub struct Portrait {
    pub id: String,
    pub texture: egui::TextureHandle,
}

pub struct BackgroundArt {
    portraits: Vec<Portrait>,
}

impl BackgroundArt {
    pub fn load(ctx: &egui::Context) -> Self {
        let mut portraits = Vec::new();
        for (id, bytes) in PORTRAITS {
            let Ok(image) = image::load_from_memory(bytes) else {
                continue;
            };
            let mut image = image.into_rgba8();
            if image.width().max(image.height()) > PORTRAIT_MAX_SIZE {
                let scale = PORTRAIT_MAX_SIZE as f32 / image.width().max(image.height()) as f32;
                let width = ((image.width() as f32 * scale).round() as u32).max(1);
                let height = ((image.height() as f32 * scale).round() as u32).max(1);
                image = image::imageops::thumbnail(&image, width, height);
            }
            let size = [image.width() as usize, image.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
            let texture = ctx.load_texture(
                format!("background-art-{id}"),
                color,
                egui::TextureOptions::LINEAR,
            );
            portraits.push(Portrait {
                id: (*id).to_owned(),
                texture,
            });
        }
        Self { portraits }
    }

    pub fn is_empty(&self) -> bool {
        self.portraits.is_empty()
    }

    #[allow(dead_code, reason = "used by tests and future art pickers")]
    pub fn ids(&self) -> Vec<&str> {
        self.portraits
            .iter()
            .map(|entry| entry.id.as_str())
            .collect()
    }

    /// Replaces every texture with one solid colour: keeps the layout while
    /// removing the artwork, which is how the preview tests isolate the art and
    /// measure exactly where it is painted.
    #[cfg(test)]
    pub(crate) fn make_solid(&mut self, ctx: &egui::Context, color: egui::Color32, tag: &str) {
        let solid = egui::ColorImage::new([1, 1], vec![color]);
        for (index, entry) in self.portraits.iter_mut().enumerate() {
            entry.texture = ctx.load_texture(
                format!("background-art-{tag}-{index}"),
                solid.clone(),
                egui::TextureOptions::LINEAR,
            );
        }
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.portraits.iter().position(|entry| entry.id == id)
    }

    pub fn len(&self) -> usize {
        self.portraits.len()
    }

    pub fn portrait(&self, index: usize) -> Option<&Portrait> {
        if self.portraits.is_empty() {
            None
        } else {
            self.portraits.get(index % self.portraits.len())
        }
    }
}

/// Alpha fade: full strength across the band, softening only the inner edge and
/// the very top so the figure blends into the page instead of ending abruptly.
pub fn fade_alpha(max_alpha: f32, horizontal: f32, vertical: f32) -> f32 {
    let clamp = |value: f32| value.clamp(0.0, 1.0);
    // `horizontal` is 1 at the outer edge and 0 at the inner edge.
    let horizontal = clamp(horizontal);
    let inner = (horizontal / PORTRAIT_INNER_FADE).clamp(0.0, 1.0);
    let top = (clamp(vertical) / PORTRAIT_TOP_FADE)
        .clamp(0.0, 1.0)
        .powf(0.7);
    (max_alpha * inner.powf(0.6) * top).clamp(0.0, 1.0)
}

/// Area reserved for the portrait on one side of the panel.
///
/// The page content is laid out next to this band, so the art is never covered
/// by the cards. Returns an empty rect when the window is too narrow.
pub fn portrait_band(panel: egui::Rect, left: bool, enabled: bool) -> egui::Rect {
    if !enabled || panel.width() < PORTRAIT_MIN_PANEL_WIDTH {
        return egui::Rect::from_min_size(panel.min, egui::Vec2::ZERO);
    }
    let width = (panel.width() * PORTRAIT_BAND_SHARE).clamp(PORTRAIT_BAND_MIN, PORTRAIT_BAND_MAX);
    let rect = if left {
        egui::Rect::from_min_max(
            panel.left_top(),
            egui::pos2(panel.left() + width, panel.bottom()),
        )
    } else {
        egui::Rect::from_min_max(
            egui::pos2(panel.right() - width, panel.top()),
            panel.right_bottom(),
        )
    };
    rect.intersect(panel)
}

/// Placement of the portrait inside a band; public for tests.
///
/// The band width decides the size (the figure fits the band exactly, so it can
/// never overlap the page content) and the art is anchored to the bottom, which
/// bleeds a little past the band.
pub fn portrait_rect(band: egui::Rect, source: egui::Vec2, left: bool) -> egui::Rect {
    let width = band.width();
    let height = if source.x > 0.0 {
        width * (source.y / source.x)
    } else {
        band.height()
    };
    let bottom = band.bottom() + band.height() * PORTRAIT_BOTTOM_BLEED;
    let horizontal = if left {
        (band.left(), band.left() + width)
    } else {
        (band.right() - width, band.right())
    };
    egui::Rect::from_min_max(
        egui::pos2(horizontal.0, bottom - height),
        egui::pos2(horizontal.1, bottom),
    )
}

/// Paints the portrait across its band, keeping the source aspect ratio.
pub fn paint_portrait(
    painter: &egui::Painter,
    texture: &egui::TextureHandle,
    left: bool,
    max_alpha: f32,
    band: egui::Rect,
) {
    let source = texture.size_vec2();
    if source.x <= 0.0 || source.y <= 0.0 || band.width() <= 0.0 || band.height() <= 0.0 {
        return;
    }
    let rect = portrait_rect(band, source, left);
    // Never paint outside the band: the bottom navigation and the page content
    // must stay untouched even if the artwork is an unexpected shape.
    let painter = &painter.with_clip_rect(band.intersect(painter.clip_rect()));

    // The fade is evaluated per grid vertex: a four-vertex quad would interpolate
    // the corner alphas linearly across the whole portrait, which washed the art
    // out instead of only softening its edges.
    const GRID: usize = 8;
    let mut mesh = egui::Mesh::with_texture(texture.id());
    for row in 0..=GRID {
        let ty = row as f32 / GRID as f32;
        let y = rect.top() + ty * rect.height();
        // Vertical fade is relative to the band, so only the strip near the top of
        // the panel is softened.
        let vertical = ((y - band.top()) / band.height()).clamp(0.0, 1.0);
        for column in 0..=GRID {
            let tx = column as f32 / GRID as f32;
            let x = rect.left() + tx * rect.width();
            // 1 at the outer edge, 0 at the inner edge.
            let horizontal = if left { 1.0 - tx } else { tx };
            mesh.vertices.push(egui::epaint::Vertex {
                pos: egui::pos2(x, y),
                uv: egui::pos2(tx, ty),
                color: egui::Color32::WHITE
                    .gamma_multiply(fade_alpha(max_alpha, horizontal, vertical)),
            });
        }
    }
    for row in 0..GRID {
        for column in 0..GRID {
            let top_left = (row * (GRID + 1) + column) as u32;
            let top_right = top_left + 1;
            let bottom_left = ((row + 1) * (GRID + 1) + column) as u32;
            let bottom_right = bottom_left + 1;
            mesh.indices.extend_from_slice(&[
                top_left,
                top_right,
                bottom_right,
                top_left,
                bottom_right,
                bottom_left,
            ]);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::{BackgroundArt, PORTRAITS, fade_alpha};

    #[test]
    fn generated_asset_list_is_available() {
        assert!(
            !PORTRAITS.is_empty(),
            "build.rs must embed assets/art/portraits/*.png"
        );
        // A real frame must load, which also proves the bytes are a valid image.
        let ctx = eframe::egui::Context::default();
        let art = BackgroundArt::load(&ctx);
        assert_eq!(art.len(), PORTRAITS.len());
        assert!(art.ids().iter().all(|id| !id.is_empty()));
        assert!(art.index_of(art.ids()[0]).is_some());
        // Out-of-range indices wrap instead of panicking.
        assert!(art.portrait(art.len() + 3).is_some());
    }

    #[test]
    fn fade_alpha_keeps_the_figure_and_softens_the_edges() {
        let strong = super::PortraitStrength::Strong.alpha();
        // Far from the faded edges the art keeps its full strength.
        let middle = fade_alpha(strong, 0.6, 0.5);
        assert_eq!(middle, strong);
        // The inner edge fades out completely, and so does the very top.
        assert_eq!(fade_alpha(strong, 0.0, 0.5), 0.0);
        assert_eq!(fade_alpha(strong, 1.0, 0.0), 0.0);
        // Half way through the inner fade strip the alpha is part way down.
        let half = fade_alpha(strong, super::PORTRAIT_INNER_FADE / 2.0, 1.0);
        assert!(half > 0.0 && half < strong, "half-way alpha was {half}");
        // Values outside 0..1 are clamped, so a mis-placed edge cannot boost alpha.
        assert_eq!(fade_alpha(strong, 5.0, 5.0), strong);
        assert!(fade_alpha(strong, 0.5, 0.5) <= strong);
    }

    #[test]
    fn band_reserves_space_beside_the_page_content() {
        let panel = eframe::egui::Rect::from_min_size(
            eframe::egui::Pos2::ZERO,
            eframe::egui::vec2(1252.0, 708.0),
        );
        for left in [true, false] {
            let band = super::portrait_band(panel, left, true);
            assert!(band.width() >= super::PORTRAIT_BAND_MIN);
            assert!(band.width() <= super::PORTRAIT_BAND_MAX);
            // The band covers the full panel height and stays inside it.
            assert_eq!(band.height(), panel.height());
            assert!(panel.contains_rect(band));
            if left {
                assert_eq!(band.left(), panel.left());
            } else {
                assert_eq!(band.right(), panel.right());
            }
            // The figure fits the band width, so it cannot reach the content.
            let rect = super::portrait_rect(band, eframe::egui::vec2(339.0, 512.0), left);
            // Fits the band width exactly, so it can never reach the content, and
            // sits on the band's bottom edge without bleeding into the nav bar.
            assert!((rect.width() - band.width()).abs() < 0.5);
            assert!((rect.bottom() - band.bottom()).abs() < 0.5);
            assert!(rect.left() >= band.left() - 0.5 && rect.right() <= band.right() + 0.5);
            assert!(rect.top() >= band.top());
        }

        // Disabled, or too narrow to keep the pages usable: no band at all.
        assert_eq!(super::portrait_band(panel, true, false).width(), 0.0);
        let narrow = eframe::egui::Rect::from_min_size(
            eframe::egui::Pos2::ZERO,
            eframe::egui::vec2(760.0, 600.0),
        );
        assert_eq!(super::portrait_band(narrow, true, true).width(), 0.0);
    }

    #[test]
    fn band_reserves_space_beside_the_page_content_bounds() {
        let panel = eframe::egui::Rect::from_min_size(
            eframe::egui::Pos2::ZERO,
            eframe::egui::vec2(1252.0, 708.0),
        );
        let band = super::portrait_band(panel, false, true);
        // Content is laid out to the left of the band, so the two never overlap.
        let content_right = band.min.x - 8.0;
        assert!(content_right > panel.left());
        assert!(content_right < band.min.x);
    }
}
