//! Decorative background art (character portraits).
//!
//! Files live in `assets/art/<category>/` and are embedded by the list
//! generated in `build.rs`, so adding art means adding a PNG and rebuilding.
//! Portraits are only decoration: they are downscaled for display, drawn with a
//! low alpha so text contrast is untouched, and can be switched off.

use eframe::egui;
use serde::{Deserialize, Serialize};

/// Strongest alpha the user can select. Headings sit at the top of the page
/// where the vertical fade has already faded the art out, and content sits on
/// opaque-enough cards, so this stays readable.
pub const MAX_PORTRAIT_ALPHA: f32 = 0.60;

/// How much of the window the portrait covers, including the part that bleeds
/// past the edges. Larger values make the character clearly visible.
const PORTRAIT_HEIGHT_FACTOR: f32 = 1.12;
const PORTRAIT_OUTER_BLEED: f32 = 0.10;
const PORTRAIT_BOTTOM_BLEED: f32 = 0.08;
/// Vertical fade start: the head keeps a little presence instead of vanishing.
const PORTRAIT_TOP_FLOOR: f32 = 0.12;

/// Longest edge uploaded for a portrait; keeps GPU memory small.
const PORTRAIT_MAX_SIZE: u32 = 512;

/// The fade cap is part of the readability contract, so it is checked at
/// compile time as well as in the unit tests.
const _: () = assert!(MAX_PORTRAIT_ALPHA <= 0.65);

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
            Self::Subtle => 0.20,
            Self::Medium => 0.42,
            Self::Strong => 0.60,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Subtle => "淡（20%）",
            Self::Medium => "中（42%）",
            Self::Strong => "强（60%）",
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

/// Alpha for one corner of the fade: strongest at the outer corner, transparent
/// towards the middle of the window. The horizontal falloff is slower than
/// linear so more of the character stays visible.
pub fn fade_alpha(max_alpha: f32, horizontal: f32, vertical: f32) -> f32 {
    let clamp = |value: f32| value.clamp(0.0, 1.0);
    let horizontal = clamp(horizontal).powf(0.7);
    let vertical = PORTRAIT_TOP_FLOOR + (1.0 - PORTRAIT_TOP_FLOOR) * clamp(vertical).powf(1.2);
    (max_alpha * horizontal * vertical).clamp(0.0, 1.0)
}

/// Placement of the portrait inside the panel; public for tests.
pub fn portrait_rect(panel: egui::Rect, source: egui::Vec2, left: bool) -> egui::Rect {
    let height = (panel.height() * PORTRAIT_HEIGHT_FACTOR).min(panel.width() * 1.4);
    let width = if source.y > 0.0 {
        height * (source.x / source.y)
    } else {
        height * 0.66
    };
    let bottom = panel.bottom() + height * PORTRAIT_BOTTOM_BLEED;
    let top = bottom - height;
    if left {
        egui::Rect::from_min_max(
            egui::pos2(panel.left() - width * PORTRAIT_OUTER_BLEED, top),
            egui::pos2(panel.left() + width * (1.0 - PORTRAIT_OUTER_BLEED), bottom),
        )
    } else {
        egui::Rect::from_min_max(
            egui::pos2(panel.right() - width * (1.0 - PORTRAIT_OUTER_BLEED), top),
            egui::pos2(panel.right() + width * PORTRAIT_OUTER_BLEED, bottom),
        )
    }
}

/// Paints a portrait in one corner with a soft two-way fade.
///
/// `left` puts it in the bottom-left corner, otherwise bottom-right.
pub fn paint_portrait(
    painter: &egui::Painter,
    panel: egui::Rect,
    texture: &egui::TextureHandle,
    left: bool,
    max_alpha: f32,
) {
    let source = texture.size_vec2();
    if source.x <= 0.0 || source.y <= 0.0 {
        return;
    }
    let rect = portrait_rect(panel, source, left);

    // Fade horizontally towards the window centre and vertically towards the top.
    let (x0, x1) = if left {
        (1.0_f32, 0.0_f32)
    } else {
        (0.0_f32, 1.0_f32)
    };
    let (y0, y1) = (0.0_f32, 1.0_f32);
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    let mut mesh = egui::Mesh::with_texture(texture.id());
    let corners = [
        (rect.left_top(), x0, y0, uv.left_top()),
        (rect.right_top(), x1, y0, uv.right_top()),
        (rect.right_bottom(), x1, y1, uv.right_bottom()),
        (rect.left_bottom(), x0, y1, uv.left_bottom()),
    ];
    for (position, horizontal, vertical, uv_point) in corners {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: position,
            uv: uv_point,
            color: egui::Color32::WHITE.gamma_multiply(fade_alpha(max_alpha, horizontal, vertical)),
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
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
    fn fade_alpha_stays_within_the_readability_budget() {
        let strong = super::PortraitStrength::Strong.alpha();
        assert_eq!(fade_alpha(strong, 1.0, 1.0), strong);
        // The inner edge still fades out completely.
        assert_eq!(fade_alpha(strong, 0.0, 1.0), 0.0);
        // The top keeps a small floor so the character is recognisable.
        let top = fade_alpha(strong, 1.0, 0.0);
        assert!(top > 0.0 && top < strong);
        // Values outside 0..1 are clamped, so a mis-placed corner cannot boost alpha.
        assert_eq!(fade_alpha(strong, 5.0, 5.0), strong);
        assert!(fade_alpha(strong, 0.5, 0.5) <= strong);
        // Headings live at the top of the page, where the art is nearly gone.
        assert!(fade_alpha(strong, 1.0, 0.08) <= 0.12);
    }

    #[test]
    fn portrait_cover_is_large_and_bleeds_past_the_panel() {
        let panel = eframe::egui::Rect::from_min_size(
            eframe::egui::Pos2::ZERO,
            eframe::egui::vec2(900.0, 700.0),
        );
        let source = eframe::egui::vec2(339.0, 512.0);
        for left in [true, false] {
            let rect = super::portrait_rect(panel, source, left);
            assert!(
                rect.height() > panel.height(),
                "the portrait must cover the full height"
            );
            assert!(
                rect.width() > panel.width() * 0.5,
                "the portrait must cover a large share of the width"
            );
            assert!(rect.bottom() > panel.bottom(), "bottom edge must bleed off");
            if left {
                assert!(rect.left() < panel.left(), "outer edge must bleed off");
            } else {
                assert!(rect.right() > panel.right(), "outer edge must bleed off");
            }
        }
    }
}
