use std::path::Path;

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use image::RgbaImage;

use crate::theme;
use crate::vision::{SearchRegion, TemplateMatch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelSelection {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub enum EditorAction {
    None,
    Cancel,
    Save {
        name: String,
        selection: PixelSelection,
    },
}

pub struct TemplateDraft {
    pub image: RgbaImage,
    texture: egui::TextureHandle,
    source_label: String,
    name: String,
    selection: Option<PixelSelection>,
    drag_start: Option<(u32, u32)>,
}

pub struct TemplateTestView {
    texture: egui::TextureHandle,
    mascot: egui::TextureHandle,
    image_width: u32,
    image_height: u32,
    template_name: String,
    search_region: SearchRegion,
    result: Option<TemplateMatch>,
    threshold: f32,
}

impl TemplateTestView {
    pub fn new(
        ctx: &egui::Context,
        image: &RgbaImage,
        template_name: impl Into<String>,
        search_region: SearchRegion,
        result: Option<TemplateMatch>,
        threshold: f32,
        mascot: egui::TextureHandle,
    ) -> Self {
        let size = [image.width() as usize, image.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        Self {
            texture: ctx.load_texture(
                "template-test-frame",
                color_image,
                egui::TextureOptions::LINEAR,
            ),
            mascot,
            image_width: image.width(),
            image_height: image.height(),
            template_name: template_name.into(),
            search_region,
            result,
            threshold,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut open = true;
        egui::Window::new("模板识别测试")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(820.0)
            .default_height(580.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(&self.mascot).fit_to_exact_size(Vec2::splat(36.0)));
                    ui.label(RichText::new(&self.template_name).strong());
                    ui.label(
                        RichText::new(format!("阈值 {:.2}", self.threshold))
                            .color(theme::SECONDARY_LABEL),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (label, color) = match self.result {
                            Some(found) => (format!("匹配成功 · {:.3}", found.score), theme::GREEN),
                            None => ("未达到阈值".to_owned(), Color32::from_rgb(255, 59, 48)),
                        };
                        ui.label(RichText::new(label).color(color).strong());
                    });
                });
                ui.label(
                    RichText::new("橙色框为搜索区域，绿色框为最佳匹配位置")
                        .size(12.0)
                        .color(theme::TERTIARY_LABEL),
                );
                ui.add_space(6.0);

                let available = ui.available_size();
                let max_size = Vec2::new(available.x.max(240.0), (available.y - 50.0).max(220.0));
                let scale = (max_size.x / self.image_width as f32)
                    .min(max_size.y / self.image_height as f32)
                    .min(1.0);
                let display_size = Vec2::new(
                    self.image_width as f32 * scale,
                    self.image_height as f32 * scale,
                );
                let response = ui.add(
                    egui::Image::new(&self.texture)
                        .fit_to_exact_size(display_size)
                        .sense(Sense::hover()),
                );

                let region_rect = search_region_to_display(
                    self.search_region,
                    response.rect,
                    self.image_width,
                    self.image_height,
                );
                ui.painter().rect_stroke(
                    region_rect,
                    4.0,
                    Stroke::new(2.0, theme::ORANGE),
                    egui::StrokeKind::Inside,
                );
                if let Some(found) = self.result {
                    let match_rect = search_region_to_display(
                        SearchRegion {
                            x: found.x,
                            y: found.y,
                            width: found.width,
                            height: found.height,
                        },
                        response.rect,
                        self.image_width,
                        self.image_height,
                    );
                    ui.painter().rect_stroke(
                        match_rect,
                        4.0,
                        Stroke::new(3.0, theme::GREEN),
                        egui::StrokeKind::Inside,
                    );
                }
            });
        open
    }
}

impl TemplateDraft {
    pub fn from_path(ctx: &egui::Context, path: &Path) -> Result<Self, String> {
        let image = image::open(path)
            .map_err(|error| format!("无法读取图片：{error}"))?
            .into_rgba8();
        let source_label = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("导入截图")
            .to_owned();
        let suggested_name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("新模板")
            .to_owned();
        Ok(Self::from_image(ctx, image, source_label, suggested_name))
    }

    pub fn from_image(
        ctx: &egui::Context,
        image: RgbaImage,
        source_label: impl Into<String>,
        suggested_name: impl Into<String>,
    ) -> Self {
        let size = [image.width() as usize, image.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        let source_label = source_label.into();
        let texture = ctx.load_texture(
            format!("template-draft-{source_label}"),
            color_image,
            egui::TextureOptions::LINEAR,
        );
        Self {
            image,
            texture,
            source_label,
            name: suggested_name.into(),
            selection: None,
            drag_start: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> EditorAction {
        let mut action = EditorAction::None;
        let mut open = true;
        egui::Window::new("框选图片模板")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(820.0)
            .default_height(600.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!(
                        "{} · {} × {}",
                        self.source_label,
                        self.image.width(),
                        self.image.height()
                    ))
                    .color(theme::SECONDARY_LABEL),
                );
                ui.label("在图片上拖动框选识别目标，尽量只包含稳定的按钮或文字区域。");
                ui.add_space(6.0);

                let available = ui.available_size();
                let max_size = Vec2::new(available.x.max(240.0), (available.y - 130.0).max(220.0));
                let scale = (max_size.x / self.image.width() as f32)
                    .min(max_size.y / self.image.height() as f32)
                    .min(1.0);
                let display_size = Vec2::new(
                    self.image.width() as f32 * scale,
                    self.image.height() as f32 * scale,
                );
                let response = ui.add(
                    egui::Image::new(&self.texture)
                        .fit_to_exact_size(display_size)
                        .sense(Sense::click_and_drag()),
                );

                if response.drag_started()
                    && let Some(position) = response.interact_pointer_pos()
                {
                    let pixel = display_to_pixel(position, response.rect, &self.image);
                    self.drag_start = Some(pixel);
                    self.selection = Some(PixelSelection {
                        x: pixel.0,
                        y: pixel.1,
                        width: 1,
                        height: 1,
                    });
                }
                if response.dragged()
                    && let (Some(start), Some(position)) =
                        (self.drag_start, response.interact_pointer_pos())
                {
                    let end = display_to_pixel(position, response.rect, &self.image);
                    self.selection = selection_between(start, end, &self.image);
                }
                if response.drag_stopped() {
                    self.drag_start = None;
                }

                if let Some(selection) = self.selection {
                    let selection_rect = pixel_to_display(selection, response.rect, &self.image);
                    ui.painter().rect_stroke(
                        selection_rect,
                        5.0,
                        Stroke::new(2.0, theme::BLUE),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().rect_filled(
                        Rect::from_min_size(selection_rect.min, Vec2::new(118.0, 24.0)),
                        5.0,
                        theme::BLUE,
                    );
                    ui.painter().text(
                        selection_rect.min + Vec2::new(7.0, 5.0),
                        egui::Align2::LEFT_TOP,
                        format!("{} × {}", selection.width, selection.height),
                        egui::FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("模板名称");
                    ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(220.0));
                    if ui.button("使用整张图片").clicked() {
                        self.selection = Some(PixelSelection {
                            x: 0,
                            y: 0,
                            width: self.image.width(),
                            height: self.image.height(),
                        });
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let can_save = self.selection.is_some() && !self.name.trim().is_empty();
                        if ui
                            .add_enabled(can_save, theme::primary_button("保存模板"))
                            .clicked()
                            && let Some(selection) = self.selection
                        {
                            action = EditorAction::Save {
                                name: self.name.trim().to_owned(),
                                selection,
                            };
                        }
                    });
                });
            });

        if !open { EditorAction::Cancel } else { action }
    }
}

fn display_to_pixel(position: Pos2, display: Rect, image: &RgbaImage) -> (u32, u32) {
    let normalized_x = ((position.x - display.left()) / display.width()).clamp(0.0, 1.0);
    let normalized_y = ((position.y - display.top()) / display.height()).clamp(0.0, 1.0);
    (
        (normalized_x * image.width().saturating_sub(1) as f32).round() as u32,
        (normalized_y * image.height().saturating_sub(1) as f32).round() as u32,
    )
}

fn selection_between(
    start: (u32, u32),
    end: (u32, u32),
    image: &RgbaImage,
) -> Option<PixelSelection> {
    let x = start.0.min(end.0);
    let y = start.1.min(end.1);
    let width = start.0.max(end.0).saturating_sub(x).max(1);
    let height = start.1.max(end.1).saturating_sub(y).max(1);
    if x + width <= image.width() && y + height <= image.height() {
        Some(PixelSelection {
            x,
            y,
            width,
            height,
        })
    } else {
        None
    }
}

fn pixel_to_display(selection: PixelSelection, display: Rect, image: &RgbaImage) -> Rect {
    let scale_x = display.width() / image.width() as f32;
    let scale_y = display.height() / image.height() as f32;
    Rect::from_min_size(
        display.min + Vec2::new(selection.x as f32 * scale_x, selection.y as f32 * scale_y),
        Vec2::new(
            selection.width as f32 * scale_x,
            selection.height as f32 * scale_y,
        ),
    )
}

fn search_region_to_display(
    region: SearchRegion,
    display: Rect,
    image_width: u32,
    image_height: u32,
) -> Rect {
    let scale_x = display.width() / image_width as f32;
    let scale_y = display.height() / image_height as f32;
    Rect::from_min_size(
        display.min + Vec2::new(region.x as f32 * scale_x, region.y as f32 * scale_y),
        Vec2::new(
            region.width as f32 * scale_x,
            region.height as f32 * scale_y,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_is_normalized_in_any_drag_direction() {
        let image = RgbaImage::new(100, 80);
        let selection = selection_between((70, 60), (20, 10), &image).unwrap();
        assert_eq!(
            selection,
            PixelSelection {
                x: 20,
                y: 10,
                width: 50,
                height: 50,
            }
        );
    }
}
