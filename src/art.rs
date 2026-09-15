//! Decorative background art (character portraits).
//!
//! Files live in `assets/art/<category>/` and are embedded by the list
//! generated in `build.rs`, so adding art means adding a PNG and rebuilding.
//! Portraits are only decoration: they are downscaled for display, drawn with a
//! low alpha so text contrast is untouched, and can be switched off.

use eframe::egui;

/// Alpha at the corner before the fade; kept low so labels stay readable even
/// where the art is brightest.
pub const MAX_PORTRAIT_ALPHA: f32 = 0.16;

/// Longest edge uploaded for a portrait; keeps GPU memory small.
const PORTRAIT_MAX_SIZE: u32 = 512;

/// The fade cap is part of the readability contract, so it is checked at
/// compile time as well as in the unit tests.
const _: () = assert!(MAX_PORTRAIT_ALPHA <= 0.2);

include!(concat!(env!("OUT_DIR"), "/art_assets.rs"));

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

/// Alpha for one corner of the fade: strongest at the corner, transparent
/// towards the middle of the window.
pub fn fade_alpha(max_alpha: f32, horizontal: f32, vertical: f32) -> f32 {
    let clamp = |value: f32| value.clamp(0.0, 1.0);
    max_alpha * clamp(horizontal) * clamp(vertical)
}

/// Paints a portrait in one corner with a soft two-way fade.
///
/// `left` puts it in the bottom-left corner, otherwise bottom-right.
pub fn paint_portrait(
    painter: &egui::Painter,
    panel: egui::Rect,
    texture: &egui::TextureHandle,
    left: bool,
) {
    let source = texture.size_vec2();
    if source.x <= 0.0 || source.y <= 0.0 {
        return;
    }
    let height = (panel.height() * 0.72).min(panel.width() * 0.62);
    let width = height * (source.x / source.y);
    let rect = if left {
        egui::Rect::from_min_max(
            egui::pos2(panel.left() - width * 0.06, panel.bottom() - height),
            egui::pos2(panel.left() + width * 0.94, panel.bottom() + height * 0.04),
        )
    } else {
        egui::Rect::from_min_max(
            egui::pos2(panel.right() - width * 0.94, panel.bottom() - height),
            egui::pos2(panel.right() + width * 0.06, panel.bottom() + height * 0.04),
        )
    };

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
            color: egui::Color32::WHITE.gamma_multiply(fade_alpha(
                MAX_PORTRAIT_ALPHA,
                horizontal,
                vertical,
            )),
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::{BackgroundArt, MAX_PORTRAIT_ALPHA, PORTRAITS, fade_alpha};

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
        assert_eq!(fade_alpha(MAX_PORTRAIT_ALPHA, 1.0, 1.0), MAX_PORTRAIT_ALPHA);
        assert_eq!(fade_alpha(MAX_PORTRAIT_ALPHA, 0.0, 1.0), 0.0);
        assert_eq!(fade_alpha(MAX_PORTRAIT_ALPHA, 1.0, 0.0), 0.0);
        // Values outside 0..1 are clamped, so a mis-placed corner cannot boost alpha.
        assert_eq!(fade_alpha(MAX_PORTRAIT_ALPHA, 5.0, 5.0), MAX_PORTRAIT_ALPHA);
        assert!(fade_alpha(MAX_PORTRAIT_ALPHA, 0.5, 0.5) <= MAX_PORTRAIT_ALPHA);
    }
}
