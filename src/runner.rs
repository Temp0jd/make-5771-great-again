use std::cell::Cell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, NaiveTime};
use image::GrayImage;

use crate::model::{
    BranchAction, BranchActionKind, BranchOutcome, ClickMethod, ConditionExpectation,
    ConditionMatchMode, ConditionOutcome, LoopMode, MacroProfile, StepKind, TemplateAsset,
    VisualConditionSpec, VisualConditionTerm, WorkflowBranch, WorkflowStep,
};
use crate::platform::{self, TargetWindow};
use crate::vision::{self, SearchRegion};

#[derive(Debug, Clone)]
pub enum RunnerEvent {
    Started,
    StepChanged(String),
    MatchFound {
        name: String,
        score: f32,
    },
    BranchMatched {
        step: String,
        branch: String,
        score: f32,
    },
    ConditionMatched {
        step: String,
    },
    TargetReconnected(String),
    Paused(String),
    Resumed,
    RoundCompleted(u32),
    Stopped(String),
    Failed(String),
}

pub struct RunnerHandle {
    stop: Arc<AtomicBool>,
    events: mpsc::Receiver<RunnerEvent>,
}

impl RunnerHandle {
    pub fn start(profile: MacroProfile, target: TargetWindow) -> Result<Self, String> {
        validate_executable_profile(&profile)?;
        let (sender, events) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        std::thread::Builder::new()
            .name("m5771-runner".to_owned())
            .spawn(move || run_workflow(profile, target, thread_stop, sender))
            .map_err(|error| format!("无法启动执行线程：{error}"))?;
        Ok(Self { stop, events })
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    pub fn drain_events(&self) -> Vec<RunnerEvent> {
        self.events.try_iter().collect()
    }
}

fn validate_executable_profile(profile: &MacroProfile) -> Result<(), String> {
    profile.validate().map_err(|issues| issues.join("；"))?;
    if !profile
        .steps
        .iter()
        .any(|step| step.enabled && step.kind == StepKind::RoundEnd)
    {
        return Err("流程缺少“本局结束”步骤".to_owned());
    }
    if profile
        .steps
        .iter()
        .rfind(|step| step.enabled)
        .map(|step| step.kind)
        != Some(StepKind::RoundEnd)
    {
        return Err("“本局结束”必须是最后一个启用的步骤".to_owned());
    }
    for step in profile.steps.iter().filter(|step| step.enabled) {
        match step.kind {
            StepKind::WaitAndClick => {
                validate_template_reference(
                    profile,
                    step.template.as_ref(),
                    &format!("步骤“{}”", step.name),
                )?;
            }
            StepKind::Delay | StepKind::RoundEnd => {}
            StepKind::WaitAny => {
                if step.branches.is_empty() {
                    return Err(format!("步骤“{}”至少需要一条分支", step.name));
                }
                for branch in &step.branches {
                    validate_template_reference(
                        profile,
                        branch.trigger_template.as_ref(),
                        &format!("分支“{}”", branch.name),
                    )?;
                    for action in &branch.actions {
                        if action.kind == BranchActionKind::WaitAndClick {
                            validate_template_reference(
                                profile,
                                action.template.as_ref(),
                                &format!("动作“{}”", action.name),
                            )?;
                        }
                    }
                }
            }
            StepKind::Branch => {
                return Err(format!("步骤“{}”使用了尚未开放执行的分支类型", step.name));
            }
            StepKind::VisualCondition => {
                if step.visual_condition.terms.is_empty() {
                    return Err(format!("步骤“{}”至少需要一条视觉条件", step.name));
                }
                for term in &step.visual_condition.terms {
                    validate_template_reference(
                        profile,
                        term.template.as_ref(),
                        &format!("视觉条件“{}”", term.name),
                    )?;
                }
            }
        }
    }
    if profile.loop_mode == LoopMode::Deadline {
        parse_deadline(&profile.deadline)?;
    }
    Ok(())
}

fn validate_template_reference(
    profile: &MacroProfile,
    path: Option<&String>,
    owner: &str,
) -> Result<(), String> {
    let Some(path) = path else {
        return Err(format!("{owner}尚未选择图片模板"));
    };
    if !profile
        .templates
        .iter()
        .any(|template| &template.path == path)
    {
        return Err(format!("{owner}引用的模板不存在"));
    }
    Ok(())
}

#[derive(Debug, Clone)]
enum StepControl {
    Continue,
    CompleteRound,
    Stop(String),
}

fn run_workflow(
    profile: MacroProfile,
    target: TargetWindow,
    stop: Arc<AtomicBool>,
    events: mpsc::Sender<RunnerEvent>,
) {
    let templates = match load_templates(&profile.templates) {
        Ok(templates) => templates,
        Err(error) => {
            let _ = events.send(RunnerEvent::Failed(error));
            return;
        }
    };
    let deadline = if profile.loop_mode == LoopMode::Deadline {
        match resolve_deadline(&profile.deadline) {
            Ok(value) => Some(value),
            Err(error) => {
                let _ = events.send(RunnerEvent::Failed(error));
                return;
            }
        }
    } else {
        None
    };
    let _ = events.send(RunnerEvent::Started);
    let mut completed_rounds = 0_u32;
    let mut ctx = StepContext {
        profile: &profile,
        target,
        templates: &templates,
        stop: &stop,
        events: &events,
        jitter: Jitter::new(),
    };

    'rounds: loop {
        if stop.load(Ordering::Acquire) {
            let _ = events.send(RunnerEvent::Stopped("用户停止".to_owned()));
            break;
        }
        if loop_condition_reached(&profile, completed_rounds, deadline) {
            let _ = events.send(RunnerEvent::Stopped(stop_reason(&profile)));
            break;
        }

        for step in profile.steps.iter().filter(|step| step.enabled) {
            if stop.load(Ordering::Acquire) {
                let _ = events.send(RunnerEvent::Stopped("用户停止".to_owned()));
                break 'rounds;
            }
            if profile.loop_mode == LoopMode::Deadline
                && !profile.finish_current_round
                && deadline.is_some_and(|deadline| Local::now() >= deadline)
            {
                let _ = events.send(RunnerEvent::Stopped("已到截止时间".to_owned()));
                break 'rounds;
            }

            let _ = events.send(RunnerEvent::StepChanged(step.name.clone()));
            let result: Result<StepControl, String> = match step.kind {
                StepKind::WaitAndClick => {
                    wait_and_click(&mut ctx, step).map(|()| StepControl::Continue)
                }
                StepKind::Delay => {
                    interruptible_wait(Duration::from_millis(step.delay_ms as u64), &stop)
                        .map(|()| StepControl::Continue)
                }
                StepKind::RoundEnd => Ok(StepControl::CompleteRound),
                StepKind::WaitAny => wait_any(&mut ctx, step),
                StepKind::Branch => Err(format!("步骤“{}”的条件分支执行尚未开放", step.name)),
                StepKind::VisualCondition => visual_condition(&mut ctx, step),
            };
            match result {
                Ok(StepControl::Continue) => {}
                Ok(StepControl::CompleteRound) => {
                    completed_rounds += 1;
                    let _ = events.send(RunnerEvent::RoundCompleted(completed_rounds));
                    continue 'rounds;
                }
                Ok(StepControl::Stop(reason)) => {
                    let _ = events.send(RunnerEvent::Stopped(reason));
                    break 'rounds;
                }
                Err(error) => {
                    let _ = events.send(if stop.load(Ordering::Acquire) {
                        RunnerEvent::Stopped("用户停止".to_owned())
                    } else {
                        RunnerEvent::Failed(error)
                    });
                    break 'rounds;
                }
            }
        }
    }
}

fn load_templates(assets: &[TemplateAsset]) -> Result<HashMap<String, LoadedTemplate>, String> {
    let mut templates = HashMap::new();
    for asset in assets {
        let image = image::open(&asset.path)
            .map_err(|error| format!("无法读取模板“{}”：{error}", asset.name))?
            .into_luma8();
        templates.insert(
            asset.path.clone(),
            LoadedTemplate {
                asset: asset.clone(),
                image,
                last_match: Cell::new(None),
            },
        );
    }
    Ok(templates)
}

struct LoadedTemplate {
    asset: TemplateAsset,
    image: GrayImage,
    /// Last match position in client coordinates; used to try a small
    /// neighborhood before paying for a full-region scan.
    last_match: Cell<Option<(u32, u32)>>,
}

/// Small deterministic PRNG (xorshift64) for click humanization.
struct Jitter(u64);

impl Jitter {
    fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() ^ u64::from(duration.subsec_nanos()))
            .unwrap_or(0x5771)
            ^ u64::from(std::process::id());
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    /// Returns a value in `[-radius, radius]`.
    fn offset(&mut self, radius: i32, enabled: bool) -> i32 {
        if !enabled || radius <= 0 {
            return 0;
        }
        (self.next() % (radius as u64 * 2 + 1)) as i32 - radius
    }

    /// Returns `ms` with up to ±20% random jitter applied.
    fn jitter_ms(&mut self, ms: u32, enabled: bool) -> u64 {
        let spread = u64::from(ms) / 5;
        if !enabled || spread == 0 {
            return u64::from(ms);
        }
        let delta = (self.next() % (spread * 2 + 1)) as i64 - spread as i64;
        (i64::from(ms) + delta).max(0) as u64
    }
}

#[derive(Default)]
struct WaitState {
    paused: bool,
    paused_since: Option<Instant>,
    window_missing: bool,
    /// Set when the gate just left a paused state; steps that rely on
    /// consecutive observations should restart their counting.
    resumed: bool,
}

struct StepContext<'a> {
    profile: &'a MacroProfile,
    target: TargetWindow,
    templates: &'a HashMap<String, LoadedTemplate>,
    stop: &'a AtomicBool,
    events: &'a mpsc::Sender<RunnerEvent>,
    jitter: Jitter,
}

impl StepContext<'_> {
    /// Clicks client coordinates, applying optional humanization and the
    /// configured click method.
    fn click(&mut self, x: u32, y: u32) -> Result<(), String> {
        let humanize = self.profile.click_jitter;
        let x = (x as i32 + self.jitter.offset(3, humanize))
            .clamp(0, self.target.client_width.saturating_sub(1) as i32) as u32;
        let y = (y as i32 + self.jitter.offset(3, humanize))
            .clamp(0, self.target.client_height.saturating_sub(1) as i32) as u32;
        match self.profile.click_method {
            ClickMethod::Foreground => platform::click_client(&self.target, x, y),
            ClickMethod::Background => platform::click_client_background(&self.target, x, y),
        }
        .map_err(|error| error.to_string())
    }

    /// Interruptible wait with optional humanization jitter.
    fn wait(&mut self, ms: u32) -> Result<(), String> {
        let humanize = self.profile.click_jitter;
        let duration = Duration::from_millis(self.jitter.jitter_ms(ms, humanize));
        interruptible_wait(duration, self.stop)
    }

    /// Handles the stop flag, dead-window reconnection and foreground pausing
    /// for one poll iteration. Returns Ok(true) when the step may proceed and
    /// Ok(false) when the iteration should be skipped.
    fn poll_gate(
        &mut self,
        state: &mut WaitState,
        timeout_at: &mut Instant,
    ) -> Result<bool, String> {
        if self.stop.load(Ordering::Acquire) {
            return Err("用户停止".to_owned());
        }
        if state.window_missing || !platform::is_window_alive(&self.target) {
            if !state.window_missing {
                state.window_missing = true;
                if !state.paused {
                    state.paused = true;
                    state.paused_since = Some(Instant::now());
                    let _ = self.events.send(RunnerEvent::Paused(
                        "游戏窗口已关闭，等待重新连接".to_owned(),
                    ));
                }
            }
            match platform::find_target_window(&self.profile.target_window) {
                Ok(found) => {
                    let _ = self
                        .events
                        .send(RunnerEvent::TargetReconnected(found.title.clone()));
                    self.target = found;
                    state.window_missing = false;
                }
                Err(_) => {
                    interruptible_wait(Duration::from_millis(500), self.stop)?;
                    return Ok(false);
                }
            }
        }
        if !platform::is_foreground(&self.target) {
            if !state.paused {
                let _ = self.events.send(RunnerEvent::Paused(
                    "游戏失去前台，已暂停识别和点击".to_owned(),
                ));
                state.paused = true;
                state.paused_since = Some(Instant::now());
            }
            interruptible_wait(Duration::from_millis(250), self.stop)?;
            return Ok(false);
        }
        if state.paused {
            if let Some(started) = state.paused_since.take() {
                *timeout_at += started.elapsed();
            }
            state.resumed = true;
            let _ = self.events.send(RunnerEvent::Resumed);
            state.paused = false;
        }
        Ok(true)
    }
}

fn wait_and_click(ctx: &mut StepContext<'_>, step: &WorkflowStep) -> Result<(), String> {
    let path = step
        .template
        .as_ref()
        .ok_or_else(|| format!("步骤“{}”没有图片模板", step.name))?;
    let template = ctx
        .templates
        .get(path)
        .ok_or_else(|| format!("步骤“{}”的图片模板未加载", step.name))?;
    let mut timeout_at = Instant::now() + Duration::from_secs(step.timeout_secs as u64);
    let mut state = WaitState::default();

    loop {
        if !ctx.poll_gate(&mut state, &mut timeout_at)? {
            continue;
        }
        if Instant::now() >= timeout_at {
            break;
        }

        let frame = platform::capture_client(&ctx.target).map_err(|error| error.to_string())?;
        ensure_expected_size(&frame, ctx.profile)?;
        let frame_gray = image::imageops::grayscale(&frame);

        if let Some(found) = find_loaded_template(
            &frame_gray,
            frame.width(),
            frame.height(),
            template,
            step.threshold,
        ) {
            let (x, y) = found.center();
            ctx.click(x, y)?;
            let _ = ctx.events.send(RunnerEvent::MatchFound {
                name: step.name.clone(),
                score: found.score,
            });
            return ctx.wait(step.delay_ms);
        }
        interruptible_wait(Duration::from_millis(300), ctx.stop)?;
    }
    Err(format!(
        "步骤“{}”在 {} 秒内未找到目标",
        step.name, step.timeout_secs
    ))
}

fn wait_any(ctx: &mut StepContext<'_>, step: &WorkflowStep) -> Result<StepControl, String> {
    let mut timeout_at = Instant::now() + Duration::from_secs(step.timeout_secs as u64);
    let mut state = WaitState::default();

    loop {
        if !ctx.poll_gate(&mut state, &mut timeout_at)? {
            continue;
        }
        if Instant::now() >= timeout_at {
            return Err(format!(
                "步骤“{}”在 {} 秒内未匹配任何分支",
                step.name, step.timeout_secs
            ));
        }

        let frame = platform::capture_client(&ctx.target).map_err(|error| error.to_string())?;
        ensure_expected_size(&frame, ctx.profile)?;
        let frame_gray = image::imageops::grayscale(&frame);

        let mut matched = None;
        for branch in &step.branches {
            let path = branch
                .trigger_template
                .as_ref()
                .ok_or_else(|| format!("分支“{}”没有触发模板", branch.name))?;
            let template = ctx
                .templates
                .get(path)
                .ok_or_else(|| format!("分支“{}”的触发模板未加载", branch.name))?;
            if let Some(found) = find_loaded_template(
                &frame_gray,
                frame.width(),
                frame.height(),
                template,
                branch.threshold,
            ) {
                matched = Some((branch, found));
                break;
            }
        }

        let Some((branch, found)) = matched else {
            interruptible_wait(Duration::from_millis(300), ctx.stop)?;
            continue;
        };

        let _ = ctx.events.send(RunnerEvent::BranchMatched {
            step: step.name.clone(),
            branch: branch.name.clone(),
            score: found.score,
        });
        if branch.click_trigger {
            let (x, y) = found.center();
            ctx.click(x, y)?;
            ctx.wait(branch.trigger_delay_ms)?;
        }
        execute_branch_actions(ctx, branch)?;

        match branch.outcome {
            BranchOutcome::ContinueFlow => return Ok(StepControl::Continue),
            BranchOutcome::RepeatWait => {
                timeout_at = Instant::now() + Duration::from_secs(step.timeout_secs as u64);
                let _ = ctx.events.send(RunnerEvent::StepChanged(step.name.clone()));
            }
            BranchOutcome::CompleteRound => return Ok(StepControl::CompleteRound),
            BranchOutcome::StopTask => {
                return Ok(StepControl::Stop(format!(
                    "分支“{}”要求停止任务",
                    branch.name
                )));
            }
        }
    }
}

fn visual_condition(ctx: &mut StepContext<'_>, step: &WorkflowStep) -> Result<StepControl, String> {
    let spec = &step.visual_condition;
    let mut timeout_at = Instant::now() + Duration::from_secs(step.timeout_secs as u64);
    let mut state = WaitState::default();
    let mut stable_hits = 0_u8;

    loop {
        if !ctx.poll_gate(&mut state, &mut timeout_at)? {
            continue;
        }
        if state.resumed {
            // Frames across a pause are not consecutive observations.
            state.resumed = false;
            stable_hits = 0;
        }
        if Instant::now() >= timeout_at {
            return Err(format!(
                "步骤“{}”的视觉条件在 {} 秒内未满足",
                step.name, step.timeout_secs
            ));
        }

        let frame = platform::capture_client(&ctx.target).map_err(|error| error.to_string())?;
        ensure_expected_size(&frame, ctx.profile)?;
        let frame_gray = image::imageops::grayscale(&frame);

        let mut satisfied = 0_usize;
        let mut matched: Option<(&VisualConditionTerm, vision::TemplateMatch)> = None;
        for term in &spec.terms {
            let path = term
                .template
                .as_ref()
                .ok_or_else(|| format!("视觉条件“{}”没有图片模板", term.name))?;
            let template = ctx
                .templates
                .get(path)
                .ok_or_else(|| format!("视觉条件“{}”的图片模板未加载", term.name))?;
            let found = find_loaded_template(
                &frame_gray,
                frame.width(),
                frame.height(),
                template,
                term.threshold,
            );
            let term_met = match term.expectation {
                ConditionExpectation::Present => found.is_some(),
                ConditionExpectation::Absent => found.is_none(),
            };
            if term_met {
                satisfied += 1;
            }
            if matched.is_none()
                && let Some(found) = found
            {
                matched = Some((term, found));
            }
        }

        let condition_met = !spec.terms.is_empty()
            && match spec.mode {
                ConditionMatchMode::All => satisfied == spec.terms.len(),
                ConditionMatchMode::Any => satisfied > 0,
            };
        if condition_met {
            stable_hits = stable_hits.saturating_add(1);
        } else {
            stable_hits = 0;
        }
        if stable_hits < spec.stable_checks {
            interruptible_wait(Duration::from_millis(300), ctx.stop)?;
            continue;
        }

        let _ = ctx.events.send(RunnerEvent::ConditionMatched {
            step: step.name.clone(),
        });
        match spec.outcome {
            ConditionOutcome::ContinueFlow => return Ok(StepControl::Continue),
            ConditionOutcome::ClickTemplate => {
                if let Some((term, found)) = matched {
                    let (x, y) = found.center();
                    ctx.click(x, y)?;
                    let _ = ctx.events.send(RunnerEvent::MatchFound {
                        name: term.name.clone(),
                        score: found.score,
                    });
                    ctx.wait(step.delay_ms)?;
                }
                return Ok(StepControl::Continue);
            }
            ConditionOutcome::CompleteRound => return Ok(StepControl::CompleteRound),
            ConditionOutcome::StopTask => {
                return Ok(StepControl::Stop(format!(
                    "视觉条件“{}”要求停止任务",
                    step.name
                )));
            }
        }
    }
}

fn execute_branch_actions(
    ctx: &mut StepContext<'_>,
    branch: &WorkflowBranch,
) -> Result<(), String> {
    for action in &branch.actions {
        let _ = ctx.events.send(RunnerEvent::StepChanged(format!(
            "{} / {}",
            branch.name, action.name
        )));
        match action.kind {
            BranchActionKind::WaitAndClick => {
                wait_and_click_action(ctx, action)?;
            }
            BranchActionKind::Delay => {
                ctx.wait(action.delay_ms)?;
            }
        }
    }
    Ok(())
}

fn wait_and_click_action(ctx: &mut StepContext<'_>, action: &BranchAction) -> Result<(), String> {
    let step = WorkflowStep {
        id: action.id,
        name: action.name.clone(),
        kind: StepKind::WaitAndClick,
        indent: 0,
        enabled: true,
        template: action.template.clone(),
        threshold: action.threshold,
        timeout_secs: action.timeout_secs,
        delay_ms: action.delay_ms,
        branches: Vec::new(),
        visual_condition: VisualConditionSpec::default(),
    };
    wait_and_click(ctx, &step)
}

fn ensure_expected_size(frame: &image::RgbaImage, profile: &MacroProfile) -> Result<(), String> {
    if frame.width() == profile.expected_client_width
        && frame.height() == profile.expected_client_height
    {
        Ok(())
    } else {
        Err(format!(
            "客户区尺寸已变化：当前 {} × {}，流程要求 {} × {}",
            frame.width(),
            frame.height(),
            profile.expected_client_width,
            profile.expected_client_height
        ))
    }
}

fn find_loaded_template(
    frame: &GrayImage,
    frame_width: u32,
    frame_height: u32,
    template: &LoadedTemplate,
    threshold: f32,
) -> Option<vision::TemplateMatch> {
    let base_region = template
        .asset
        .search_region
        .filter(|_| {
            template.asset.reference_width == frame_width
                && template.asset.reference_height == frame_height
        })
        .map(|region| SearchRegion {
            x: region.x,
            y: region.y,
            width: region.width,
            height: region.height,
        })
        .unwrap_or_else(|| SearchRegion::full(frame));

    if let Some((last_x, last_y)) = template.last_match.get()
        && let Some(tracked) = tracking_region(
            base_region,
            last_x,
            last_y,
            template.image.width(),
            template.image.height(),
        )
        && let Some(found) = vision::find_template(frame, &template.image, tracked, threshold)
    {
        template.last_match.set(Some((found.x, found.y)));
        return Some(found);
    }
    let found = vision::find_template(frame, &template.image, base_region, threshold);
    template
        .last_match
        .set(found.map(|found| (found.x, found.y)));
    found
}

/// Extra pixels scanned around the last known match position before falling
/// back to a full scan; stationary UI elements are found almost for free.
const TRACK_MARGIN: u32 = 48;

fn tracking_region(
    base: SearchRegion,
    last_x: u32,
    last_y: u32,
    template_width: u32,
    template_height: u32,
) -> Option<SearchRegion> {
    let left = last_x.saturating_sub(TRACK_MARGIN).max(base.x);
    let top = last_y.saturating_sub(TRACK_MARGIN).max(base.y);
    let right = (last_x + template_width + TRACK_MARGIN).min(base.x + base.width);
    let bottom = (last_y + template_height + TRACK_MARGIN).min(base.y + base.height);
    if right.saturating_sub(left) >= template_width && bottom.saturating_sub(top) >= template_height
    {
        Some(SearchRegion {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    } else {
        None
    }
}

fn interruptible_wait(duration: Duration, stop: &AtomicBool) -> Result<(), String> {
    let end = Instant::now() + duration;
    while Instant::now() < end {
        if stop.load(Ordering::Acquire) {
            return Err("用户停止".to_owned());
        }
        std::thread::sleep(
            Duration::from_millis(25).min(end.saturating_duration_since(Instant::now())),
        );
    }
    Ok(())
}

fn parse_deadline(value: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(value, "%H:%M").map_err(|_| "截止时间必须使用 HH:MM 格式".to_owned())
}

fn resolve_deadline(value: &str) -> Result<DateTime<Local>, String> {
    resolve_deadline_at(Local::now(), value)
}

/// Resolves an HH:MM deadline to an absolute timestamp. A time that already
/// passed today rolls over to tomorrow — overnight automation is the norm.
fn resolve_deadline_at(now: DateTime<Local>, value: &str) -> Result<DateTime<Local>, String> {
    let time = parse_deadline(value)?;
    let naive = now.date_naive().and_time(time);
    let mut deadline = naive
        .and_local_timezone(Local)
        .earliest()
        .ok_or_else(|| "截止时间无效".to_owned())?;
    if deadline <= now {
        deadline += chrono::Duration::days(1);
    }
    Ok(deadline)
}

fn loop_condition_reached(
    profile: &MacroProfile,
    completed_rounds: u32,
    deadline: Option<DateTime<Local>>,
) -> bool {
    match profile.loop_mode {
        LoopMode::Count => completed_rounds >= profile.loop_count,
        LoopMode::Deadline => deadline.is_some_and(|deadline| Local::now() >= deadline),
        LoopMode::Continuous => false,
    }
}

fn stop_reason(profile: &MacroProfile) -> String {
    match profile.loop_mode {
        LoopMode::Count => "已完成指定局数".to_owned(),
        LoopMode::Deadline => "已到截止时间并完成当前对局".to_owned(),
        LoopMode::Continuous => "运行结束".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BranchOutcome, WorkflowBranch};

    #[test]
    fn executable_profile_requires_templates() {
        let profile = MacroProfile::default();
        let error = validate_executable_profile(&profile).unwrap_err();
        assert!(error.contains("尚未选择图片模板"));
    }

    #[test]
    fn count_condition_uses_completed_rounds() {
        let profile = MacroProfile {
            loop_mode: LoopMode::Count,
            loop_count: 3,
            ..MacroProfile::default()
        };
        assert!(!loop_condition_reached(&profile, 2, None));
        assert!(loop_condition_reached(&profile, 3, None));
    }

    #[test]
    fn parses_24_hour_deadline() {
        assert_eq!(
            parse_deadline("23:30").unwrap(),
            NaiveTime::from_hms_opt(23, 30, 0).unwrap()
        );
        assert!(parse_deadline("25:00").is_err());
    }

    #[test]
    fn passed_deadline_rolls_to_next_day() {
        use chrono::TimeZone;
        let now = Local.with_ymd_and_hms(2026, 8, 31, 23, 30, 0).unwrap();
        let deadline = resolve_deadline_at(now, "01:00").unwrap();
        assert_eq!(
            deadline.date_naive(),
            now.date_naive() + chrono::Duration::days(1)
        );
        assert_eq!(deadline.time(), NaiveTime::from_hms_opt(1, 0, 0).unwrap());
    }

    #[test]
    fn future_deadline_stays_today() {
        use chrono::TimeZone;
        let now = Local.with_ymd_and_hms(2026, 8, 31, 12, 0, 0).unwrap();
        let deadline = resolve_deadline_at(now, "23:30").unwrap();
        assert_eq!(deadline.date_naive(), now.date_naive());
    }

    #[test]
    fn executable_profile_accepts_wait_any_branches() {
        let template_path = "templates/test.png".to_owned();
        let mut profile = MacroProfile::default();
        profile.templates.push(TemplateAsset {
            id: 1,
            name: "test".to_owned(),
            path: template_path.clone(),
            width: 20,
            height: 20,
            reference_width: 1280,
            reference_height: 720,
            search_region: None,
        });
        profile.steps[0].template = Some(template_path.clone());
        profile.steps[1].kind = StepKind::WaitAny;
        profile.steps[1].template = None;
        let mut branch = WorkflowBranch::new(1, "settlement");
        branch.trigger_template = Some(template_path.clone());
        branch.outcome = BranchOutcome::CompleteRound;
        profile.steps[1].branches.push(branch);
        profile.steps[2].template = Some(template_path);

        assert!(validate_executable_profile(&profile).is_ok());
    }

    #[test]
    fn executable_profile_accepts_visual_condition() {
        let template_path = "templates/test.png".to_owned();
        let mut profile = MacroProfile::default();
        profile.templates.push(TemplateAsset {
            id: 1,
            name: "test".to_owned(),
            path: template_path.clone(),
            width: 20,
            height: 20,
            reference_width: 1280,
            reference_height: 720,
            search_region: None,
        });
        for step in &mut profile.steps[..3] {
            step.template = Some(template_path.clone());
        }
        profile.steps[1].kind = StepKind::VisualCondition;
        profile.steps[1].template = None;
        let mut term =
            VisualConditionTerm::new(1, "settlement shown", ConditionExpectation::Present);
        term.template = Some(template_path);
        profile.steps[1].visual_condition.terms.push(term);

        assert!(validate_executable_profile(&profile).is_ok());
    }

    #[test]
    fn executable_profile_rejects_visual_condition_without_terms() {
        let mut profile = MacroProfile::default();
        let template_path = "templates/test.png".to_owned();
        profile.templates.push(TemplateAsset {
            id: 1,
            name: "test".to_owned(),
            path: template_path.clone(),
            width: 20,
            height: 20,
            reference_width: 1280,
            reference_height: 720,
            search_region: None,
        });
        for step in &mut profile.steps[..3] {
            step.template = Some(template_path.clone());
        }
        profile.steps[1].kind = StepKind::VisualCondition;
        profile.steps[1].template = None;

        let error = validate_executable_profile(&profile).unwrap_err();
        assert!(error.contains("至少需要一条视觉条件"));
    }
}
