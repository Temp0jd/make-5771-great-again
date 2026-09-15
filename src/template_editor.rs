use std::path::Path;

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use image::RgbaImage;

use crate::theme;
use crate::vision::{SearchRegion, TemplateMatch};
use crate::workflow_ui::{self, DiagnosticSeverity};

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

pub enum RoiEditorAction {
    None,
    Cancel,
    Apply(PixelSelection),
    /// Skip the region entirely: search the whole client area.
    FullFrame,
}

/// Full-frame visual ROI picker used directly by workflow template uses.
pub struct RoiDraft {
    pub image: RgbaImage,
    texture: egui::TextureHandle,
    selection: Option<PixelSelection>,
    drag_start: Option<(u32, u32)>,
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
    best_score: f32,
    candidates: Vec<TemplateMatch>,
    threshold: f32,
    search_strategy: String,
    mostly_background: bool,
    expert_mode: bool,
}

impl TemplateTestView {
    #[allow(
        clippy::too_many_arguments,
        reason = "test diagnostics view model plus texture inputs"
    )]
    pub fn new(
        ctx: &egui::Context,
        image: &RgbaImage,
        template_name: impl Into<String>,
        search_region: SearchRegion,
        result: Option<TemplateMatch>,
        best_score: f32,
        candidates: Vec<TemplateMatch>,
        threshold: f32,
        search_strategy: impl Into<String>,
        mascot: egui::TextureHandle,
        mostly_background: bool,
        expert_mode: bool,
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
            best_score,
            candidates,
            threshold,
            search_strategy: search_strategy.into(),
            mostly_background,
            expert_mode,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut open = true;
        let diag = workflow_ui::analyze_test_outcome(
            self.result,
            self.best_score,
            self.threshold,
            &self.candidates,
            self.mostly_background,
            &self.search_strategy,
            self.expert_mode,
        );

        egui::Window::new("模板识别测试与诊断")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(840.0)
            .default_height(640.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(&self.mascot).fit_to_exact_size(Vec2::splat(36.0)));
                    ui.label(RichText::new(&self.template_name).size(16.0).strong());
                    ui.label(
                        RichText::new(format!("测试阈值 {:.2}", self.threshold))
                            .color(theme::secondary_label()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (label, color) = match self.result {
                            Some(found) => {
                                (format!("达到阈值 · {:.3}", found.score), theme::green())
                            }
                            None => (
                                format!("未命中 · 最佳 {:.3}", self.best_score),
                                Color32::from_rgb(255, 59, 48),
                            ),
                        };
                        ui.label(RichText::new(label).color(color).strong());
                    });
                });

                // Actionable diagnostic card
                theme::section_card().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (icon, color) = match diag.severity {
                            DiagnosticSeverity::Success => ("通过", theme::green()),
                            DiagnosticSeverity::Warning => ("注意", theme::orange()),
                            DiagnosticSeverity::Error => ("失败", theme::red()),
                        };
                        ui.label(
                            RichText::new(format!("{icon} {}", diag.title))
                                .color(color)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("范围策略：{}", self.search_strategy))
                                    .size(11.0)
                                    .color(theme::tertiary_label()),
                            );
                        });
                    });
                    ui.label(RichText::new(&diag.detail).size(12.0).color(theme::label()));
                    ui.label(
                        RichText::new(&diag.advice)
                            .size(11.5)
                            .color(theme::secondary_label()),
                    );
                    if let Some(expert_note) = &diag.expert_note {
                        ui.label(RichText::new(expert_note).size(11.0).color(theme::purple()));
                    }
                    if let Some(bg_warning) = &diag.background_warning {
                        ui.label(RichText::new(bg_warning).size(11.0).color(theme::orange()));
                    }
                });

                ui.add_space(4.0);
                ui.label(
                    RichText::new("橙色框为搜索区域；绿色为已接受目标；黄色为其他高分候选")
                        .size(11.5)
                        .color(theme::tertiary_label()),
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
                    Stroke::new(2.0, theme::orange()),
                    egui::StrokeKind::Inside,
                );
                for (index, candidate) in self.candidates.iter().take(5).enumerate() {
                    if self.result == Some(*candidate) {
                        continue;
                    }
                    let candidate_rect = search_region_to_display(
                        SearchRegion {
                            x: candidate.x,
                            y: candidate.y,
                            width: candidate.width,
                            height: candidate.height,
                        },
                        response.rect,
                        self.image_width,
                        self.image_height,
                    );
                    ui.painter().rect_stroke(
                        candidate_rect,
                        4.0,
                        Stroke::new(2.0, Color32::from_rgb(255, 196, 64)),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        candidate_rect.min,
                        egui::Align2::LEFT_TOP,
                        format!("{} · {:.3}", index + 1, candidate.score),
                        egui::FontId::proportional(11.0),
                        Color32::from_rgb(255, 196, 64),
                    );
                }
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
                        Stroke::new(3.0, theme::green()),
                        egui::StrokeKind::Inside,
                    );
                }
            });
        open
    }
}

fn clamp_selection(selection: PixelSelection, image: (u32, u32)) -> PixelSelection {
    let (image_width, image_height) = image;
    let width = selection.width.clamp(8, image_width.max(8));
    let height = selection.height.clamp(8, image_height.max(8));
    PixelSelection {
        x: selection.x.min(image_width.saturating_sub(width)),
        y: selection.y.min(image_height.saturating_sub(height)),
        width,
        height,
    }
}

/// Moves a box so the grabbed point stays under the pointer.
fn move_selection(
    selection: PixelSelection,
    target: (u32, u32),
    grab: Vec2,
    image: (u32, u32),
) -> PixelSelection {
    clamp_selection(
        PixelSelection {
            x: (target.0 as f32 - grab.x).max(0.0) as u32,
            y: (target.1 as f32 - grab.y).max(0.0) as u32,
            width: selection.width,
            height: selection.height,
        },
        image,
    )
}

/// Resizes a box between the original anchor and the pointer.
fn resize_selection(anchor: (u32, u32), target: (u32, u32), image: (u32, u32)) -> PixelSelection {
    clamp_selection(
        PixelSelection {
            x: anchor.0.min(target.0),
            y: anchor.1.min(target.1),
            width: anchor.0.abs_diff(target.0),
            height: anchor.1.abs_diff(target.1),
        },
        image,
    )
}

/// Rectangle the next drag edits in [`CaptureDraft`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBox {
    /// Becomes the template image.
    Template,
    /// Becomes the step's search region.
    Region,
}

#[derive(Debug, Clone, Copy)]
enum CaptureDrag {
    Move {
        which: CaptureBox,
        grab: Vec2,
    },
    Resize {
        which: CaptureBox,
        start: (u32, u32),
    },
}

pub enum CaptureAction {
    None,
    Cancel,
    Apply {
        template: PixelSelection,
        /// `None` keeps the whole client area as the search region.
        region: Option<PixelSelection>,
    },
}

/// One editor for both decisions: the blue box becomes the template image and
/// the green box becomes the step's search region. Both boxes can be moved by
/// dragging inside them and resized from the bottom-right handle; dragging on
/// empty space creates the box currently selected on the left.
pub struct CaptureDraft {
    pub image: RgbaImage,
    texture: egui::TextureHandle,
    name: String,
    template: PixelSelection,
    region: Option<PixelSelection>,
    active: CaptureBox,
    drag: Option<CaptureDrag>,
}

impl CaptureDraft {
    pub fn from_image(ctx: &egui::Context, image: RgbaImage, name: impl Into<String>) -> Self {
        let size = [image.width() as usize, image.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        let width = (image.width() / 4).max(16);
        let height = (image.height() / 4).max(16);
        let template = PixelSelection {
            x: image.width().saturating_sub(width) / 2,
            y: image.height().saturating_sub(height) / 2,
            width,
            height,
        };
        Self {
            texture: ctx.load_texture(
                "workflow-capture-draft",
                color_image,
                egui::TextureOptions::LINEAR,
            ),
            image,
            name: name.into(),
            template,
            region: None,
            active: CaptureBox::Template,
            drag: None,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn selection(&self, which: CaptureBox) -> Option<PixelSelection> {
        match which {
            CaptureBox::Template => Some(self.template),
            CaptureBox::Region => self.region,
        }
    }

    fn selection_mut(&mut self, which: CaptureBox) -> &mut PixelSelection {
        match which {
            CaptureBox::Template => &mut self.template,
            CaptureBox::Region => self.region.get_or_insert(self.template),
        }
    }

    fn handle_rect(&self, which: CaptureBox, display: egui::Rect) -> Option<egui::Rect> {
        let selection = self.selection(which)?;
        let rect = pixel_to_display(selection, display, &self.image);
        Some(egui::Rect::from_center_size(
            rect.right_bottom(),
            Vec2::splat(12.0),
        ))
    }

    pub fn show(&mut self, ctx: &egui::Context) -> CaptureAction {
        let mut action = CaptureAction::None;
        let mut open = true;
        egui::Window::new("框选截图区与识别区")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(880.0)
            .default_height(660.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(
                    "蓝色框是截图区（会成为模板图片），绿色框是识别区（步骤的搜索范围）。拖动框内部移动，拖动右下角小方块缩放；在空白处拖动会新建当前选中的框。",
                );
                ui.add_space(6.0);
                let available = ui.available_size();
                let max_size = Vec2::new(available.x.max(240.0), (available.y - 140.0).max(220.0));
                let scale = (max_size.x / self.image.width() as f32)
                    .min(max_size.y / self.image.height() as f32)
                    .min(1.0);
                let displayed = Vec2::new(
                    self.image.width() as f32 * scale,
                    self.image.height() as f32 * scale,
                );
                let response = ui.add(
                    egui::Image::new(&self.texture)
                        .fit_to_exact_size(displayed)
                        .sense(Sense::click_and_drag()),
                );
                let display = response.rect;


                let image_size = (self.image.width(), self.image.height());

                if response.drag_started()
                    && let Some(position) = response.interact_pointer_pos()
                {
                    let pixel = display_to_pixel(position, display, &self.image);
                    let on_handle = [CaptureBox::Template, CaptureBox::Region]
                        .into_iter()
                        .find(|which| {
                            self.handle_rect(*which, display)
                                .is_some_and(|rect| rect.contains(position))
                        });
                    let inside = |selection: PixelSelection| {
                        pixel.0 >= selection.x
                            && pixel.1 >= selection.y
                            && pixel.0 < selection.x + selection.width
                            && pixel.1 < selection.y + selection.height
                    };
                    self.drag = if let Some(which) = on_handle {
                        Some(CaptureDrag::Resize { which, start: pixel })
                    } else if inside(self.template) && self.active == CaptureBox::Template
                        || self.region.is_some_and(&inside) && self.active == CaptureBox::Region
                    {
                        let which = self.active;
                        let selection = self.selection(which).expect("active box exists");
                        let grab = Vec2::new(
                            (pixel.0 - selection.x) as f32,
                            (pixel.1 - selection.y) as f32,
                        );
                        Some(CaptureDrag::Move { which, grab })
                    } else if inside(self.template) {
                        let grab = Vec2::new(
                            (pixel.0 - self.template.x) as f32,
                            (pixel.1 - self.template.y) as f32,
                        );
                        Some(CaptureDrag::Move {
                            which: CaptureBox::Template,
                            grab,
                        })
                    } else if let Some(region) = self.region.filter(|region| inside(*region)) {
                        let grab =
                            Vec2::new((pixel.0 - region.x) as f32, (pixel.1 - region.y) as f32);
                        Some(CaptureDrag::Move {
                            which: CaptureBox::Region,
                            grab,
                        })
                    } else {
                        let which = self.active;
                        *self.selection_mut(which) = clamp_selection(
                            PixelSelection {
                                x: pixel.0,
                                y: pixel.1,
                                width: 8,
                                height: 8,
                            },
                            image_size,
                        );
                        Some(CaptureDrag::Resize { which, start: pixel })
                    };
                }
                if response.dragged()
                    && let (Some(drag), Some(position)) =
                        (self.drag, response.interact_pointer_pos())
                {
                    let pixel = display_to_pixel(position, display, &self.image);
                    match drag {
                        CaptureDrag::Move { which, grab } => {
                            let current = self.selection(which).expect("active box exists");
                            let moved = move_selection(current, pixel, grab, image_size);
                            *self.selection_mut(which) = moved;
                        }
                        CaptureDrag::Resize { which, start } => {
                            let resized = resize_selection(start, pixel, image_size);
                            *self.selection_mut(which) = resized;
                        }
                    }
                }
                if response.drag_stopped() {
                    self.drag = None;
                }

                if ui.is_rect_visible(display) {
                    for (which, color, label) in [
                        (CaptureBox::Template, theme::blue(), "截图区"),
                        (CaptureBox::Region, theme::green(), "识别区"),
                    ] {
                        let Some(selection) = self.selection(which) else {
                            continue;
                        };
                        let rect = pixel_to_display(selection, display, &self.image);
                        let highlight = self.active == which;
                        ui.painter().rect_stroke(
                            rect,
                            4.0,
                            Stroke::new(if highlight { 3.0 } else { 2.0 }, color),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            rect.min + Vec2::new(7.0, 5.0),
                            egui::Align2::LEFT_TOP,
                            format!("{label} {} × {}", selection.width, selection.height),
                            egui::FontId::proportional(12.0),
                            color,
                        );
                        if let Some(handle) = self.handle_rect(which, display) {
                            ui.painter().rect_filled(handle, 3.0, color);
                        }
                    }
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("当前编辑");
                    ui.selectable_value(&mut self.active, CaptureBox::Template, "截图区");
                    ui.selectable_value(&mut self.active, CaptureBox::Region, "识别区");
                    if ui
                        .button("识别区=全屏")
                        .on_hover_text("清除识别区，改为整个画面搜索")
                        .clicked()
                    {
                        self.region = None;
                    }
                    if ui
                        .button("识别区=截图区")
                        .on_hover_text("让识别区与截图区等大")
                        .clicked()
                    {
                        self.region = Some(self.template);
                    }
                    if ui.button("重置").clicked() {
                        *self = Self::from_image(ctx, self.image.clone(), self.name.clone());
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("模板名称");
                    ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(200.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(theme::primary_button("应用（截图+识别区）")).clicked() {
                            action = CaptureAction::Apply {
                                template: self.template,
                                region: self.region,
                            };
                        }
                        if ui.button("取消").clicked() {
                            action = CaptureAction::Cancel;
                        }
                        ui.label(
                            RichText::new("识别区留空表示全屏搜索")
                                .size(11.0)
                                .color(theme::tertiary_label()),
                        );
                    });
                });
            });
        if !open { CaptureAction::Cancel } else { action }
    }
}

impl RoiDraft {
    pub fn from_image(ctx: &egui::Context, image: RgbaImage) -> Self {
        let size = [image.width() as usize, image.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        Self {
            texture: ctx.load_texture(
                "workflow-roi-draft",
                color_image,
                egui::TextureOptions::LINEAR,
            ),
            image,
            selection: None,
            drag_start: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> RoiEditorAction {
        let mut action = RoiEditorAction::None;
        let mut open = true;
        egui::Window::new("框选此步骤的识别范围")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(820.0)
            .default_height(600.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("在游戏画面上拖动框选。严格区域可排除同屏相似图标；区域优先允许失败后回退全屏。");
                ui.add_space(6.0);
                let available = ui.available_size();
                let max_size = Vec2::new(available.x.max(240.0), (available.y - 100.0).max(220.0));
                let scale = (max_size.x / self.image.width() as f32)
                    .min(max_size.y / self.image.height() as f32)
                    .min(1.0);
                let response = ui.add(
                    egui::Image::new(&self.texture)
                        .fit_to_exact_size(Vec2::new(
                            self.image.width() as f32 * scale,
                            self.image.height() as f32 * scale,
                        ))
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
                    let rect = pixel_to_display(selection, response.rect, &self.image);
                    ui.painter().rect_stroke(
                        rect,
                        5.0,
                        Stroke::new(3.0, theme::orange()),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        rect.min + Vec2::new(7.0, 5.0),
                        egui::Align2::LEFT_TOP,
                        format!("{} × {}", selection.width, selection.height),
                        egui::FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }
                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(self.selection.is_some(), theme::primary_button("应用范围"))
                        .clicked()
                        && let Some(selection) = self.selection
                    {
                        action = RoiEditorAction::Apply(selection);
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if ui
                            .add(theme::secondary_button("全屏"))
                            .on_hover_text("不做区域限制：在整个画面里搜索（懒得框选时用这个）")
                            .clicked()
                        {
                            action = RoiEditorAction::FullFrame;
                        }
                        if ui
                            .add(theme::secondary_button("铺满画面"))
                            .on_hover_text("把当前画面整块设成识别范围，仍保存为区域")
                            .clicked()
                        {
                            self.selection = Some(PixelSelection {
                                x: 0,
                                y: 0,
                                width: self.image.width(),
                                height: self.image.height(),
                            });
                        }
                        if ui
                            .add_enabled(self.selection.is_some(), egui::Button::new("清除选择"))
                            .clicked()
                        {
                            self.selection = None;
                        }
                    });
                });
            });
        if !open {
            RoiEditorAction::Cancel
        } else {
            action
        }
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

    /// Seeds the draft with a ready-made selection (used by the combined
    /// screenshot + region editor).
    pub fn with_selection(mut self, selection: PixelSelection) -> Self {
        self.selection = Some(selection);
        self
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
                    .color(theme::secondary_label()),
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
                        Stroke::new(2.0, theme::blue()),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().rect_filled(
                        Rect::from_min_size(selection_rect.min, Vec2::new(118.0, 24.0)),
                        5.0,
                        theme::blue(),
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
mod capture_box_tests {
    use super::{PixelSelection, move_selection, resize_selection};

    #[test]
    fn moving_a_box_keeps_the_grabbed_point_under_the_pointer() {
        let selection = PixelSelection {
            x: 100,
            y: 60,
            width: 40,
            height: 20,
        };
        // Grabbed 10 px inside the box, dragged to (200, 100): the box follows.
        let moved = move_selection(
            selection,
            (200, 100),
            eframe::egui::vec2(10.0, 5.0),
            (1280, 720),
        );
        assert_eq!((moved.x, moved.y), (190, 95));
        assert_eq!((moved.width, moved.height), (40, 20));
    }

    #[test]
    fn a_box_never_leaves_the_frame_and_keeps_a_minimum_size() {
        let selection = PixelSelection {
            x: 1200,
            y: 700,
            width: 40,
            height: 20,
        };
        let moved = move_selection(
            selection,
            (5000, 5000),
            eframe::egui::vec2(0.0, 0.0),
            (1280, 720),
        );
        assert_eq!((moved.x, moved.y), (1240, 700));
        let resized = resize_selection((600, 400), (601, 401), (1280, 720));
        assert_eq!((resized.width, resized.height), (8, 8));
        assert_eq!((resized.x, resized.y), (600, 400));
    }

    #[test]
    fn resizing_works_in_every_drag_direction() {
        let up_left = resize_selection((600, 400), (500, 300), (1280, 720));
        assert_eq!(
            (up_left.x, up_left.y, up_left.width, up_left.height),
            (500, 300, 100, 100)
        );
        let down_right = resize_selection((500, 300), (600, 420), (1280, 720));
        assert_eq!(
            (
                down_right.x,
                down_right.y,
                down_right.width,
                down_right.height
            ),
            (500, 300, 100, 120)
        );
    }
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
