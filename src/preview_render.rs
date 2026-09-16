//! Headless software rasteriser for egui frames (test-only).
//!
//! The UI is developed on a host where the Windows build cannot be launched, so
//! "is the decoration actually visible?" could only be reasoned about, never
//! checked. This module replays egui's own tessellation output with a small CPU
//! rasteriser that mirrors egui's shader (premultiplied colours, coverage
//! textures for glyphs), which is enough to review layout, transparency and
//! whether a decorative layer is visible at all.
#![cfg(test)]

use std::collections::HashMap;

use eframe::egui::{self, Color32, TextureId};

/// Textures uploaded during the frame, including the font atlas.
type Textures = HashMap<TextureId, Texture>;

struct Texture {
    size: [usize; 2],
    /// 1 byte per pixel (coverage, used for glyphs) or 4 (RGBA).
    bytes_per_pixel: usize,
    pixels: Vec<u8>,
}

impl Texture {
    fn apply(&mut self, delta: &egui::epaint::ImageDelta) {
        let egui::epaint::ImageData::Color(image) = &delta.image;
        let width = image.width();
        let height = image.height();
        let raw: Vec<u8> = image
            .pixels
            .iter()
            .flat_map(|color| color.to_array())
            .collect();
        match delta.pos {
            None => {
                self.size = [width, height];
                self.bytes_per_pixel = 4;
                self.pixels = raw;
            }
            Some([x, y]) => {
                // Partial update (the font atlas grows glyph by glyph).
                self.bytes_per_pixel = 4;
                for row in 0..height {
                    let target_y = y + row;
                    if target_y >= self.size[1] {
                        continue;
                    }
                    let source = row * width * 4;
                    let target = (target_y * self.size[0] + x) * 4;
                    let length = (width * 4).min(self.pixels.len().saturating_sub(target));
                    if length == 0 {
                        continue;
                    }
                    self.pixels[target..target + length]
                        .copy_from_slice(&raw[source..source + length]);
                }
            }
        }
    }

    /// Bilinear sample in texel space; `wrap` clamps.
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let width = self.size[0] as i64;
        let height = self.size[1] as i64;
        if width <= 0 || height <= 0 || self.pixels.is_empty() {
            return [1.0, 1.0, 1.0, 1.0];
        }
        let x = (u - 0.5).clamp(0.0, (width - 1) as f32);
        let y = (v - 0.5).clamp(0.0, (height - 1) as f32);
        let x0 = x.floor() as i64;
        let y0 = y.floor() as i64;
        let x1 = (x0 + 1).min(width - 1);
        let y1 = (y0 + 1).min(height - 1);
        let tx = x - x0 as f32;
        let ty = y - y0 as f32;
        let fetch = |px: i64, py: i64| -> [f32; 4] {
            let index = (py * width + px) as usize * self.bytes_per_pixel;
            if index >= self.pixels.len() {
                return [0.0, 0.0, 0.0, 0.0];
            }
            if self.bytes_per_pixel == 1 {
                // Coverage texture: alpha only, colour comes from the vertex.
                let alpha = self.pixels[index] as f32 / 255.0;
                [alpha, alpha, alpha, alpha]
            } else {
                [
                    self.pixels[index] as f32 / 255.0,
                    self.pixels[index + 1] as f32 / 255.0,
                    self.pixels[index + 2] as f32 / 255.0,
                    self.pixels[index + 3] as f32 / 255.0,
                ]
            }
        };
        let (c00, c10, c01, c11) = (fetch(x0, y0), fetch(x1, y0), fetch(x0, y1), fetch(x1, y1));
        let mut out = [0.0_f32; 4];
        for channel in 0..4 {
            let top = c00[channel] * (1.0 - tx) + c10[channel] * tx;
            let bottom = c01[channel] * (1.0 - tx) + c11[channel] * tx;
            out[channel] = top * (1.0 - ty) + bottom * ty;
        }
        out
    }

    /// A coverage texture must be tinted with the vertex colour instead of being
    /// multiplied channel-wise, matching egui's shader.
    fn is_coverage(&self) -> bool {
        self.bytes_per_pixel == 1
    }
}

fn to_linear(color: Color32) -> [f32; 4] {
    [
        color.r() as f32 / 255.0,
        color.g() as f32 / 255.0,
        color.b() as f32 / 255.0,
        color.a() as f32 / 255.0,
    ]
}

/// Keeps uploaded textures between frames, because egui only sends deltas.
#[derive(Default)]
pub(crate) struct FrameRenderer {
    textures: Textures,
}

impl FrameRenderer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Rasterises one frame to an RGBA image.
    pub(crate) fn draw(
        &mut self,
        ctx: &egui::Context,
        output: &egui::FullOutput,
        size: [usize; 2],
    ) -> image::RgbaImage {
        for (id, delta) in &output.textures_delta.set {
            self.textures
                .entry(*id)
                .or_insert_with(|| Texture {
                    size: [0, 0],
                    bytes_per_pixel: 4,
                    pixels: Vec::new(),
                })
                .apply(delta);
        }
        let textures = &self.textures;
        draw_frame(ctx, output, size, textures)
    }
}

fn draw_frame(
    ctx: &egui::Context,
    output: &egui::FullOutput,
    size: [usize; 2],
    textures: &Textures,
) -> image::RgbaImage {
    let pixels_per_point = output.pixels_per_point;
    let mut buffer = vec![[0.0_f32; 4]; size[0] * size[1]];
    // egui paints in order, so each primitive blends over what is already there.
    let primitives = ctx.tessellate(output.shapes.clone(), pixels_per_point);

    for primitive in primitives {
        let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        let Some(texture) = textures.get(&mesh.texture_id) else {
            continue;
        };
        let clip = primitive.clip_rect;
        let clip_min_x = ((clip.min.x * pixels_per_point).floor().max(0.0)) as usize;
        let clip_min_y = ((clip.min.y * pixels_per_point).floor().max(0.0)) as usize;
        let clip_max_x = ((clip.max.x * pixels_per_point).ceil().min(size[0] as f32)) as usize;
        let clip_max_y = ((clip.max.y * pixels_per_point).ceil().min(size[1] as f32)) as usize;

        for triangle in mesh.indices.as_chunks::<3>().0 {
            let vertices = [
                &mesh.vertices[triangle[0] as usize],
                &mesh.vertices[triangle[1] as usize],
                &mesh.vertices[triangle[2] as usize],
            ];
            let positions: Vec<(f32, f32)> = vertices
                .iter()
                .map(|vertex| {
                    (
                        vertex.pos.x * pixels_per_point,
                        vertex.pos.y * pixels_per_point,
                    )
                })
                .collect();
            let area = (positions[1].0 - positions[0].0) * (positions[2].1 - positions[0].1)
                - (positions[2].0 - positions[0].0) * (positions[1].1 - positions[0].1);
            if area.abs() < f32::EPSILON {
                continue;
            }

            let min_x = positions
                .iter()
                .map(|(x, _)| *x)
                .fold(f32::INFINITY, f32::min)
                .max(clip_min_x as f32)
                .max(0.0) as usize;
            let max_x = (positions
                .iter()
                .map(|(x, _)| *x)
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .min(clip_max_x as f32))
            .max(0.0) as usize;
            let min_y = positions
                .iter()
                .map(|(_, y)| *y)
                .fold(f32::INFINITY, f32::min)
                .max(clip_min_y as f32)
                .max(0.0) as usize;
            let max_y = (positions
                .iter()
                .map(|(_, y)| *y)
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .min(clip_max_y as f32))
            .max(0.0) as usize;

            for y in min_y..max_y.min(size[1]) {
                for x in min_x..max_x.min(size[0]) {
                    let px = x as f32 + 0.5;
                    let py = y as f32 + 0.5;
                    let w0 = ((positions[1].0 - px) * (positions[2].1 - py)
                        - (positions[2].0 - px) * (positions[1].1 - py))
                        / area;
                    let w1 = ((positions[2].0 - px) * (positions[0].1 - py)
                        - (positions[0].0 - px) * (positions[2].1 - py))
                        / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }

                    let u = w0 * vertices[0].uv.x + w1 * vertices[1].uv.x + w2 * vertices[2].uv.x;
                    let v = w0 * vertices[0].uv.y + w1 * vertices[1].uv.y + w2 * vertices[2].uv.y;
                    let texel =
                        texture.sample(u * texture.size[0] as f32, v * texture.size[1] as f32);
                    let weights = [w0, w1, w2];
                    let mut color = [0.0_f32; 4];
                    for (vertex, weight) in vertices.iter().zip(weights) {
                        let linear = to_linear(vertex.color);
                        for channel in 0..4 {
                            color[channel] += linear[channel] * weight;
                        }
                    }

                    let source = if texture.is_coverage() {
                        // Coverage textures replace the alpha of the vertex colour.
                        [
                            color[0] * texel[3],
                            color[1] * texel[3],
                            color[2] * texel[3],
                            color[3] * texel[3],
                        ]
                    } else {
                        [
                            color[0] * texel[0],
                            color[1] * texel[1],
                            color[2] * texel[2],
                            color[3] * texel[3],
                        ]
                    };

                    let target = &mut buffer[y * size[0] + x];
                    let inverse = 1.0 - source[3];
                    for channel in 0..4 {
                        target[channel] = source[channel] + target[channel] * inverse;
                    }
                }
            }
        }
    }

    let mut image = image::RgbaImage::new(size[0] as u32, size[1] as u32);
    for (index, pixel) in buffer.iter().enumerate() {
        let x = (index % size[0]) as u32;
        let y = (index / size[0]) as u32;
        let to_byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        image.put_pixel(
            x,
            y,
            image::Rgba([
                to_byte(pixel[0]),
                to_byte(pixel[1]),
                to_byte(pixel[2]),
                to_byte(pixel[3]),
            ]),
        );
    }
    image
}
