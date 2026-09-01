use chrono::Local;
use eframe::egui::{self, Align, Color32, CornerRadius, Layout, RichText, Sense, Stroke, Vec2};

use crate::mascot::Mascots;
use crate::model::{
    AppTab, BranchAction, BranchActionKind, BranchOutcome, ClickMethod, ConditionExpectation,
    ConditionMatchMode, ConditionOutcome, LogEntry, LogLevel, LoopMode, MacroProfile, RunnerStatus,
    SearchRegionSpec, StepKind, TemplateAsset, VisualConditionTerm, WorkflowBranch, WorkflowStep,
};
use crate::platform::{self, TargetWindow};
use crate::runner::{RunnerEvent, RunnerHandle};
use crate::storage;
use crate::template_editor::{EditorAction, PixelSelection, TemplateDraft, TemplateTestView};
use crate::theme;
use crate::vision::{self, SearchRegion};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturePurpose {
    NewTemplate,
    TestTemplate(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NewFlowPreset {
    AutoStarter,
    Blank,
}

impl NewFlowPreset {
    const ALL: [Self; 2] = [Self::AutoStarter, Self::Blank];

    fn label(self) -> &'static str {
        match self {
            Self::AutoStarter => "Auto 刷关起步流程",
            Self::Blank => "空白流程",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunOutcome {
    None,
    Completed,
    Failed,
}

/// On-demand thumbnail textures for template assets, plus the floating
/// preview window state. Loaded lazily from disk and cached by path.
#[derive(Default)]
struct TemplateThumbs {
    cache: std::collections::HashMap<String, egui::TextureHandle>,
    preview: Option<u64>,
}

impl TemplateThumbs {
    fn texture(
        &mut self,
        ctx: &egui::Context,
        path: &str,
        name: &str,
    ) -> Option<&egui::TextureHandle> {
        if !self.cache.contains_key(path) {
            let image = image::open(path).ok()?.into_rgba8();
            let size = [image.width() as usize, image.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
            let handle = ctx.load_texture(
                format!("thumb-{name}"),
                color_image,
                egui::TextureOptions::LINEAR,
            );
            self.cache.insert(path.to_owned(), handle);
        }
        self.cache.get(path)
    }
}

pub struct Make5771App {
    active_tab: AppTab,
    profile: MacroProfile,
    selected_step: Option<u64>,
    runner_status: RunnerStatus,
    completed_rounds: u32,
    current_step: String,
    logs: Vec<LogEntry>,
    toast: Option<String>,
    target_window: Option<TargetWindow>,
    template_draft: Option<TemplateDraft>,
    hotkey_receiver: Option<std::sync::mpsc::Receiver<platform::GlobalHotkey>>,
    _hotkey_guard: Option<platform::HotkeyGuard>,
    tray_receiver: Option<std::sync::mpsc::Receiver<platform::TrayEvent>>,
    tray_guard: Option<platform::TrayGuard>,
    run_started_at: Option<std::time::Instant>,
    last_round_at: Option<std::time::Instant>,
    round_durations: std::collections::VecDeque<u64>,
    last_run_outcome: RunOutcome,
    mascots: Mascots,
    thumbs: TemplateThumbs,
    countdown_capture_at: Option<std::time::Instant>,
    pending_capture: Option<image::RgbaImage>,
    capture_purpose: CapturePurpose,
    pending_test_capture: Option<(u64, image::RgbaImage)>,
    template_test_view: Option<TemplateTestView>,
    template_test_threshold: f32,
    workflow_runner: Option<RunnerHandle>,
    window_picker_open: bool,
    available_windows: Vec<TargetWindow>,
    window_filter: String,
    new_flow_open: bool,
    new_flow_name: String,
    new_flow_preset: NewFlowPreset,
    import_confirm_open: bool,
    pending_import_path: Option<std::path::PathBuf>,
    pending_delete_template: Option<u64>,
    current_profile_path: std::path::PathBuf,
    profiles_cache: Vec<std::path::PathBuf>,
    selected_profile: Option<std::path::PathBuf>,
    pending_open_profile: Option<std::path::PathBuf>,
    pending_delete_profile: Option<std::path::PathBuf>,
    save_as_open: bool,
    save_as_name: String,
}

impl Make5771App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut profile =
            storage::load_profile(&storage::default_profile_path()).unwrap_or_default();
        theme::install(&cc.egui_ctx, profile.dark_mode);
        cc.egui_ctx.set_zoom_factor(profile.ui_scale);
        let target_window = platform::find_target_window(&profile.target_window).ok();
        if profile.templates.is_empty()
            && let Some(target) = &target_window
            && target.client_width > 0
            && target.client_height > 0
        {
            profile.expected_client_width = target.client_width;
            profile.expected_client_height = target.client_height;
        }
        let (hotkey_receiver, hotkey_guard, hotkey_error) = match platform::install_global_hotkeys()
        {
            Ok((receiver, guard)) => (Some(receiver), Some(guard), None),
            Err(error) => (None, None, Some(error.to_string())),
        };
        let (tray_receiver, tray_guard, tray_error) = match platform::install_tray_icon() {
            Ok((receiver, guard)) => (Some(receiver), Some(guard), None),
            Err(error) => (None, None, Some(error.to_string())),
        };
        let mut app = Self {
            active_tab: AppTab::Run,
            profile,
            selected_step: Some(1),
            runner_status: RunnerStatus::Ready,
            completed_rounds: 0,
            current_step: "等待开始".to_owned(),
            logs: Vec::new(),
            toast: None,
            target_window,
            template_draft: None,
            hotkey_receiver,
            _hotkey_guard: hotkey_guard,
            tray_receiver,
            tray_guard,
            run_started_at: None,
            last_round_at: None,
            round_durations: std::collections::VecDeque::new(),
            last_run_outcome: RunOutcome::None,
            mascots: Mascots::new(&cc.egui_ctx),
            thumbs: TemplateThumbs::default(),
            countdown_capture_at: None,
            pending_capture: None,
            capture_purpose: CapturePurpose::NewTemplate,
            pending_test_capture: None,
            template_test_view: None,
            template_test_threshold: 0.90,
            workflow_runner: None,
            window_picker_open: false,
            available_windows: Vec::new(),
            window_filter: String::new(),
            new_flow_open: false,
            new_flow_name: "新流程".to_owned(),
            new_flow_preset: NewFlowPreset::AutoStarter,
            import_confirm_open: false,
            pending_import_path: None,
            pending_delete_template: None,
            current_profile_path: storage::default_profile_path(),
            profiles_cache: storage::list_profiles(),
            selected_profile: Some(storage::default_profile_path()),
            pending_open_profile: None,
            pending_delete_profile: None,
            save_as_open: false,
            save_as_name: String::new(),
        };
        if let Some(target) = &app.target_window {
            app.push_log(
                LogLevel::Success,
                format!("已自动连接窗口：{}", target.title),
            );
        } else {
            app.push_log(LogLevel::Info, "应用已启动，等待连接游戏窗口");
        }
        if cfg!(windows)
            && let Some(error) = hotkey_error
        {
            app.push_log(LogLevel::Warning, format!("全局快捷键不可用：{error}"));
        }
        if cfg!(windows)
            && let Some(error) = tray_error
        {
            app.push_log(LogLevel::Warning, format!("系统托盘不可用：{error}"));
        }
        app
    }

    fn connect_target_window(&mut self) {
        match platform::find_target_window(&self.profile.target_window) {
            Ok(target) => {
                if self.profile.templates.is_empty()
                    && target.client_width > 0
                    && target.client_height > 0
                {
                    self.profile.expected_client_width = target.client_width;
                    self.profile.expected_client_height = target.client_height;
                }
                let size_matches = target.client_width == self.profile.expected_client_width
                    && target.client_height == self.profile.expected_client_height;
                let message = if size_matches {
                    format!(
                        "已连接 {}（{} × {}）",
                        target.title, target.client_width, target.client_height
                    )
                } else {
                    format!(
                        "已连接，但客户区为 {} × {}；流程基准为 {} × {}，运行时将按比例缩放模板",
                        target.client_width,
                        target.client_height,
                        self.profile.expected_client_width,
                        self.profile.expected_client_height
                    )
                };
                self.push_log(
                    if size_matches {
                        LogLevel::Success
                    } else {
                        LogLevel::Warning
                    },
                    &message,
                );
                self.toast = Some(message);
                self.target_window = Some(target);
            }
            Err(error) => {
                let message = error.to_string();
                self.push_log(LogLevel::Warning, &message);
                self.toast = Some(message);
                self.target_window = None;
                self.open_window_picker();
            }
        }
    }

    fn open_window_picker(&mut self) {
        match platform::list_visible_windows() {
            Ok(windows) => {
                self.available_windows = windows;
                self.window_picker_open = true;
            }
            Err(error) => {
                self.toast = Some(error.to_string());
                self.push_log(LogLevel::Warning, error.to_string());
            }
        }
    }

    fn select_target_window(&mut self, target: TargetWindow) {
        if self.profile.templates.is_empty() {
            self.profile.expected_client_width = target.client_width;
            self.profile.expected_client_height = target.client_height;
        }
        self.profile.target_window = target.title.clone();
        let size_matches = target.client_width == self.profile.expected_client_width
            && target.client_height == self.profile.expected_client_height;
        self.target_window = Some(target.clone());
        self.window_picker_open = false;
        let message = if size_matches {
            format!(
                "已选择窗口：{}（{} × {}）",
                target.title, target.client_width, target.client_height
            )
        } else {
            format!(
                "窗口已选择，但尺寸为 {} × {}；现有模板基准为 {} × {}",
                target.client_width,
                target.client_height,
                self.profile.expected_client_width,
                self.profile.expected_client_height
            )
        };
        self.toast = Some(message.clone());
        self.push_log(
            if size_matches {
                LogLevel::Success
            } else {
                LogLevel::Warning
            },
            message,
        );
        if let Err(error) = storage::save_profile(&self.current_profile_path, &self.profile) {
            self.push_log(LogLevel::Warning, format!("窗口选择未能写入配置：{error}"));
        }
    }

    fn import_screenshot(&mut self, ctx: &egui::Context) {
        match platform::open_image_file_dialog() {
            Ok(Some(path)) => self.open_screenshot_path(ctx, &path),
            Ok(None) => {}
            Err(error) => {
                self.toast = Some(error.to_string());
                self.push_log(LogLevel::Warning, error.to_string());
            }
        }
    }

    fn begin_countdown_capture(&mut self, ctx: &egui::Context, purpose: CapturePurpose) {
        let Some(target) = self.target_window.as_ref() else {
            self.toast = Some("请先连接游戏窗口".to_owned());
            return;
        };
        if let Err(error) = platform::focus_target(target) {
            self.toast = Some(error.to_string());
            self.push_log(LogLevel::Warning, error.to_string());
            return;
        }
        self.countdown_capture_at = Some(
            std::time::Instant::now()
                .checked_add(std::time::Duration::from_secs(3))
                .unwrap_or_else(std::time::Instant::now),
        );
        self.capture_purpose = purpose;
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        self.push_log(LogLevel::Info, "3 秒后截取游戏画面");
    }

    fn capture_game_frame(&mut self, ctx: &egui::Context, source: &str) {
        let Some(target) = self.target_window.as_ref() else {
            self.toast = Some("请先连接游戏窗口".to_owned());
            return;
        };
        if !platform::is_foreground(target) {
            self.toast = Some("截图已取消：游戏不在前台".to_owned());
            self.push_log(LogLevel::Warning, "截图已取消：游戏不在前台");
        } else {
            match platform::capture_client(target) {
                Ok(image) => {
                    match self.capture_purpose {
                        CapturePurpose::NewTemplate => self.pending_capture = Some(image),
                        CapturePurpose::TestTemplate(template_id) => {
                            self.pending_test_capture = Some((template_id, image));
                        }
                    }
                    self.capture_purpose = CapturePurpose::NewTemplate;
                    self.push_log(LogLevel::Success, format!("已通过 {source} 截取游戏画面"));
                }
                Err(error) => {
                    self.toast = Some(error.to_string());
                    self.push_log(LogLevel::Warning, error.to_string());
                }
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.request_repaint();
    }

    fn process_background_events(&mut self, ctx: &egui::Context) {
        let events: Vec<_> = self
            .hotkey_receiver
            .as_ref()
            .map(|receiver| receiver.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                platform::GlobalHotkey::CaptureTemplate => {
                    self.capture_purpose = CapturePurpose::NewTemplate;
                    self.capture_game_frame(ctx, "F6");
                }
                platform::GlobalHotkey::Stop => {
                    if let Some(runner) = &self.workflow_runner {
                        runner.request_stop();
                        self.runner_status = RunnerStatus::Finishing;
                        self.current_step = "正在停止".to_owned();
                        self.push_log(LogLevel::Info, "已通过 F8 请求停止运行");
                    }
                }
            }
        }

        let tray_events: Vec<_> = self
            .tray_receiver
            .as_ref()
            .map(|receiver| receiver.try_iter().collect())
            .unwrap_or_default();
        for event in tray_events {
            match event {
                platform::TrayEvent::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                platform::TrayEvent::Exit => {
                    if let Some(runner) = &self.workflow_runner {
                        runner.request_stop();
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        let runner_events = self
            .workflow_runner
            .as_ref()
            .map(RunnerHandle::drain_events)
            .unwrap_or_default();
        let mut runner_finished = false;
        for event in runner_events {
            match event {
                RunnerEvent::Started => {
                    self.runner_status = RunnerStatus::Running;
                    self.run_started_at = Some(std::time::Instant::now());
                    self.last_round_at = self.run_started_at;
                    self.round_durations.clear();
                    self.last_run_outcome = RunOutcome::None;
                    self.push_log(LogLevel::Success, "流程已开始运行");
                }
                RunnerEvent::StepChanged(name) => {
                    self.current_step = name;
                    self.runner_status = RunnerStatus::Running;
                }
                RunnerEvent::MatchFound { name, score } => self.push_log(
                    LogLevel::Success,
                    format!("已识别并点击“{name}”，相似度 {score:.3}"),
                ),
                RunnerEvent::BranchMatched {
                    step,
                    branch,
                    score,
                } => self.push_log(
                    LogLevel::Success,
                    format!("“{step}”命中分支“{branch}”，相似度 {score:.3}"),
                ),
                RunnerEvent::ConditionMatched { step } => {
                    self.push_log(LogLevel::Success, format!("视觉条件“{step}”已稳定满足"))
                }
                RunnerEvent::TargetReconnected(title) => {
                    self.push_log(LogLevel::Success, format!("已重新连接游戏窗口：{title}"))
                }
                RunnerEvent::Notice(message) => self.push_log(LogLevel::Info, message),
                RunnerEvent::Paused(reason) => {
                    self.runner_status = RunnerStatus::Paused;
                    self.push_log(LogLevel::Warning, reason);
                }
                RunnerEvent::Resumed => {
                    self.runner_status = RunnerStatus::Running;
                    self.push_log(LogLevel::Info, "游戏回到前台，流程继续");
                }
                RunnerEvent::RoundCompleted(rounds) => {
                    self.completed_rounds = rounds;
                    let now = std::time::Instant::now();
                    if let Some(previous) = self.last_round_at.replace(now) {
                        self.round_durations.push_back(previous.elapsed().as_secs());
                        if self.round_durations.len() > 20 {
                            self.round_durations.pop_front();
                        }
                    }
                    self.push_log(LogLevel::Success, format!("已完成第 {rounds} 局"));
                }
                RunnerEvent::Stopped(reason) => {
                    self.runner_status = RunnerStatus::Ready;
                    self.current_step = "等待开始".to_owned();
                    self.run_started_at = None;
                    self.last_run_outcome = RunOutcome::Completed;
                    self.push_log(LogLevel::Info, format!("流程已停止：{reason}"));
                    self.notify_run_finished("流程已停止", &reason);
                    runner_finished = true;
                }
                RunnerEvent::Failed(error) => {
                    self.runner_status = RunnerStatus::Ready;
                    self.current_step = "运行失败".to_owned();
                    self.run_started_at = None;
                    self.last_run_outcome = RunOutcome::Failed;
                    self.toast = Some(error.clone());
                    self.push_log(LogLevel::Warning, format!("运行失败：{error}"));
                    self.notify_run_finished("运行失败", &error);
                    runner_finished = true;
                }
            }
        }
        if runner_finished {
            self.workflow_runner = None;
        }

        if self
            .countdown_capture_at
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            self.countdown_capture_at = None;
            self.capture_game_frame(ctx, "倒计时");
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }

    fn toggle_runner(&mut self) {
        if self.workflow_runner.is_some() {
            if let Some(runner) = &self.workflow_runner {
                runner.request_stop();
            }
            self.runner_status = RunnerStatus::Finishing;
            self.current_step = "正在停止".to_owned();
            self.push_log(LogLevel::Info, "正在停止流程");
            return;
        }

        let Some(target) = self.target_window.clone() else {
            self.toast = Some("请先连接游戏窗口".to_owned());
            return;
        };
        match RunnerHandle::start(self.profile.clone(), target.clone()) {
            Ok(runner) => {
                if let Err(error) = platform::focus_target(&target) {
                    runner.request_stop();
                    self.toast = Some(error.to_string());
                    self.push_log(LogLevel::Warning, error.to_string());
                    return;
                }
                self.completed_rounds = 0;
                self.current_step = "正在启动".to_owned();
                self.runner_status = RunnerStatus::Running;
                self.workflow_runner = Some(runner);
            }
            Err(error) => {
                self.toast = Some(error.clone());
                self.push_log(LogLevel::Warning, format!("无法开始：{error}"));
            }
        }
    }

    fn run_template_test(
        &mut self,
        ctx: &egui::Context,
        template_id: u64,
        frame: image::RgbaImage,
    ) {
        let Some(template_asset) = self
            .profile
            .templates
            .iter()
            .find(|template| template.id == template_id)
            .cloned()
        else {
            self.toast = Some("待测试的模板不存在".to_owned());
            return;
        };
        let mut template = match image::open(&template_asset.path) {
            Ok(image) => image.into_luma8(),
            Err(error) => {
                let message = format!("无法读取模板图片：{error}");
                self.toast = Some(message.clone());
                self.push_log(LogLevel::Warning, message);
                return;
            }
        };
        let frame_gray = image::imageops::grayscale(&frame);
        // Templates captured at another resolution are scaled to the frame,
        // matching what the runner does at run time.
        let reference_width = if template_asset.reference_width > 0 {
            template_asset.reference_width
        } else {
            frame.width()
        };
        let reference_height = if template_asset.reference_height > 0 {
            template_asset.reference_height
        } else {
            frame.height()
        };
        let mut scaled_region = template_asset.search_region;
        if reference_width != frame.width() || reference_height != frame.height() {
            let scale = |value: u32, from: u32, to: u32| {
                ((u64::from(value) * u64::from(to) + u64::from(from) / 2) / u64::from(from.max(1)))
                    .max(1) as u32
            };
            template = image::imageops::resize(
                &template,
                scale(template_asset.width, reference_width, frame.width()),
                scale(template_asset.height, reference_height, frame.height()),
                image::imageops::FilterType::Triangle,
            );
            scaled_region = scaled_region.map(|region| {
                region.scaled(
                    reference_width,
                    reference_height,
                    frame.width(),
                    frame.height(),
                )
            });
        }
        let search_region = scaled_region
            .map(|region| SearchRegion {
                x: region.x,
                y: region.y,
                width: region.width,
                height: region.height,
            })
            .unwrap_or_else(|| SearchRegion::full(&frame_gray));
        let report = vision::find_template_report(
            &frame_gray,
            &template,
            search_region,
            self.template_test_threshold,
        );
        let result = report.matched;
        let log_message = match result {
            Some(found) => format!(
                "模板“{}”匹配成功，相似度 {:.3}",
                template_asset.name, found.score
            ),
            None => format!(
                "模板“{}”未达到阈值 {:.2}（最佳相似度 {:.2}）",
                template_asset.name, self.template_test_threshold, report.best_score
            ),
        };
        self.push_log(
            if result.is_some() {
                LogLevel::Success
            } else {
                LogLevel::Warning
            },
            log_message,
        );
        self.template_test_view = Some(TemplateTestView::new(
            ctx,
            &frame,
            template_asset.name,
            search_region,
            result,
            self.template_test_threshold,
            self.mascots.ramona_pro.clone(),
        ));
    }

    fn open_screenshot_path(&mut self, ctx: &egui::Context, path: &std::path::Path) {
        match TemplateDraft::from_path(ctx, path) {
            Ok(draft) => {
                self.template_draft = Some(draft);
                self.push_log(LogLevel::Info, format!("已导入截图：{}", path.display()));
            }
            Err(error) => {
                self.toast = Some(error.clone());
                self.push_log(LogLevel::Warning, error);
            }
        }
    }

    fn save_template_from_draft(&mut self, name: String, selection: PixelSelection) {
        let Some(draft) = self.template_draft.take() else {
            return;
        };
        let reference_width = draft.image.width();
        let reference_height = draft.image.height();
        let id = self
            .profile
            .templates
            .iter()
            .map(|template| template.id)
            .max()
            .unwrap_or(0)
            + 1;
        let safe_name = safe_file_name(&name);
        let path = std::path::PathBuf::from("templates").join(format!("{safe_name}-{id}.png"));
        let result = (|| -> Result<(), String> {
            std::fs::create_dir_all("templates")
                .map_err(|error| format!("无法创建模板目录：{error}"))?;
            let cropped = image::imageops::crop_imm(
                &draft.image,
                selection.x,
                selection.y,
                selection.width,
                selection.height,
            )
            .to_image();
            cropped
                .save(&path)
                .map_err(|error| format!("无法保存模板图片：{error}"))?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                self.profile.templates.push(TemplateAsset {
                    id,
                    name: name.clone(),
                    path: path.to_string_lossy().into_owned(),
                    width: selection.width,
                    height: selection.height,
                    reference_width,
                    reference_height,
                    search_region: Some(suggest_search_region(
                        selection,
                        reference_width,
                        reference_height,
                    )),
                });
                if let Err(error) = storage::save_profile(&self.current_profile_path, &self.profile)
                {
                    self.push_log(
                        LogLevel::Warning,
                        format!("模板已保存，但流程更新失败：{error}"),
                    );
                }
                self.toast = Some(format!("模板“{name}”已保存"));
                self.push_log(LogLevel::Success, format!("已保存模板：{name}"));
            }
            Err(error) => {
                self.toast = Some(error.clone());
                self.push_log(LogLevel::Warning, error);
            }
        }
    }

    fn push_log(&mut self, level: LogLevel, message: impl Into<String>) {
        let entry = LogEntry {
            time: Local::now().format("%H:%M:%S").to_string(),
            level,
            message: message.into(),
        };
        let _ = storage::append_log(&entry);
        self.logs.push(entry);
    }

    /// Flashes the taskbar button and shows a tray balloon when a run ends.
    fn notify_run_finished(&self, title: &str, message: &str) {
        platform::flash_app_window();
        if let Some(guard) = &self.tray_guard {
            guard.show_notification(title, message);
        }
    }

    fn status_mascot(&self) -> &egui::TextureHandle {
        match self.last_run_outcome {
            RunOutcome::Completed => &self.mascots.luotan_easy,
            RunOutcome::Failed => &self.mascots.kekesi_cry,
            RunOutcome::None => match self.runner_status {
                RunnerStatus::Ready => &self.mascots.ogier_salute,
                RunnerStatus::Running => &self.mascots.wanda_work,
                RunnerStatus::Paused => &self.mascots.turu_sleep,
                RunnerStatus::Finishing => &self.mascots.agrippa_watch,
            },
        }
    }

    fn save_profile(&mut self) {
        let result = storage::save_profile(&self.current_profile_path, &self.profile);

        match result {
            Ok(()) => {
                self.toast = Some("流程已保存".to_owned());
                self.push_log(
                    LogLevel::Success,
                    format!("已保存流程“{}”", self.profile.name),
                );
            }
            Err(error) => {
                self.toast = Some(format!("保存失败：{error}"));
                self.push_log(LogLevel::Warning, format!("保存失败：{error}"));
            }
        }
    }

    fn refresh_profiles(&mut self) {
        self.profiles_cache = storage::list_profiles();
        if self.selected_profile.is_none() {
            self.selected_profile = Some(self.current_profile_path.clone());
        }
    }

    fn open_profile(&mut self, path: &std::path::Path) {
        if self.workflow_runner.is_some() {
            self.toast = Some("请先停止当前运行的流程".to_owned());
            return;
        }
        match storage::load_profile(path) {
            Ok(profile) => {
                self.profile = profile;
                self.current_profile_path = path.to_path_buf();
                self.selected_profile = Some(path.to_path_buf());
                self.selected_step = self.profile.steps.first().map(|step| step.id);
                self.target_window = platform::find_target_window(&self.profile.target_window).ok();
                self.template_draft = None;
                self.template_test_view = None;
                let message = format!("已打开流程文件：{}", storage::profile_display_name(path));
                self.toast = Some(message.clone());
                self.push_log(LogLevel::Success, message);
            }
            Err(error) => {
                let message = format!("打开流程失败：{error}");
                self.toast = Some(message.clone());
                self.push_log(LogLevel::Warning, message);
            }
        }
    }

    fn save_profile_as(&mut self) {
        let name = safe_file_name(&self.save_as_name);
        if name.is_empty() {
            self.toast = Some("流程文件名不能为空".to_owned());
            return;
        }
        let path = std::path::PathBuf::from(format!("profiles/{name}{}", storage::PROFILE_SUFFIX));
        match storage::save_profile(&path, &self.profile) {
            Ok(()) => {
                self.current_profile_path = path.clone();
                self.selected_profile = Some(path);
                self.refresh_profiles();
                let message = format!("已另存为流程文件：{name}");
                self.toast = Some(message.clone());
                self.push_log(LogLevel::Success, message);
            }
            Err(error) => {
                let message = format!("另存失败：{error}");
                self.toast = Some(message.clone());
                self.push_log(LogLevel::Warning, message);
            }
        }
    }

    fn delete_profile_file(&mut self, path: &std::path::Path) {
        if self.workflow_runner.is_some() {
            self.toast = Some("请先停止当前运行的流程".to_owned());
            return;
        }
        match std::fs::remove_file(path) {
            Ok(()) => {
                if self.current_profile_path == path {
                    self.current_profile_path = storage::default_profile_path();
                }
                self.selected_profile = None;
                self.refresh_profiles();
                let message = format!("已删除流程文件：{}", storage::profile_display_name(path));
                self.toast = Some(message.clone());
                self.push_log(LogLevel::Success, message);
            }
            Err(error) => {
                let message = format!("删除流程文件失败：{error}");
                self.toast = Some(message.clone());
                self.push_log(LogLevel::Warning, message);
            }
        }
    }

    fn request_new_flow(&mut self) {
        if self.workflow_runner.is_some() {
            self.toast = Some("请先停止当前运行的流程".to_owned());
            return;
        }
        self.new_flow_name = "新流程".to_owned();
        self.new_flow_preset = NewFlowPreset::AutoStarter;
        self.new_flow_open = true;
    }

    fn create_new_flow(&mut self) {
        let name = self.new_flow_name.trim();
        if name.is_empty() {
            self.toast = Some("流程名称不能为空".to_owned());
            return;
        }
        let mut profile = MacroProfile::default();
        profile.name = name.to_owned();
        profile.target_window = self.profile.target_window.clone();
        profile.expected_client_width = self.profile.expected_client_width;
        profile.expected_client_height = self.profile.expected_client_height;
        profile.sharing.author = self.profile.sharing.author.clone();
        profile.sharing.game_language = self.profile.sharing.game_language.clone();
        if self.new_flow_preset == NewFlowPreset::Blank {
            profile.steps = vec![WorkflowStep::new(1, "新步骤", StepKind::WaitAndClick, 0)];
        }
        match storage::save_profile(&self.current_profile_path, &profile) {
            Ok(()) => {
                let profile_name = profile.name.clone();
                self.profile = profile;
                self.selected_step = self.profile.steps.first().map(|step| step.id);
                self.active_tab = AppTab::Flow;
                self.new_flow_open = false;
                self.template_draft = None;
                self.template_test_view = None;
                self.toast = Some(format!("已新建流程“{profile_name}”"));
                self.push_log(LogLevel::Success, format!("已新建流程“{profile_name}”"));
            }
            Err(error) => {
                self.toast = Some(format!("新建流程失败：{error}"));
                self.push_log(LogLevel::Warning, format!("新建流程失败：{error}"));
            }
        }
    }

    fn request_import_flow(&mut self, path: Option<std::path::PathBuf>) {
        if self.workflow_runner.is_some() {
            self.toast = Some("请先停止当前运行的流程".to_owned());
            return;
        }
        self.pending_import_path = path;
        self.import_confirm_open = true;
    }

    fn import_flow_package(&mut self) {
        let path = match self.pending_import_path.take() {
            Some(path) => Some(path),
            None => match platform::open_workflow_package_dialog() {
                Ok(path) => path,
                Err(error) => {
                    self.toast = Some(error.to_string());
                    self.push_log(LogLevel::Warning, error.to_string());
                    return;
                }
            },
        };
        let Some(path) = path else {
            return;
        };
        match storage::import_workflow_package(&path) {
            Ok((profile, summary)) => {
                if let Err(error) = storage::save_profile(&self.current_profile_path, &profile) {
                    self.toast = Some(format!("导入后保存失败：{error}"));
                    self.push_log(LogLevel::Warning, format!("导入后保存失败：{error}"));
                    return;
                }
                self.profile = profile;
                self.target_window = platform::find_target_window(&self.profile.target_window).ok();
                self.selected_step = self.profile.steps.first().map(|step| step.id);
                self.active_tab = AppTab::Flow;
                self.template_draft = None;
                self.template_test_view = None;
                let message = format!(
                    "已导入流程“{}”{}，包含 {} 个图片模板",
                    summary.profile_name,
                    if summary.author.trim().is_empty() {
                        String::new()
                    } else {
                        format!("（作者：{}）", summary.author)
                    },
                    summary.template_count
                );
                self.toast = Some(message.clone());
                self.push_log(LogLevel::Success, message);
            }
            Err(error) => {
                self.toast = Some(error.to_string());
                self.push_log(LogLevel::Warning, error.to_string());
            }
        }
    }

    fn export_flow_package(&mut self) {
        let suggested_name: String = safe_file_name(&self.profile.name)
            .chars()
            .take(80)
            .collect();
        let path = match platform::save_workflow_package_dialog(&suggested_name) {
            Ok(path) => path,
            Err(error) => {
                self.toast = Some(error.to_string());
                self.push_log(LogLevel::Warning, error.to_string());
                return;
            }
        };
        let Some(path) = path else {
            return;
        };
        match storage::export_workflow_package(&path, &self.profile) {
            Ok(summary) => {
                let message = format!(
                    "已导出流程“{}”，打包 {} 个图片模板",
                    summary.profile_name, summary.template_count
                );
                self.toast = Some(message.clone());
                self.push_log(LogLevel::Success, message);
            }
            Err(error) => {
                self.toast = Some(error.to_string());
                self.push_log(LogLevel::Warning, error.to_string());
            }
        }
    }

    fn delete_template(&mut self, template_id: u64) {
        if self.workflow_runner.is_some() {
            self.toast = Some("请先停止当前运行的流程".to_owned());
            return;
        }
        let Some(template) = self
            .profile
            .templates
            .iter()
            .find(|template| template.id == template_id)
            .cloned()
        else {
            self.toast = Some("待删除的模板不存在".to_owned());
            return;
        };

        let mut updated = self.profile.clone();
        updated
            .templates
            .retain(|candidate| candidate.id != template_id);
        clear_template_references(&mut updated, &template.path);
        if let Err(error) = storage::save_profile(&self.current_profile_path, &updated) {
            self.toast = Some(format!("删除模板失败：{error}"));
            self.push_log(LogLevel::Warning, format!("删除模板失败：{error}"));
            return;
        }

        let source = std::path::PathBuf::from(&template.path);
        let recovery = if source.is_file() {
            let trash_dir = std::path::PathBuf::from("trash").join("templates");
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            let file_name = source
                .file_name()
                .and_then(|name| name.to_str())
                .map(safe_file_name)
                .unwrap_or_else(|| format!("template-{template_id}.png"));
            let destination = trash_dir.join(format!("{timestamp}-{file_name}"));
            match std::fs::create_dir_all(&trash_dir)
                .and_then(|()| std::fs::rename(&source, &destination))
            {
                Ok(()) => Some(destination),
                Err(error) => {
                    self.push_log(
                        LogLevel::Warning,
                        format!("模板已从流程移除，但图片未能移入回收目录：{error}"),
                    );
                    None
                }
            }
        } else {
            None
        };

        self.profile = updated;
        self.pending_delete_template = None;
        let references = count_template_references(&self.profile, &template.path);
        let message = if let Some(destination) = recovery {
            format!(
                "已删除模板“{}”并清理引用；图片可在 {} 恢复",
                template.name,
                destination.display()
            )
        } else {
            format!("已删除模板“{}”并清理引用", template.name)
        };
        debug_assert_eq!(references, 0);
        self.toast = Some(message.clone());
        self.push_log(LogLevel::Success, message);
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let drag_width = (ui.available_width() - 500.0)
                .max(240.0)
                .min(ui.available_width());
            let (drag_rect, drag) =
                ui.allocate_exact_size(Vec2::new(drag_width, 44.0), Sense::click_and_drag());
            ui.painter().image(
                self.mascots.keeper_hi.id(),
                egui::Rect::from_center_size(
                    drag_rect.left_center() + Vec2::new(19.0, 0.0),
                    Vec2::splat(34.0),
                ),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            ui.painter().text(
                drag_rect.left_top() + Vec2::new(42.0, 0.0),
                egui::Align2::LEFT_TOP,
                "Make 5771 Great Again",
                egui::FontId::proportional(22.0),
                theme::label(),
            );
            ui.painter().text(
                drag_rect.left_bottom() + Vec2::new(42.0, -1.0),
                egui::Align2::LEFT_BOTTOM,
                "Mythag University · Keeper's Terminal",
                egui::FontId::proportional(11.0),
                theme::tertiary_label(),
            );
            if drag.hovered() {
                ui.ctx().set_cursor_icon(if drag.dragged() {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Grab
                });
            }
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                let maximized = ui.input(|input| input.viewport().maximized.unwrap_or(false));
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if window_control_button(ui, WindowControl::Close).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                let maximized = ui.input(|input| input.viewport().maximized.unwrap_or(false));
                let maximize_control = if maximized {
                    WindowControl::Restore
                } else {
                    WindowControl::Maximize
                };
                if window_control_button(ui, maximize_control).clicked() {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                }
                if window_control_button(ui, WindowControl::Minimize).clicked() {
                    if self.tray_guard.is_some() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Visible(false));
                        self.push_log(LogLevel::Info, "已最小化到系统托盘，点击托盘图标恢复");
                    } else {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                }
                ui.add_space(8.0);
                if ui.button("设置").clicked() {
                    self.active_tab = AppTab::Settings;
                }
                if ui.button("选择窗口").clicked() {
                    self.open_window_picker();
                }
                ui.add_space(8.0);
                let (dot, label) = match self.target_window.as_ref() {
                    Some(target) if platform::is_foreground(target) => {
                        (theme::green(), "游戏在前台")
                    }
                    Some(_) => (theme::orange(), "等待游戏前台"),
                    None => (theme::orange(), "等待连接"),
                };
                ui.label(RichText::new(label).color(theme::secondary_label()));
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 4.5, dot);
            });
        });
    }

    fn run_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading(RichText::new("运行").size(28.0));
        ui.label(RichText::new("选择流程并设置本次运行方式").color(theme::secondary_label()));
        ui.add_space(10.0);

        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("当前流程")
                            .size(12.0)
                            .color(theme::secondary_label()),
                    );
                    ui.label(RichText::new(&self.profile.name).size(19.0).strong());
                    ui.label(
                        RichText::new(format!(
                            "{} · {} × {}",
                            self.profile.target_window,
                            self.profile.expected_client_width,
                            self.profile.expected_client_height
                        ))
                        .size(12.0)
                        .color(theme::tertiary_label()),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("编辑流程").clicked() {
                        self.active_tab = AppTab::Flow;
                    }
                    if ui.button("选择窗口").clicked() {
                        self.open_window_picker();
                    }
                    if ui
                        .button(if self.target_window.is_some() {
                            "重新连接"
                        } else {
                            "连接窗口"
                        })
                        .clicked()
                    {
                        self.connect_target_window();
                    }
                });
            });
        });

        ui.add_space(8.0);
        theme::card().show(ui, |ui| {
            ui.label(
                RichText::new("循环方式")
                    .size(12.0)
                    .color(theme::secondary_label()),
            );
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                for mode in LoopMode::ALL {
                    let selected = self.profile.loop_mode == mode;
                    let button =
                        egui::Button::new(RichText::new(mode.label()).color(if selected {
                            Color32::WHITE
                        } else {
                            theme::secondary_label()
                        }))
                        .fill(if selected {
                            theme::blue()
                        } else {
                            theme::surface_muted()
                        })
                        .stroke(if selected {
                            Stroke::NONE
                        } else {
                            Stroke::new(1.0, theme::separator())
                        })
                        .min_size(Vec2::new(115.0, 36.0));
                    if ui.add(button).clicked() {
                        self.profile.loop_mode = mode;
                    }
                }
            });
            ui.add_space(8.0);

            match self.profile.loop_mode {
                LoopMode::Count => {
                    ui.horizontal(|ui| {
                        ui.label("运行局数");
                        ui.add(
                            egui::DragValue::new(&mut self.profile.loop_count)
                                .range(1..=9999)
                                .suffix(" 局"),
                        );
                    });
                }
                LoopMode::Deadline => {
                    ui.horizontal(|ui| {
                        ui.label("停止时间");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.profile.deadline)
                                .desired_width(90.0)
                                .hint_text("23:30"),
                        );
                    });
                }
                LoopMode::Continuous => {
                    ui.label(
                        RichText::new("持续运行，直到按下 F8 或点击停止")
                            .color(theme::secondary_label()),
                    );
                }
            }

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("到达条件后完成当前对局");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.toggle_value(&mut self.profile.finish_current_round, "开启");
                });
            });
        });

        ui.add_space(8.0);
        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                let status_color = match self.runner_status {
                    RunnerStatus::Ready => theme::green(),
                    RunnerStatus::Running => theme::blue(),
                    RunnerStatus::Paused | RunnerStatus::Finishing => theme::orange(),
                };
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.0, status_color);
                ui.label(RichText::new(self.runner_status.label()).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add(
                        egui::Image::new(self.status_mascot()).fit_to_exact_size(Vec2::splat(64.0)),
                    );
                    ui.label(
                        RichText::new(format!("已完成 {} 局", self.completed_rounds))
                            .color(theme::secondary_label()),
                    );
                });
            });
            ui.label(
                RichText::new(format!("当前步骤：{}", self.current_step))
                    .size(12.0)
                    .color(theme::tertiary_label()),
            );

            let average_round = (!self.round_durations.is_empty()).then(|| {
                self.round_durations.iter().sum::<u64>() / self.round_durations.len() as u64
            });
            let mut stats = Vec::new();
            if let Some(average) = average_round {
                stats.push(format!("每局约 {}", format_duration_secs(average)));
                if self.profile.loop_mode == LoopMode::Count {
                    let remaining = self
                        .profile
                        .loop_count
                        .saturating_sub(self.completed_rounds);
                    if remaining > 0 && self.runner_status != RunnerStatus::Ready {
                        stats.push(format!(
                            "预计剩余 {}",
                            format_duration_secs(average * u64::from(remaining))
                        ));
                    }
                }
            }
            if let Some(started) = self.run_started_at {
                stats.push(format!(
                    "本次已运行 {}",
                    format_duration_secs(started.elapsed().as_secs())
                ));
            }
            if !stats.is_empty() {
                ui.label(
                    RichText::new(stats.join(" · "))
                        .size(12.0)
                        .color(theme::secondary_label()),
                );
            }
        });

        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            if self.runner_status == RunnerStatus::Ready
                && self.last_run_outcome == RunOutcome::None
            {
                ui.add(
                    egui::Image::new(&self.mascots.ramona_point)
                        .fit_to_exact_size(Vec2::splat(72.0)),
                );
                ui.add_space(4.0);
            } else if self.runner_status != RunnerStatus::Ready {
                ui.add(
                    egui::Image::new(&self.mascots.wincor_run).fit_to_exact_size(Vec2::splat(72.0)),
                );
                ui.add_space(4.0);
            }
            let running = self.runner_status != RunnerStatus::Ready;
            let text = if running {
                "停止运行"
            } else {
                "开始运行"
            };
            let button = egui::Button::new(
                RichText::new(text)
                    .size(18.0)
                    .color(Color32::WHITE)
                    .strong(),
            )
            .fill(if running {
                theme::orange()
            } else {
                theme::blue()
            })
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(16))
            .min_size(Vec2::new(260.0, 64.0));
            if ui.add(button).clicked() {
                self.toggle_runner();
            }
        });
    }

    fn flow_page(&mut self, ui: &mut egui::Ui) {
        let template_options: Vec<(u64, String, String)> = self
            .profile
            .templates
            .iter()
            .map(|template| (template.id, template.name.clone(), template.path.clone()))
            .collect();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading(RichText::new("流程编辑").size(28.0));
                ui.label(
                    RichText::new("编排线性步骤、画面分支和分支内动作")
                        .color(theme::secondary_label()),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(theme::primary_button("保存流程")).clicked() {
                    self.save_profile();
                }
                ui.add(
                    egui::Image::new(&self.mascots.dexter_cheers)
                        .fit_to_exact_size(Vec2::splat(44.0)),
                );
            });
        });
        ui.horizontal(|ui| {
            if ui.button("新建流程").clicked() {
                self.request_new_flow();
            }
            if ui.button("导入分享包").clicked() {
                self.request_import_flow(None);
            }
            if ui.button("导出分享包").clicked() {
                self.export_flow_package();
            }
            ui.label(
                RichText::new(".m5771pack 会同时包含流程与图片模板")
                    .size(11.0)
                    .color(theme::tertiary_label()),
            );
        });
        ui.horizontal(|ui| {
            ui.label("流程文件");
            let profiles_cache = self.profiles_cache.clone();
            let selected_label = self
                .selected_profile
                .as_ref()
                .map(|path| storage::profile_display_name(path))
                .unwrap_or_else(|| "未选择".to_owned());
            egui::ComboBox::from_id_salt("profile-file")
                .selected_text(selected_label)
                .show_ui(ui, |ui| {
                    for path in &profiles_cache {
                        ui.selectable_value(
                            &mut self.selected_profile,
                            Some(path.clone()),
                            storage::profile_display_name(path),
                        );
                    }
                });
            let has_selection = self.selected_profile.is_some();
            if ui
                .add_enabled(has_selection, egui::Button::new("打开"))
                .clicked()
            {
                self.pending_open_profile = self.selected_profile.clone();
            }
            if ui.button("另存为").clicked() {
                self.save_as_name = self.profile.name.clone();
                self.save_as_open = true;
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("删除"))
                .clicked()
            {
                self.pending_delete_profile = self.selected_profile.clone();
            }
            if ui.button("刷新").clicked() {
                self.refresh_profiles();
            }
        });
        ui.add_space(10.0);

        let list_width = (ui.available_width() * 0.43).clamp(320.0, 440.0);
        ui.columns(2, |columns| {
            columns[0].set_width(list_width);
            theme::card().show(&mut columns[0], |ui| {
                let selected_index = self
                    .selected_step
                    .and_then(|id| self.profile.steps.iter().position(|step| step.id == id));
                let mut list_command = None;
                ui.label(RichText::new(&self.profile.name).size(18.0).strong());
                ui.horizontal(|ui| {
                    if ui.button("添加步骤").clicked() {
                        list_command = Some(StepListCommand::Add);
                    }
                    if ui
                        .add_enabled(
                            selected_index.is_some_and(|index| index > 0),
                            egui::Button::new("上移"),
                        )
                        .clicked()
                    {
                        list_command = selected_index.map(StepListCommand::MoveUp);
                    }
                    if ui
                        .add_enabled(
                            selected_index
                                .is_some_and(|index| index + 1 < self.profile.steps.len()),
                            egui::Button::new("下移"),
                        )
                        .clicked()
                    {
                        list_command = selected_index.map(StepListCommand::MoveDown);
                    }
                    if ui
                        .add_enabled(
                            selected_index.is_some() && self.profile.steps.len() > 1,
                            egui::Button::new("删除"),
                        )
                        .clicked()
                    {
                        list_command = selected_index.map(StepListCommand::Delete);
                    }
                });
                match list_command {
                    Some(StepListCommand::Add) => {
                        let id = self
                            .profile
                            .steps
                            .iter()
                            .map(|step| step.id)
                            .max()
                            .unwrap_or(0)
                            + 1;
                        self.profile.steps.push(WorkflowStep::new(
                            id,
                            "新步骤",
                            StepKind::WaitAndClick,
                            0,
                        ));
                        self.selected_step = Some(id);
                    }
                    Some(StepListCommand::MoveUp(index)) => {
                        self.profile.steps.swap(index, index - 1);
                    }
                    Some(StepListCommand::MoveDown(index)) => {
                        self.profile.steps.swap(index, index + 1);
                    }
                    Some(StepListCommand::Delete(index)) => {
                        self.profile.steps.remove(index);
                        let next_index = index.min(self.profile.steps.len() - 1);
                        self.selected_step = Some(self.profile.steps[next_index].id);
                    }
                    None => {}
                }
                ui.separator();

                egui::ScrollArea::vertical()
                    .id_salt("workflow-step-list")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .max_height(460.0)
                    .show(ui, |ui| {
                        for (index, step) in self.profile.steps.iter().enumerate() {
                            let selected = self.selected_step == Some(step.id);
                            ui.horizontal(|ui| {
                                ui.add_space(step.indent as f32 * 22.0);
                                if step.indent > 0 {
                                    let (marker, _) =
                                        ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                                    ui.painter().circle_filled(
                                        marker.center(),
                                        3.0,
                                        theme::tertiary_label(),
                                    );
                                }
                                let label = format!("{}   {}", index + 1, step.name);
                                let button_width =
                                    (ui.available_width() - step.indent as f32 * 22.0 - 18.0)
                                        .max(150.0);
                                let button =
                                    egui::Button::new(RichText::new(label).color(if selected {
                                        theme::blue()
                                    } else {
                                        theme::label()
                                    }))
                                    .fill(if selected {
                                        theme::blue().gamma_multiply(0.10)
                                    } else {
                                        Color32::TRANSPARENT
                                    })
                                    .stroke(if selected {
                                        Stroke::new(1.0, theme::blue().gamma_multiply(0.35))
                                    } else {
                                        Stroke::NONE
                                    })
                                    .min_size(Vec2::new(button_width, 38.0));
                                if ui.add(button).clicked() {
                                    self.selected_step = Some(step.id);
                                }
                            });
                        }
                    });
            });

            theme::card().show(&mut columns[1], |ui| {
                ui.label(RichText::new("步骤属性").size(18.0).strong());
                ui.separator();
                let Some(selected_id) = self.selected_step else {
                    ui.label("请选择一个步骤");
                    return;
                };
                let Some(step) = self
                    .profile
                    .steps
                    .iter_mut()
                    .find(|step| step.id == selected_id)
                else {
                    ui.label("步骤不存在");
                    return;
                };

                ui.label(
                    RichText::new("名称")
                        .size(12.0)
                        .color(theme::secondary_label()),
                );
                ui.text_edit_singleline(&mut step.name);
                ui.add_space(6.0);
                ui.label(
                    RichText::new("类型")
                        .size(12.0)
                        .color(theme::secondary_label()),
                );
                egui::ComboBox::from_id_salt("step-kind")
                    .selected_text(step.kind.label())
                    .show_ui(ui, |ui| {
                        for (kind, label) in [
                            (StepKind::WaitAndClick, "等待并点击"),
                            (StepKind::WaitAny, "等待任一目标"),
                            (StepKind::VisualCondition, "视觉条件"),
                            (StepKind::Delay, "固定等待"),
                            (StepKind::RoundEnd, "本局结束"),
                        ] {
                            ui.selectable_value(&mut step.kind, kind, label);
                        }
                    });
                ui.add_space(6.0);
                match step.kind {
                    StepKind::WaitAndClick => {
                        template_picker(
                            ui,
                            ("step-template", step.id),
                            "图片模板",
                            &mut step.template,
                            &template_options,
                            &mut self.thumbs,
                        );
                        threshold_editor(ui, &mut step.threshold);
                        timeout_editor(ui, &mut step.timeout_secs);
                        delay_editor(ui, "识别后等待", &mut step.delay_ms);
                    }
                    StepKind::WaitAny => {
                        wait_any_editor(ui, step, &template_options, &mut self.thumbs)
                    }
                    StepKind::VisualCondition => {
                        visual_condition_editor(ui, step, &template_options, &mut self.thumbs)
                    }
                    StepKind::Delay => delay_editor(ui, "等待时间", &mut step.delay_ms),
                    StepKind::RoundEnd => {
                        ui.label(
                            RichText::new("将当前局计入已完成局数，然后开始下一轮。")
                                .size(11.0)
                                .color(theme::secondary_label()),
                        );
                    }
                    StepKind::Branch => {
                        ui.label(
                            RichText::new(
                                "通用 If/Else 条件节点尚未开放；请先使用可执行的“等待任一目标”。",
                            )
                            .size(11.0)
                            .color(theme::orange()),
                        );
                    }
                }
                if template_options.is_empty()
                    && matches!(
                        step.kind,
                        StepKind::WaitAndClick | StepKind::WaitAny | StepKind::VisualCondition
                    )
                {
                    ui.label(
                        RichText::new("请先在模板库中创建图片模板")
                            .size(11.0)
                            .color(theme::orange()),
                    );
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("启用此步骤");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.toggle_value(&mut step.enabled, "开启");
                    });
                });
            });
        });
    }

    fn templates_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading(RichText::new("模板库").size(28.0));
                ui.label(
                    RichText::new("管理用于视觉识别的画面片段").color(theme::secondary_label()),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(theme::primary_button("导入截图")).clicked() {
                    self.import_screenshot(ui.ctx());
                }
                if ui.button("3 秒截图").clicked() {
                    self.begin_countdown_capture(ui.ctx(), CapturePurpose::NewTemplate);
                }
            });
        });
        ui.add_space(12.0);
        let mut requested_test = None;
        let mut requested_delete = None;
        theme::card().show(ui, |ui| {
            ui.set_min_height(360.0);
            if self.profile.templates.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(70.0);
                    ui.add(
                        egui::Image::new(&self.mascots.agrippa_watch)
                            .fit_to_exact_size(Vec2::splat(110.0)),
                    );
                    ui.add_space(6.0);
                    ui.label(RichText::new("还没有图片模板").size(18.0).strong());
                    ui.label(
                        RichText::new("连接游戏窗口后，框选“开始”“Auto”和“结算”等目标")
                            .color(theme::secondary_label()),
                    );
                    ui.label(
                        RichText::new("也可以将 PNG/JPG 拖到窗口中")
                            .size(12.0)
                            .color(theme::tertiary_label()),
                    );
                });
            } else {
                ui.horizontal(|ui| {
                    ui.label("测试阈值");
                    ui.add(
                        egui::Slider::new(&mut self.template_test_threshold, 0.50..=1.00)
                            .fixed_decimals(2),
                    );
                    ui.label(
                        RichText::new("点击模板右侧的“测试”后自动切回游戏截图")
                            .size(11.0)
                            .color(theme::tertiary_label()),
                    );
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("template-list")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .max_height(430.0)
                    .show(ui, |ui| {
                        for template in self.profile.templates.clone() {
                            ui.horizontal(|ui| {
                                match self
                                    .thumbs
                                    .texture(ui.ctx(), &template.path, &template.name)
                                {
                                    Some(texture) => {
                                        let response = ui.add(
                                            egui::Image::new(texture)
                                                .fit_to_exact_size(Vec2::new(64.0, 40.0))
                                                .sense(Sense::click()),
                                        );
                                        if response.clicked() {
                                            self.thumbs.preview = Some(template.id);
                                        }
                                        response.on_hover_text("点击预览模板图片");
                                    }
                                    None => template_icon(ui, 30.0, theme::blue()),
                                }
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&template.name).strong());
                                    ui.label(
                                        RichText::new(format!(
                                            "{} × {} · {}",
                                            template.width, template.height, template.path
                                        ))
                                        .size(11.0)
                                        .color(theme::tertiary_label()),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.button("测试").clicked() {
                                        requested_test = Some(template.id);
                                    }
                                    if ui.button("删除").clicked() {
                                        requested_delete = Some(template.id);
                                    }
                                    let references =
                                        count_template_references(&self.profile, &template.path);
                                    if references > 0 {
                                        ui.label(
                                            RichText::new(format!("{references} 处引用"))
                                                .size(11.0)
                                                .color(theme::orange()),
                                        );
                                    }
                                    if let Some(region) = template.search_region {
                                        ui.label(
                                            RichText::new(format!(
                                                "搜索区 {} × {}",
                                                region.width, region.height
                                            ))
                                            .size(11.0)
                                            .color(theme::tertiary_label()),
                                        );
                                    }
                                });
                            });
                            ui.separator();
                        }
                    });
            }
        });
        if let Some(template_id) = requested_test {
            self.begin_countdown_capture(ui.ctx(), CapturePurpose::TestTemplate(template_id));
        }
        if let Some(template_id) = requested_delete {
            self.pending_delete_template = Some(template_id);
        }
    }

    fn logs_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading(RichText::new("运行日志").size(28.0));
                ui.label(RichText::new("查看识别、点击与异常记录").color(theme::secondary_label()));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("清空").clicked() {
                    self.logs.clear();
                }
                ui.add(
                    egui::Image::new(&self.mascots.hilo_vanish)
                        .fit_to_exact_size(Vec2::splat(34.0)),
                );
            });
        });
        ui.add_space(12.0);
        theme::card().show(ui, |ui| {
            ui.set_min_height(410.0);
            if self.logs.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(110.0);
                    ui.add(
                        egui::Image::new(&self.mascots.keeper_me)
                            .fit_to_exact_size(Vec2::splat(96.0)),
                    );
                    ui.add_space(6.0);
                    ui.label(RichText::new("还没有日志").size(18.0).strong());
                    ui.label(
                        RichText::new("开始运行或连接游戏窗口后，这里会显示识别与点击记录")
                            .color(theme::secondary_label()),
                    );
                });
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("log-list")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .max_height(430.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for entry in &self.logs {
                        let color = match entry.level {
                            LogLevel::Info => theme::blue(),
                            LogLevel::Success => theme::green(),
                            LogLevel::Warning => theme::orange(),
                        };
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&entry.time)
                                    .monospace()
                                    .color(theme::tertiary_label()),
                            );
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
                            ui.painter().circle_filled(rect.center(), 3.5, color);
                            ui.label(&entry.message);
                        });
                        ui.separator();
                    }
                });
        });
    }

    fn settings_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading(RichText::new("设置").size(28.0));
                ui.label(
                    RichText::new("管理流程信息、目标窗口与执行保护")
                        .color(theme::secondary_label()),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(theme::primary_button("保存设置")).clicked() {
                    self.save_profile();
                }
            });
        });
        ui.add_space(12.0);

        theme::card().show(ui, |ui| {
            ui.label(RichText::new("界面").size(18.0).strong());
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("深色模式");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .toggle_value(&mut self.profile.dark_mode, "开启")
                        .changed()
                    {
                        theme::apply(ui.ctx(), self.profile.dark_mode);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label("界面缩放");
                if ui
                    .add(
                        egui::Slider::new(&mut self.profile.ui_scale, 0.85..=1.50)
                            .fixed_decimals(2)
                            .custom_formatter(|value, _| format!("{:.0}%", value * 100.0)),
                    )
                    .changed()
                {
                    ui.ctx().set_zoom_factor(self.profile.ui_scale);
                }
            });
            ui.label(
                RichText::new("高分辨率或高缩放比的显示器可适当放大；切换后立即生效并随配置保存。")
                    .size(11.0)
                    .color(theme::tertiary_label()),
            );
        });

        ui.add_space(10.0);
        theme::card().show(ui, |ui| {
            ui.label(RichText::new("基本信息").size(18.0).strong());
            ui.separator();
            ui.label(
                RichText::new("流程名称")
                    .size(12.0)
                    .color(theme::secondary_label()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.profile.name)
                    .desired_width(ui.available_width()),
            );
        });

        ui.add_space(10.0);
        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("分享信息").size(18.0).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add(
                        egui::Image::new(&self.mascots.erika_ok)
                            .fit_to_exact_size(Vec2::splat(44.0)),
                    );
                });
            });
            ui.label(
                RichText::new("这些信息会随 .m5771pack 一起发布，方便别人判断是否适用。")
                    .size(11.0)
                    .color(theme::secondary_label()),
            );
            ui.separator();
            ui.columns(2, |columns| {
                columns[0].label(
                    RichText::new("作者")
                        .size(12.0)
                        .color(theme::secondary_label()),
                );
                columns[0].text_edit_singleline(&mut self.profile.sharing.author);
                columns[1].label(
                    RichText::new("游戏版本")
                        .size(12.0)
                        .color(theme::secondary_label()),
                );
                columns[1].text_edit_singleline(&mut self.profile.sharing.game_version);
            });
            ui.columns(2, |columns| {
                columns[0].label(
                    RichText::new("游戏语言")
                        .size(12.0)
                        .color(theme::secondary_label()),
                );
                columns[0].text_edit_singleline(&mut self.profile.sharing.game_language);
                columns[1].label(
                    RichText::new("标签（逗号分隔）")
                        .size(12.0)
                        .color(theme::secondary_label()),
                );
                columns[1].text_edit_singleline(&mut self.profile.sharing.tags);
            });
            ui.label(
                RichText::new("说明")
                    .size(12.0)
                    .color(theme::secondary_label()),
            );
            ui.add(
                egui::TextEdit::multiline(&mut self.profile.sharing.description)
                    .desired_rows(3)
                    .desired_width(ui.available_width())
                    .hint_text("说明用途、入口画面、特殊要求和已知限制"),
            );
        });

        ui.add_space(10.0);
        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("目标窗口").size(18.0).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (color, status) = match self.target_window.as_ref() {
                        Some(target) if platform::is_foreground(target) => {
                            (theme::green(), "已连接且在前台")
                        }
                        Some(_) => (theme::orange(), "已连接，等待前台"),
                        None => (theme::orange(), "未连接"),
                    };
                    ui.label(RichText::new(status).color(theme::secondary_label()));
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                    ui.painter().circle_filled(dot.center(), 4.5, color);
                });
            });
            ui.separator();
            ui.label(
                RichText::new("自动匹配的窗口标题")
                    .size(12.0)
                    .color(theme::secondary_label()),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.profile.target_window)
                    .hint_text("输入窗口标题的一部分")
                    .desired_width(ui.available_width()),
            );
            ui.horizontal(|ui| {
                if ui.button("按标题连接").clicked() {
                    self.connect_target_window();
                }
                if ui.button("从可见窗口选择").clicked() {
                    self.open_window_picker();
                }
            });

            ui.add_space(8.0);
            ui.label(
                RichText::new("客户区基准尺寸")
                    .size(12.0)
                    .color(theme::secondary_label()),
            );
            ui.horizontal(|ui| {
                ui.label("宽");
                ui.add(
                    egui::DragValue::new(&mut self.profile.expected_client_width)
                        .range(320..=7680)
                        .suffix(" px"),
                );
                ui.label("高");
                ui.add(
                    egui::DragValue::new(&mut self.profile.expected_client_height)
                        .range(240..=4320)
                        .suffix(" px"),
                );
                if ui
                    .add_enabled(
                        self.target_window.is_some(),
                        egui::Button::new("使用当前窗口尺寸"),
                    )
                    .clicked()
                    && let Some(target) = &self.target_window
                {
                    self.profile.expected_client_width = target.client_width;
                    self.profile.expected_client_height = target.client_height;
                }
            });
            if !self.profile.templates.is_empty() {
                ui.label(
                    RichText::new("改变基准尺寸后，旧模板的局部搜索区可能需要重新截取。")
                        .size(11.0)
                        .color(theme::orange()),
                );
            }
        });

        ui.add_space(10.0);
        theme::card().show(ui, |ui| {
            ui.label(RichText::new("点击方式").size(18.0).strong());
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("执行点击");
                egui::ComboBox::from_id_salt("click-method")
                    .selected_text(self.profile.click_method.label())
                    .show_ui(ui, |ui| {
                        for method in ClickMethod::ALL {
                            ui.selectable_value(
                                &mut self.profile.click_method,
                                method,
                                method.label(),
                            );
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("拟人化（点击位置 ±3 px、等待时间 ±20% 随机抖动）");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.toggle_value(&mut self.profile.click_jitter, "开启");
                });
            });
            if self.profile.click_method == ClickMethod::Background {
                ui.label(
                    RichText::new(
                        "后台点击不移动鼠标、不要求游戏在前台；如果游戏没有反应，请换回前台点击。",
                    )
                    .size(11.0)
                    .color(theme::orange()),
                );
            }
        });

        ui.add_space(10.0);
        ui.columns(2, |columns| {
            theme::card().show(&mut columns[0], |ui| {
                ui.label(RichText::new("快捷键").size(18.0).strong());
                ui.separator();
                settings_value_row(ui, "F6", "截取当前游戏画面");
                settings_value_row(ui, "F8", "立即请求停止流程");
                ui.label(
                    RichText::new("快捷键为全局注册；如果被其他软件占用，日志页会显示错误。")
                        .size(11.0)
                        .color(theme::tertiary_label()),
                );
            });
            theme::card().show(&mut columns[1], |ui| {
                ui.label(RichText::new("执行保护").size(18.0).strong());
                ui.separator();
                safety_status_row(ui, "只在目标窗口处于前台时点击");
                safety_status_row(ui, "分辨率不符时按比例缩放模板");
                safety_status_row(ui, "运行中客户区尺寸变化时停止");
                safety_status_row(ui, "仅使用截图与 Windows 标准输入");
            });
        });

        ui.add_space(10.0);
        theme::card().show(ui, |ui| {
            ui.label(RichText::new("本地数据").size(18.0).strong());
            ui.separator();
            settings_value_row(
                ui,
                "流程配置",
                &self.current_profile_path.display().to_string(),
            );
            settings_value_row(ui, "图片模板", "templates/");
            settings_value_row(ui, "运行日志", "logs/");
            settings_value_row(ui, "应用版本", env!("CARGO_PKG_VERSION"));
            ui.label(
                RichText::new("所有配置和模板均保存在程序当前工作目录，不会上传。")
                    .size(11.0)
                    .color(theme::tertiary_label()),
            );
            ui.horizontal(|ui| {
                if ui.button("新建流程").clicked() {
                    self.request_new_flow();
                }
                if ui.button("导入分享包").clicked() {
                    self.request_import_flow(None);
                }
                if ui.button("导出分享包").clicked() {
                    self.export_flow_package();
                }
            });
        });
        ui.add_space(12.0);
    }

    fn show_window_picker(&mut self, ctx: &egui::Context) {
        if !self.window_picker_open {
            return;
        }
        let mut open = self.window_picker_open;
        let mut chosen = None;
        let mut refresh = false;
        egui::Window::new("选择目标窗口")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .default_height(460.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("选择软件需要识别和操作的游戏窗口")
                            .color(theme::secondary_label()),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            egui::Image::new(&self.mascots.celeste_pray)
                                .fit_to_exact_size(Vec2::splat(44.0)),
                        );
                    });
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.window_filter)
                            .hint_text("搜索窗口标题")
                            .desired_width(390.0),
                    );
                    if ui.button("刷新列表").clicked() {
                        refresh = true;
                    }
                });
                ui.separator();

                let filter = self.window_filter.trim().to_lowercase();
                egui::ScrollArea::vertical()
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut shown = 0;
                        for window in &self.available_windows {
                            if window.title == "Make 5771 Great Again"
                                || (!filter.is_empty()
                                    && !window.title.to_lowercase().contains(&filter))
                            {
                                continue;
                            }
                            shown += 1;
                            let selected = self
                                .target_window
                                .as_ref()
                                .is_some_and(|target| target.handle == window.handle);
                            let response = ui.add(
                                egui::Button::new(
                                    RichText::new(format!(
                                        "{}\n{} x {}",
                                        window.title, window.client_width, window.client_height
                                    ))
                                    .color(if selected {
                                        theme::blue()
                                    } else {
                                        theme::label()
                                    }),
                                )
                                .fill(if selected {
                                    theme::blue().gamma_multiply(0.10)
                                } else {
                                    theme::surface_muted()
                                })
                                .stroke(Stroke::new(
                                    1.0,
                                    if selected {
                                        theme::blue().gamma_multiply(0.45)
                                    } else {
                                        theme::separator()
                                    },
                                ))
                                .min_size(Vec2::new(ui.available_width(), 54.0)),
                            );
                            if response.clicked() {
                                chosen = Some(window.clone());
                            }
                            ui.add_space(4.0);
                        }
                        if shown == 0 {
                            ui.vertical_centered(|ui| {
                                ui.add_space(40.0);
                                ui.add(
                                    egui::Image::new(&self.mascots.brown_dunno)
                                        .fit_to_exact_size(Vec2::splat(80.0)),
                                );
                                ui.add_space(4.0);
                                ui.label("没有符合条件的可见窗口");
                            });
                        }
                    });
            });

        self.window_picker_open = open;
        if refresh {
            self.open_window_picker();
        }
        if let Some(target) = chosen {
            self.select_target_window(target);
        }
    }

    fn show_workflow_dialogs(&mut self, ctx: &egui::Context) {
        if self.new_flow_open {
            let mut open = self.new_flow_open;
            let mut create = false;
            let mut cancel = false;
            egui::Window::new("新建流程")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(460.0)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new("新建后会替换当前工作区。如需保留当前流程，请先导出分享包。")
                            .color(theme::orange()),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("流程名称")
                            .size(12.0)
                            .color(theme::secondary_label()),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_flow_name)
                            .desired_width(ui.available_width()),
                    );
                    ui.label(
                        RichText::new("起始结构")
                            .size(12.0)
                            .color(theme::secondary_label()),
                    );
                    for preset in NewFlowPreset::ALL {
                        ui.radio_value(&mut self.new_flow_preset, preset, preset.label());
                    }
                    ui.label(
                        RichText::new(match self.new_flow_preset {
                            NewFlowPreset::AutoStarter => {
                                "包含开始游戏、开启 Auto、结算和本局结束四个步骤。"
                            }
                            NewFlowPreset::Blank => "只创建一个待配置步骤，适合从零编排。",
                        })
                        .size(11.0)
                        .color(theme::tertiary_label()),
                    );
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.add(theme::primary_button("创建流程")).clicked() {
                            create = true;
                        }
                        if ui.button("取消").clicked() {
                            cancel = true;
                        }
                    });
                });
            if cancel {
                open = false;
            }
            self.new_flow_open = open;
            if create {
                self.create_new_flow();
            }
        }

        if self.import_confirm_open {
            let mut open = self.import_confirm_open;
            let mut proceed = false;
            let mut cancel = false;
            egui::Window::new("导入流程分享包")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(480.0)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new("导入会替换当前工作区的流程，但不会删除现有模板文件。")
                            .color(theme::orange()),
                    );
                    if let Some(path) = &self.pending_import_path {
                        ui.label(
                            RichText::new(format!("待导入：{}", path.display()))
                                .size(11.0)
                                .color(theme::secondary_label()),
                        );
                    } else {
                        ui.label(
                            RichText::new("继续后将打开 .m5771pack 文件选择器。")
                                .size(11.0)
                                .color(theme::secondary_label()),
                        );
                    }
                    ui.label(
                        RichText::new(
                            "包内图片会写入独立的 imports 目录；分享包不能携带可执行脚本。",
                        )
                        .size(11.0)
                        .color(theme::tertiary_label()),
                    );
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.add(theme::primary_button("继续导入")).clicked() {
                            proceed = true;
                        }
                        if ui.button("取消").clicked() {
                            cancel = true;
                        }
                    });
                });
            if cancel {
                self.pending_import_path = None;
                open = false;
            }
            if proceed {
                open = false;
            }
            self.import_confirm_open = open;
            if proceed {
                self.import_flow_package();
            }
        }

        if let Some(template_id) = self.pending_delete_template {
            let template = self
                .profile
                .templates
                .iter()
                .find(|template| template.id == template_id)
                .cloned();
            if let Some(template) = template {
                let references = count_template_references(&self.profile, &template.path);
                let mut open = true;
                let mut confirm = false;
                let mut cancel = false;
                egui::Window::new("删除图片模板")
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(false)
                    .default_width(480.0)
                    .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Image::new(&self.mascots.kekesi_grudge)
                                    .fit_to_exact_size(Vec2::splat(64.0)),
                            );
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&template.name).size(18.0).strong());
                                ui.label(
                                    RichText::new(format!("文件：{}", template.path))
                                        .size(11.0)
                                        .color(theme::secondary_label()),
                                );
                            });
                        });
                        if references > 0 {
                            ui.label(
                                RichText::new(format!(
                                    "该模板被 {references} 个步骤或分支引用，删除后这些引用会被清空。"
                                ))
                                .color(theme::orange()),
                            );
                        }
                        ui.label(
                            RichText::new(
                                "图片文件会移入 trash/templates 回收目录，不会立即永久删除。",
                            )
                            .size(11.0)
                            .color(theme::tertiary_label()),
                        );
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.add(theme::primary_button("确认删除")).clicked() {
                                confirm = true;
                            }
                            if ui.button("取消").clicked() {
                                cancel = true;
                            }
                        });
                    });
                if cancel || !open {
                    self.pending_delete_template = None;
                }
                if confirm {
                    self.delete_template(template_id);
                }
            } else {
                self.pending_delete_template = None;
            }
        }

        if let Some(path) = self.pending_open_profile.clone() {
            let mut open = true;
            let mut confirm = false;
            let mut cancel = false;
            egui::Window::new("打开流程文件")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(460.0)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "打开“{}”将替换当前工作区，未保存的修改会丢失。",
                        storage::profile_display_name(&path)
                    ));
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.add(theme::primary_button("打开")).clicked() {
                            confirm = true;
                        }
                        if ui.button("取消").clicked() {
                            cancel = true;
                        }
                    });
                });
            if cancel || !open {
                self.pending_open_profile = None;
            }
            if confirm {
                self.open_profile(&path);
                self.pending_open_profile = None;
            }
        }

        if self.save_as_open {
            let mut open = self.save_as_open;
            let mut confirm = false;
            let mut cancel = false;
            egui::Window::new("流程另存为")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(460.0)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new("文件名")
                            .size(12.0)
                            .color(theme::secondary_label()),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.save_as_name)
                            .desired_width(ui.available_width()),
                    );
                    ui.label(
                        RichText::new(format!(
                            "保存到 profiles/<文件名>{}",
                            storage::PROFILE_SUFFIX
                        ))
                        .size(11.0)
                        .color(theme::tertiary_label()),
                    );
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.add(theme::primary_button("保存")).clicked() {
                            confirm = true;
                        }
                        if ui.button("取消").clicked() {
                            cancel = true;
                        }
                    });
                });
            if cancel {
                open = false;
            }
            self.save_as_open = open;
            if confirm {
                self.save_profile_as();
                self.save_as_open = false;
            }
        }

        if let Some(path) = self.pending_delete_profile.clone() {
            let mut open = true;
            let mut confirm = false;
            let mut cancel = false;
            egui::Window::new("删除流程文件")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(460.0)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "将从磁盘删除“{}”，当前工作区内容不受影响。",
                        storage::profile_display_name(&path)
                    ));
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.add(theme::primary_button("确认删除")).clicked() {
                            confirm = true;
                        }
                        if ui.button("取消").clicked() {
                            cancel = true;
                        }
                    });
                });
            if cancel || !open {
                self.pending_delete_profile = None;
            }
            if confirm {
                self.delete_profile_file(&path);
                self.pending_delete_profile = None;
            }
        }

        if let Some(template_id) = self.thumbs.preview {
            let template = self
                .profile
                .templates
                .iter()
                .find(|template| template.id == template_id)
                .cloned();
            match template {
                Some(template) => {
                    let mut open = true;
                    egui::Window::new("模板预览")
                        .open(&mut open)
                        .collapsible(false)
                        .resizable(true)
                        .default_width(420.0)
                        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                        .show(ctx, |ui| {
                            ui.label(RichText::new(&template.name).strong());
                            ui.label(
                                RichText::new(format!(
                                    "{} × {} · {}",
                                    template.width, template.height, template.path
                                ))
                                .size(11.0)
                                .color(theme::tertiary_label()),
                            );
                            ui.add_space(6.0);
                            if let Some(texture) =
                                self.thumbs
                                    .texture(ui.ctx(), &template.path, &template.name)
                            {
                                let size = texture.size_vec2();
                                let scale = (400.0 / size.x).min(280.0 / size.y).min(2.0);
                                ui.add(
                                    egui::Image::new(texture)
                                        .fit_to_exact_size(size * scale.max(0.1)),
                                );
                            } else {
                                ui.label(RichText::new("模板图片读取失败").color(theme::orange()));
                            }
                        });
                    if !open {
                        self.thumbs.preview = None;
                    }
                }
                None => self.thumbs.preview = None,
            }
        }
    }

    fn bottom_navigation(&mut self, ui: &mut egui::Ui) {
        // Spread the tabs across the panel width so they adapt when the window
        // is resized, but cap the width so very wide windows stay usable.
        let tab_width = (ui.available_width() / AppTab::ALL.len() as f32).clamp(96.0, 220.0);
        ui.horizontal_centered(|ui| {
            for tab in AppTab::ALL {
                let selected = self.active_tab == tab;
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(tab_width, 52.0), Sense::click());
                let color = if selected {
                    theme::blue()
                } else {
                    theme::secondary_label()
                };
                if selected {
                    ui.painter().rect_filled(
                        rect.shrink2(Vec2::new(4.0, 3.0)),
                        12.0,
                        theme::blue().gamma_multiply(0.12),
                    );
                } else if response.hovered() {
                    ui.painter().rect_filled(
                        rect.shrink2(Vec2::new(4.0, 3.0)),
                        12.0,
                        theme::surface_muted(),
                    );
                }
                nav_icon(
                    ui.painter(),
                    tab,
                    rect.center_top() + Vec2::new(0.0, 16.0),
                    color,
                );
                ui.painter().text(
                    rect.center_bottom() - Vec2::new(0.0, 9.0),
                    egui::Align2::CENTER_CENTER,
                    tab.label(),
                    egui::FontId::proportional(11.0),
                    color,
                );
                if response.clicked() {
                    self.active_tab = tab;
                }
            }
        });
    }
}

/// Simple painter-drawn glyphs for the bottom tab bar (no icon font needed).
fn nav_icon(painter: &egui::Painter, tab: AppTab, center: egui::Pos2, color: Color32) {
    match tab {
        AppTab::Run => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + Vec2::new(-4.0, -6.5),
                    center + Vec2::new(-4.0, 6.5),
                    center + Vec2::new(7.0, 0.0),
                ],
                color,
                Stroke::NONE,
            ));
        }
        AppTab::Flow => {
            for (index, width) in [10.0_f32, 14.0, 7.0].iter().enumerate() {
                let y = center.y + (index as f32 - 1.0) * 6.0;
                painter.circle_filled(egui::Pos2::new(center.x - 7.0, y), 1.8, color);
                painter.line_segment(
                    [
                        egui::Pos2::new(center.x - 3.0, y),
                        egui::Pos2::new(center.x - 3.0 + width, y),
                    ],
                    Stroke::new(1.8, color),
                );
            }
        }
        AppTab::Templates => {
            let rect = egui::Rect::from_center_size(center, Vec2::splat(15.0));
            painter.rect_stroke(rect, 3.0, Stroke::new(1.6, color), egui::StrokeKind::Inside);
            painter.circle_filled(center + Vec2::new(-3.0, -3.0), 1.6, color);
            for (from, to) in [
                (Vec2::new(-5.5, 5.5), Vec2::new(-0.5, 0.5)),
                (Vec2::new(-0.5, 0.5), Vec2::new(2.5, 3.5)),
                (Vec2::new(2.5, 3.5), Vec2::new(5.5, 0.5)),
            ] {
                painter.line_segment([center + from, center + to], Stroke::new(1.6, color));
            }
        }
        AppTab::Logs => {
            let rect = egui::Rect::from_center_size(center, Vec2::new(13.0, 16.0));
            painter.rect_stroke(rect, 2.5, Stroke::new(1.6, color), egui::StrokeKind::Inside);
            for index in 0..3 {
                let y = center.y + (index as f32 - 1.0) * 4.5;
                painter.line_segment(
                    [
                        egui::Pos2::new(center.x - 3.5, y),
                        egui::Pos2::new(center.x + 3.5, y),
                    ],
                    Stroke::new(1.4, color),
                );
            }
        }
        AppTab::Settings => {
            painter.circle_stroke(center, 4.5, Stroke::new(1.8, color));
            for index in 0..8 {
                let direction = Vec2::angled(index as f32 * std::f32::consts::TAU / 8.0);
                painter.line_segment(
                    [center + direction * 6.5, center + direction * 8.5],
                    Stroke::new(1.6, color),
                );
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum StepListCommand {
    Add,
    MoveUp(usize),
    MoveDown(usize),
    Delete(usize),
}

#[derive(Debug, Clone, Copy)]
enum BranchListCommand {
    MoveUp(usize),
    MoveDown(usize),
    Delete(usize),
}

#[derive(Debug, Clone, Copy)]
enum ActionListCommand {
    MoveUp(usize),
    MoveDown(usize),
    Delete(usize),
}

fn wait_any_editor(
    ui: &mut egui::Ui,
    step: &mut WorkflowStep,
    template_options: &[(u64, String, String)],
    thumbs: &mut TemplateThumbs,
) {
    timeout_editor(ui, &mut step.timeout_secs);
    ui.label(
        RichText::new("每次只执行第一个匹配的分支，从上到下代表优先级。")
            .size(11.0)
            .color(theme::secondary_label()),
    );
    ui.add_space(4.0);

    let mut command = None;
    let branch_count = step.branches.len();
    egui::ScrollArea::vertical()
        .id_salt(("wait-any-branches", step.id))
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .max_height(390.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for index in 0..branch_count {
                let title = format!("{}. {}", index + 1, step.branches[index].name);
                egui::CollapsingHeader::new(title)
                    .id_salt(("workflow-branch", step.id, step.branches[index].id))
                    .default_open(index == 0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(index > 0, egui::Button::new("上移"))
                                .clicked()
                            {
                                command = Some(BranchListCommand::MoveUp(index));
                            }
                            if ui
                                .add_enabled(index + 1 < branch_count, egui::Button::new("下移"))
                                .clicked()
                            {
                                command = Some(BranchListCommand::MoveDown(index));
                            }
                            if ui.button("删除分支").clicked() {
                                command = Some(BranchListCommand::Delete(index));
                            }
                        });
                        edit_workflow_branch(
                            ui,
                            step.id,
                            &mut step.branches[index],
                            template_options,
                            thumbs,
                        );
                    });
                ui.separator();
            }
        });

    match command {
        Some(BranchListCommand::MoveUp(index)) => step.branches.swap(index, index - 1),
        Some(BranchListCommand::MoveDown(index)) => step.branches.swap(index, index + 1),
        Some(BranchListCommand::Delete(index)) => {
            step.branches.remove(index);
        }
        None => {}
    }

    if ui.button("添加分支").clicked() {
        let id = step
            .branches
            .iter()
            .map(|branch| branch.id)
            .max()
            .unwrap_or(0)
            + 1;
        step.branches.push(WorkflowBranch::new(id, "新分支"));
    }
}

fn visual_condition_editor(
    ui: &mut egui::Ui,
    step: &mut WorkflowStep,
    template_options: &[(u64, String, String)],
    thumbs: &mut TemplateThumbs,
) {
    timeout_editor(ui, &mut step.timeout_secs);
    ui.horizontal(|ui| {
        ui.label("匹配方式");
        egui::ComboBox::from_id_salt(("visual-condition-mode", step.id))
            .selected_text(step.visual_condition.mode.label())
            .show_ui(ui, |ui| {
                for mode in ConditionMatchMode::ALL {
                    ui.selectable_value(&mut step.visual_condition.mode, mode, mode.label());
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("稳定确认次数");
        ui.add(
            egui::DragValue::new(&mut step.visual_condition.stable_checks)
                .range(1..=10)
                .suffix(" 次"),
        );
    });
    ui.horizontal(|ui| {
        ui.label("条件满足后");
        egui::ComboBox::from_id_salt(("visual-condition-outcome", step.id))
            .selected_text(step.visual_condition.outcome.label())
            .show_ui(ui, |ui| {
                for outcome in ConditionOutcome::ALL {
                    ui.selectable_value(
                        &mut step.visual_condition.outcome,
                        outcome,
                        outcome.label(),
                    );
                }
            });
    });
    if step.visual_condition.outcome == ConditionOutcome::ClickTemplate {
        delay_editor(ui, "点击后等待", &mut step.delay_ms);
    }
    ui.label(
        RichText::new(
            "连续达到确认次数后才执行结果动作；“点击指定模板”会点击第一条命中的“出现”条件。",
        )
        .size(11.0)
        .color(theme::secondary_label()),
    );
    ui.add_space(4.0);

    let mut delete_index = None;
    let term_count = step.visual_condition.terms.len();
    egui::ScrollArea::vertical()
        .id_salt(("visual-condition-terms", step.id))
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .max_height(300.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, term) in step.visual_condition.terms.iter_mut().enumerate() {
                theme::subtle_card().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}. {}", index + 1, term.name)).strong());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add_enabled(term_count > 1, egui::Button::new("删除"))
                                .clicked()
                            {
                                delete_index = Some(index);
                            }
                        });
                    });
                    ui.text_edit_singleline(&mut term.name);
                    template_picker(
                        ui,
                        ("visual-condition-template", step.id, term.id),
                        "检查画面",
                        &mut term.template,
                        template_options,
                        thumbs,
                    );
                    ui.horizontal(|ui| {
                        ui.label("期望");
                        egui::ComboBox::from_id_salt((
                            "visual-condition-expectation",
                            step.id,
                            term.id,
                        ))
                        .selected_text(term.expectation.label())
                        .show_ui(ui, |ui| {
                            for expectation in ConditionExpectation::ALL {
                                ui.selectable_value(
                                    &mut term.expectation,
                                    expectation,
                                    expectation.label(),
                                );
                            }
                        });
                    });
                    threshold_editor(ui, &mut term.threshold);
                });
                ui.add_space(4.0);
            }
        });
    if let Some(index) = delete_index {
        step.visual_condition.terms.remove(index);
    }
    if ui.button("添加视觉条件").clicked() {
        let id = step
            .visual_condition
            .terms
            .iter()
            .map(|term| term.id)
            .max()
            .unwrap_or(0)
            + 1;
        step.visual_condition.terms.push(VisualConditionTerm::new(
            id,
            "新条件",
            ConditionExpectation::Present,
        ));
    }
}

fn edit_workflow_branch(
    ui: &mut egui::Ui,
    step_id: u64,
    branch: &mut WorkflowBranch,
    template_options: &[(u64, String, String)],
    thumbs: &mut TemplateThumbs,
) {
    ui.label(
        RichText::new("分支名称")
            .size(11.0)
            .color(theme::secondary_label()),
    );
    ui.text_edit_singleline(&mut branch.name);
    template_picker(
        ui,
        ("branch-trigger", step_id, branch.id),
        "触发画面",
        &mut branch.trigger_template,
        template_options,
        thumbs,
    );
    threshold_editor(ui, &mut branch.threshold);
    ui.horizontal(|ui| {
        ui.label("命中后点击触发目标");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.toggle_value(&mut branch.click_trigger, "开启");
        });
    });
    if branch.click_trigger {
        delay_editor(ui, "点击后等待", &mut branch.trigger_delay_ms);
    }
    ui.horizontal(|ui| {
        ui.label("分支结束后");
        egui::ComboBox::from_id_salt(("branch-outcome", step_id, branch.id))
            .selected_text(branch.outcome.label())
            .show_ui(ui, |ui| {
                for outcome in BranchOutcome::ALL {
                    ui.selectable_value(&mut branch.outcome, outcome, outcome.label());
                }
            });
    });

    ui.add_space(6.0);
    ui.label(RichText::new("分支动作").strong());
    if branch.actions.is_empty() {
        ui.label(
            RichText::new("没有后续动作；命中后直接执行分支结果。")
                .size(11.0)
                .color(theme::tertiary_label()),
        );
    }
    let mut action_command = None;
    let action_count = branch.actions.len();
    for (index, action) in branch.actions.iter_mut().enumerate() {
        theme::subtle_card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}. {}", index + 1, action.name)).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("删除").clicked() {
                        action_command = Some(ActionListCommand::Delete(index));
                    }
                    if ui
                        .add_enabled(index + 1 < action_count, egui::Button::new("下移"))
                        .clicked()
                    {
                        action_command = Some(ActionListCommand::MoveDown(index));
                    }
                    if ui
                        .add_enabled(index > 0, egui::Button::new("上移"))
                        .clicked()
                    {
                        action_command = Some(ActionListCommand::MoveUp(index));
                    }
                });
            });
            ui.text_edit_singleline(&mut action.name);
            egui::ComboBox::from_id_salt(("branch-action-kind", step_id, branch.id, action.id))
                .selected_text(action.kind.label())
                .show_ui(ui, |ui| {
                    for kind in BranchActionKind::ALL {
                        ui.selectable_value(&mut action.kind, kind, kind.label());
                    }
                });
            match action.kind {
                BranchActionKind::WaitAndClick => {
                    template_picker(
                        ui,
                        ("branch-action-template", step_id, branch.id, action.id),
                        "目标画面",
                        &mut action.template,
                        template_options,
                        thumbs,
                    );
                    threshold_editor(ui, &mut action.threshold);
                    timeout_editor(ui, &mut action.timeout_secs);
                    delay_editor(ui, "点击后等待", &mut action.delay_ms);
                }
                BranchActionKind::Delay => {
                    delay_editor(ui, "等待时间", &mut action.delay_ms);
                }
            }
        });
        ui.add_space(4.0);
    }
    match action_command {
        Some(ActionListCommand::MoveUp(index)) => branch.actions.swap(index, index - 1),
        Some(ActionListCommand::MoveDown(index)) => branch.actions.swap(index, index + 1),
        Some(ActionListCommand::Delete(index)) => {
            branch.actions.remove(index);
        }
        None => {}
    }
    if ui.button("添加分支动作").clicked() {
        let id = branch
            .actions
            .iter()
            .map(|action| action.id)
            .max()
            .unwrap_or(0)
            + 1;
        branch.actions.push(BranchAction::new(
            id,
            "新动作",
            BranchActionKind::WaitAndClick,
        ));
    }
}

fn template_picker(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    selected: &mut Option<String>,
    template_options: &[(u64, String, String)],
    thumbs: &mut TemplateThumbs,
) {
    ui.label(
        RichText::new(label)
            .size(11.0)
            .color(theme::secondary_label()),
    );
    ui.horizontal(|ui| {
        let selected_entry = selected.as_ref().and_then(|path| {
            template_options
                .iter()
                .find(|(_, _, candidate)| candidate == path)
        });
        let selected_name = selected_entry
            .map(|(_, name, _)| name.as_str())
            .unwrap_or("尚未选择");
        egui::ComboBox::from_id_salt(id)
            .selected_text(selected_name)
            .show_ui(ui, |ui| {
                ui.selectable_value(selected, None, "尚未选择");
                for (_, name, path) in template_options {
                    ui.selectable_value(selected, Some(path.clone()), name);
                }
            });
        if let Some(&(template_id, ref name, ref path)) = selected_entry
            && let Some(texture) = thumbs.texture(ui.ctx(), path, name)
        {
            let response = ui.add(
                egui::Image::new(texture)
                    .fit_to_exact_size(Vec2::new(64.0, 28.0))
                    .sense(Sense::click()),
            );
            if response.clicked() {
                thumbs.preview = Some(template_id);
            }
            response.on_hover_text("点击预览模板图片");
        }
    });
}

fn threshold_editor(ui: &mut egui::Ui, threshold: &mut f32) {
    ui.label(
        RichText::new("识别相似度")
            .size(11.0)
            .color(theme::secondary_label()),
    );
    ui.add(egui::Slider::new(threshold, 0.50..=1.00).fixed_decimals(2));
}

fn timeout_editor(ui: &mut egui::Ui, timeout_secs: &mut u32) {
    ui.horizontal(|ui| {
        ui.label("超时");
        ui.add(
            egui::DragValue::new(timeout_secs)
                .range(1..=3600)
                .suffix(" 秒"),
        );
    });
}

fn delay_editor(ui: &mut egui::Ui, label: &str, delay_ms: &mut u32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(delay_ms)
                .range(0..=60000)
                .suffix(" ms"),
        );
    });
}

fn format_duration_secs(secs: u64) -> String {
    if secs >= 3600 {
        format!("{} 小时 {} 分", secs / 3600, secs % 3600 / 60)
    } else if secs >= 60 {
        format!("{} 分 {} 秒", secs / 60, secs % 60)
    } else {
        format!("{secs} 秒")
    }
}

fn settings_value_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(theme::secondary_label()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).strong());
        });
    });
}

fn safety_status_row(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
        ui.painter()
            .circle_filled(dot.center(), 4.0, theme::green());
        ui.label(label);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new("始终开启")
                    .size(11.0)
                    .color(theme::secondary_label()),
            );
        });
    });
}

fn count_template_references(profile: &MacroProfile, path: &str) -> usize {
    let mut count = 0;
    for step in &profile.steps {
        count += usize::from(step.template.as_deref() == Some(path));
        for branch in &step.branches {
            count += usize::from(branch.trigger_template.as_deref() == Some(path));
            for action in &branch.actions {
                count += usize::from(action.template.as_deref() == Some(path));
            }
        }
    }
    count
}

fn clear_template_references(profile: &mut MacroProfile, path: &str) {
    for step in &mut profile.steps {
        clear_matching_path(&mut step.template, path);
        for branch in &mut step.branches {
            clear_matching_path(&mut branch.trigger_template, path);
            for action in &mut branch.actions {
                clear_matching_path(&mut action.template, path);
            }
        }
    }
}

fn clear_matching_path(value: &mut Option<String>, path: &str) {
    if value.as_deref() == Some(path) {
        *value = None;
    }
}

fn safe_file_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .filter(|character| {
            !matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
        })
        .collect();
    let sanitized = sanitized.trim().trim_end_matches('.');
    if sanitized.is_empty() {
        "template".to_owned()
    } else {
        sanitized.to_owned()
    }
}

fn suggest_search_region(
    selection: PixelSelection,
    image_width: u32,
    image_height: u32,
) -> SearchRegionSpec {
    let horizontal_margin = selection.width.max(80);
    let vertical_margin = selection.height.max(60);
    let x = selection.x.saturating_sub(horizontal_margin);
    let y = selection.y.saturating_sub(vertical_margin);
    let right = selection
        .x
        .saturating_add(selection.width)
        .saturating_add(horizontal_margin)
        .min(image_width);
    let bottom = selection
        .y
        .saturating_add(selection.height)
        .saturating_add(vertical_margin)
        .min(image_height);
    SearchRegionSpec {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
}

#[derive(Clone, Copy)]
enum WindowControl {
    Minimize,
    Maximize,
    Restore,
    Close,
}

fn window_control_button(ui: &mut egui::Ui, control: WindowControl) -> egui::Response {
    let response = ui.add(
        egui::Button::new("")
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(34.0, 30.0)),
    );

    let is_close = matches!(control, WindowControl::Close);
    if response.hovered() {
        let hover_color = if is_close {
            Color32::from_rgb(255, 59, 48)
        } else {
            theme::surface_muted()
        };
        ui.painter().rect_filled(response.rect, 9.0, hover_color);
    }
    let color = if is_close && response.hovered() {
        Color32::WHITE
    } else {
        theme::secondary_label()
    };
    let center = response.rect.center();
    let stroke = Stroke::new(1.5, color);
    match control {
        WindowControl::Minimize => {
            ui.painter().line_segment(
                [center + Vec2::new(-5.0, 3.0), center + Vec2::new(5.0, 3.0)],
                stroke,
            );
        }
        WindowControl::Maximize => {
            ui.painter().rect_stroke(
                egui::Rect::from_center_size(center, Vec2::new(10.0, 9.0)),
                1.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        WindowControl::Restore => {
            let back =
                egui::Rect::from_min_size(center + Vec2::new(-3.0, -5.0), Vec2::new(9.0, 8.0));
            let front =
                egui::Rect::from_min_size(center + Vec2::new(-6.0, -2.0), Vec2::new(9.0, 8.0));
            ui.painter()
                .rect_stroke(back, 1.0, stroke, egui::StrokeKind::Inside);
            ui.painter().rect_filled(front, 1.0, theme::background());
            ui.painter()
                .rect_stroke(front, 1.0, stroke, egui::StrokeKind::Inside);
        }
        WindowControl::Close => {
            ui.painter().line_segment(
                [center + Vec2::new(-4.5, -4.5), center + Vec2::new(4.5, 4.5)],
                stroke,
            );
            ui.painter().line_segment(
                [center + Vec2::new(-4.5, 4.5), center + Vec2::new(4.5, -4.5)],
                stroke,
            );
        }
    }
    response
}

fn template_icon(ui: &mut egui::Ui, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let frame = rect.shrink(size * 0.14);
    let stroke = Stroke::new((size / 24.0).clamp(1.2, 2.0), color);
    ui.painter()
        .rect_stroke(frame, size * 0.12, stroke, egui::StrokeKind::Inside);
    ui.painter().circle_filled(
        frame.min + Vec2::splat(frame.width() * 0.28),
        size * 0.07,
        color,
    );
    let base = frame.bottom() - frame.height() * 0.18;
    ui.painter().add(egui::Shape::line(
        vec![
            egui::pos2(frame.left() + frame.width() * 0.14, base),
            egui::pos2(frame.left() + frame.width() * 0.42, frame.center().y),
            egui::pos2(
                frame.left() + frame.width() * 0.58,
                base - frame.height() * 0.10,
            ),
            egui::pos2(frame.right() - frame.width() * 0.12, base),
        ],
        stroke,
    ));
}

fn window_resize_borders(ui: &mut egui::Ui) {
    if ui.input(|input| input.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let bounds = ui.max_rect();
    let edge = 5.0;
    let corner = 10.0;
    let resize_areas = [
        (
            "resize-north",
            egui::ResizeDirection::North,
            egui::CursorIcon::ResizeVertical,
            egui::Rect::from_min_max(
                bounds.min + Vec2::new(corner, 0.0),
                egui::pos2(bounds.right() - corner, bounds.top() + edge),
            ),
        ),
        (
            "resize-south",
            egui::ResizeDirection::South,
            egui::CursorIcon::ResizeVertical,
            egui::Rect::from_min_max(
                egui::pos2(bounds.left() + corner, bounds.bottom() - edge),
                bounds.max - Vec2::new(corner, 0.0),
            ),
        ),
        (
            "resize-west",
            egui::ResizeDirection::West,
            egui::CursorIcon::ResizeHorizontal,
            egui::Rect::from_min_max(
                bounds.min + Vec2::new(0.0, corner),
                egui::pos2(bounds.left() + edge, bounds.bottom() - corner),
            ),
        ),
        (
            "resize-east",
            egui::ResizeDirection::East,
            egui::CursorIcon::ResizeHorizontal,
            egui::Rect::from_min_max(
                egui::pos2(bounds.right() - edge, bounds.top() + corner),
                bounds.max - Vec2::new(0.0, corner),
            ),
        ),
        (
            "resize-north-west",
            egui::ResizeDirection::NorthWest,
            egui::CursorIcon::ResizeNwSe,
            egui::Rect::from_min_size(bounds.min, Vec2::splat(corner)),
        ),
        (
            "resize-north-east",
            egui::ResizeDirection::NorthEast,
            egui::CursorIcon::ResizeNeSw,
            egui::Rect::from_min_size(
                egui::pos2(bounds.right() - corner, bounds.top()),
                Vec2::splat(corner),
            ),
        ),
        (
            "resize-south-west",
            egui::ResizeDirection::SouthWest,
            egui::CursorIcon::ResizeNeSw,
            egui::Rect::from_min_size(
                egui::pos2(bounds.left(), bounds.bottom() - corner),
                Vec2::splat(corner),
            ),
        ),
        (
            "resize-south-east",
            egui::ResizeDirection::SouthEast,
            egui::CursorIcon::ResizeNwSe,
            egui::Rect::from_min_size(bounds.max - Vec2::splat(corner), Vec2::splat(corner)),
        ),
    ];

    for (id, direction, cursor, rect) in resize_areas {
        let response = ui.interact(rect, egui::Id::new(id), Sense::drag());
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(cursor);
        }
        if response.drag_started() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
        }
    }
}

impl eframe::App for Make5771App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_background_events(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if let Some(image) = self.pending_capture.take() {
            self.active_tab = AppTab::Templates;
            self.template_draft =
                Some(TemplateDraft::from_image(&ctx, image, "游戏截图", "新模板"));
        }
        if let Some((template_id, frame)) = self.pending_test_capture.take() {
            self.active_tab = AppTab::Templates;
            self.run_template_test(&ctx, template_id, frame);
        }

        let dropped_paths: Vec<_> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect()
        });
        if self.template_draft.is_none()
            && let Some(path) = dropped_paths.first()
        {
            let is_package = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("m5771pack"));
            if is_package {
                self.request_import_flow(Some(path.clone()));
            } else {
                self.active_tab = AppTab::Templates;
                self.open_screenshot_path(&ctx, path);
            }
        }

        egui::Panel::top("top-bar")
            .frame(
                egui::Frame::new()
                    .fill(theme::background())
                    .inner_margin(egui::Margin::symmetric(24, 16)),
            )
            .show(ui, |ui| self.top_bar(ui));

        egui::Panel::bottom("navigation")
            .frame(
                egui::Frame::new()
                    .fill(theme::surface())
                    .stroke(Stroke::new(1.0, theme::separator()))
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(ui, |ui| self.bottom_navigation(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::background())
                    .inner_margin(egui::Margin::symmetric(24, 12)),
            )
            .show(ui, |ui| {
                let active_tab = self.active_tab;
                egui::ScrollArea::vertical()
                    .id_salt(("main-page", active_tab.label()))
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        match active_tab {
                            AppTab::Run => self.run_page(ui),
                            AppTab::Flow => self.flow_page(ui),
                            AppTab::Templates => self.templates_page(ui),
                            AppTab::Logs => self.logs_page(ui),
                            AppTab::Settings => self.settings_page(ui),
                        }
                    });
            });

        self.show_window_picker(&ctx);
        self.show_workflow_dialogs(&ctx);

        if let Some(message) = self.toast.clone() {
            let mut dismiss = false;
            egui::Window::new("提示")
                .anchor(egui::Align2::CENTER_TOP, [0.0, 78.0])
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(message);
                        if ui.small_button("关闭").clicked() {
                            dismiss = true;
                        }
                    });
                });
            if dismiss {
                self.toast = None;
            }
        }

        let editor_action = self
            .template_draft
            .as_mut()
            .map(|draft| draft.show(&ctx))
            .unwrap_or(EditorAction::None);
        match editor_action {
            EditorAction::None => {}
            EditorAction::Cancel => self.template_draft = None,
            EditorAction::Save { name, selection } => {
                self.save_template_from_draft(name, selection);
            }
        }

        if self
            .template_test_view
            .as_mut()
            .is_some_and(|view| !view.show(&ctx))
        {
            self.template_test_view = None;
        }

        window_resize_borders(ui);
    }
}
