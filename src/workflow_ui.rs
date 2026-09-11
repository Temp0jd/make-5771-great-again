use eframe::egui::{self, Color32};

use crate::model::{
    KeyInputMode, MacroProfile, SearchStrategy, StepKind, WorkflowStep, parse_key_combo,
};
use crate::platform::TargetWindow;
use crate::theme;
use crate::vision::TemplateMatch;

// ---------------------------------------------------------------------------
// 1. Save Status & Dirty Tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveStatus {
    Saved,
    Modified,
    Failed(String),
}

#[derive(Debug, Clone, Default)]
pub struct SaveTracker {
    saved_snapshot: Option<String>,
    last_error: Option<String>,
}

impl SaveTracker {
    pub fn new(profile: &MacroProfile) -> Self {
        let saved_snapshot = serde_json::to_string(profile).ok();
        Self {
            saved_snapshot,
            last_error: None,
        }
    }

    pub fn record_saved(&mut self, profile: &MacroProfile) {
        self.saved_snapshot = serde_json::to_string(profile).ok();
        self.last_error = None;
    }

    pub fn record_failed(&mut self, error: impl Into<String>) {
        self.last_error = Some(error.into());
    }

    pub fn status(&self, current: &MacroProfile) -> SaveStatus {
        if let Some(error) = &self.last_error {
            return SaveStatus::Failed(error.clone());
        }
        let current_json = serde_json::to_string(current).ok();
        if current_json == self.saved_snapshot {
            SaveStatus::Saved
        } else {
            SaveStatus::Modified
        }
    }

    pub fn is_dirty(&self, current: &MacroProfile) -> bool {
        serde_json::to_string(current).ok() != self.saved_snapshot
    }
}

// ---------------------------------------------------------------------------
// 2. Step Summary & Warnings
// ---------------------------------------------------------------------------

pub fn step_kind_chip(kind: StepKind) -> (&'static str, Color32) {
    match kind {
        StepKind::WaitAndClick => ("点击", theme::blue()),
        StepKind::WaitAny => ("任一", theme::orange()),
        StepKind::VisualCondition => ("条件", theme::green()),
        StepKind::Delay => ("等待", theme::tertiary_label()),
        StepKind::SendKeys => ("键盘", theme::purple()),
        StepKind::RoundEnd => ("结束", theme::label()),
        StepKind::Branch => ("分支", theme::tertiary_label()),
    }
}

fn scan_summary(step: &WorkflowStep) -> String {
    step.scan_interval_secs
        .map(|seconds| format!("每 {seconds} 秒扫描"))
        .unwrap_or_else(|| "继承扫描频率".to_owned())
}

fn search_summary(strategy: SearchStrategy) -> &'static str {
    match strategy {
        SearchStrategy::Inherit => "继承搜索范围",
        SearchStrategy::FullFrame => "全屏搜索",
        SearchStrategy::FixedRoi => "限定区域",
        SearchStrategy::RoiThenFullFrame => "区域优先",
    }
}

pub fn step_summary(step: &WorkflowStep, templates: &[(u64, String, String)]) -> String {
    let template_name = |path: &Option<String>| -> String {
        path.as_ref()
            .map(|p| {
                templates
                    .iter()
                    .find(|(_, _, candidate)| candidate == p)
                    .map(|(_, name, _)| name.clone())
                    .unwrap_or_else(|| "模板已丢失".to_owned())
            })
            .unwrap_or_else(|| "未选择模板".to_owned())
    };
    let mut summary = match step.kind {
        StepKind::WaitAndClick => format!(
            "{}，{}，{}，超时 {}s{}",
            template_name(&step.template),
            search_summary(step.search.strategy),
            scan_summary(step),
            step.timeout_secs,
            if step.click_count > 1 {
                format!("，连点 {} 次", step.click_count)
            } else {
                String::new()
            }
        ),
        StepKind::WaitAny => format!(
            "{} 个目标分支，{}，超时 {}s",
            step.branches.len(),
            scan_summary(step),
            step.timeout_secs
        ),
        StepKind::VisualCondition => format!(
            "{} 个检查项，{}，满足后{}",
            step.visual_condition.terms.len(),
            scan_summary(step),
            step.visual_condition.outcome.label()
        ),
        StepKind::Delay => format!("等待 {:.1}s", step.delay_ms as f32 / 1000.0),
        StepKind::SendKeys => match step.key_mode {
            KeyInputMode::Text => {
                if step.key_text.trim().is_empty() {
                    "未设置文本".to_owned()
                } else {
                    let text: String = step.key_text.chars().take(12).collect();
                    format!("键入“{text}” · 间隔 {}ms", step.key_interval_ms)
                }
            }
            KeyInputMode::Combo => {
                if step.key_combo.trim().is_empty() {
                    "未设置按键".to_owned()
                } else {
                    format!("按键 {}", step.key_combo)
                }
            }
        },
        StepKind::RoundEnd => "结算本局并开始下一轮".to_owned(),
        StepKind::Branch => "条件分支（旧版占位）".to_owned(),
    };
    if step.kind == StepKind::WaitAndClick
        && let Some(point) = step.relative_click
    {
        summary.push_str(&format!(
            "，点击窗口 ({:.1}%, {:.1}%)",
            point.x_percent, point.y_percent
        ));
    }
    if !step.enabled {
        summary.push_str(" · 已停用");
    }
    summary
}

/// Filtering is presentation-only: callers receive matching source indices and
/// never a reordered or cloned workflow.
pub fn filtered_step_indices(
    steps: &[WorkflowStep],
    templates: &[(u64, String, String)],
    query: &str,
) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| {
            if query.is_empty()
                || step.name.to_lowercase().contains(&query)
                || step_summary(step, templates)
                    .to_lowercase()
                    .contains(&query)
            {
                Some(index)
            } else {
                None
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepHealth {
    Ready,
    Disabled,
    NeedsAttention(String),
}

impl StepHealth {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ready => "配置正常",
            Self::Disabled => "已停用",
            Self::NeedsAttention(_) => "需要处理",
        }
    }
}

pub fn step_health(
    step: &WorkflowStep,
    templates: &[(u64, String, String)],
    profile_scan_secs: Option<u8>,
) -> StepHealth {
    if !step.enabled {
        StepHealth::Disabled
    } else if let Some(warning) = step_warning(step, templates, profile_scan_secs) {
        StepHealth::NeedsAttention(warning)
    } else {
        StepHealth::Ready
    }
}

pub fn step_test_unavailable_reason(
    step: &WorkflowStep,
    templates: &[(u64, String, String)],
    target_connected: bool,
) -> Option<&'static str> {
    if step.kind != StepKind::WaitAndClick {
        Some("当前步骤不是单目标识别步骤，请在各分支或检查项内测试")
    } else if step.template.is_none() {
        Some("尚未选择图片模板")
    } else if !templates
        .iter()
        .any(|(_, _, path)| Some(path) == step.template.as_ref())
    {
        Some("引用的图片模板不存在")
    } else if !target_connected {
        Some("尚未连接游戏窗口")
    } else {
        None
    }
}

pub fn step_warning(
    step: &WorkflowStep,
    templates: &[(u64, String, String)],
    profile_scan_secs: Option<u8>,
) -> Option<String> {
    if !step.enabled {
        return None;
    }
    match step.kind {
        StepKind::WaitAndClick => {
            if step.template.is_none() {
                return Some("未选择模板图片".to_owned());
            }
            if let Some(path) = &step.template
                && !templates.iter().any(|(_, _, p)| p == path)
            {
                return Some("引用的模板图片不存在".to_owned());
            }
            if !(0.75..=0.98).contains(&step.threshold) {
                return Some(format!("阈值 {:.2} 较极端", step.threshold));
            }
            let scan_secs = step.scan_interval_secs.or(profile_scan_secs).unwrap_or(1);
            if step.timeout_secs <= u32::from(scan_secs) {
                return Some("超时时间不大于扫描间隔".to_owned());
            }
        }
        StepKind::WaitAny => {
            if step.branches.is_empty() {
                return Some("尚未添加任何分支".to_owned());
            }
            let scan_secs = step.scan_interval_secs.or(profile_scan_secs).unwrap_or(1);
            if step.timeout_secs <= u32::from(scan_secs) {
                return Some("超时时间不大于扫描间隔".to_owned());
            }
            for branch in &step.branches {
                if branch.trigger_template.is_none() {
                    return Some(format!("分支“{}”未选择触发模板", branch.name));
                }
                if !(0.75..=0.98).contains(&branch.threshold) {
                    return Some(format!("分支“{}”阈值较极端", branch.name));
                }
            }
        }
        StepKind::VisualCondition => {
            if step.visual_condition.terms.is_empty() {
                return Some("尚未添加任何检查条件".to_owned());
            }
            for term in &step.visual_condition.terms {
                if term.template.is_none() {
                    return Some(format!("条件“{}”未选择检查模板", term.name));
                }
            }
        }
        StepKind::SendKeys => match step.key_mode {
            KeyInputMode::Text if step.key_text.trim().is_empty() => {
                return Some("未填写输入文本".to_owned());
            }
            KeyInputMode::Combo => {
                if step.key_combo.trim().is_empty() {
                    return Some("未填写按键组合".to_owned());
                }
                if let Err(e) = parse_key_combo(&step.key_combo) {
                    return Some(format!("按键格式错误: {e}"));
                }
            }
            _ => {}
        },
        _ => {}
    }
    None
}

pub fn step_row_text(
    step: &WorkflowStep,
    index: usize,
    selected: bool,
    templates: &[(u64, String, String)],
    profile_scan_secs: Option<u8>,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let format = |size: f32, color: Color32| egui::TextFormat {
        font_id: egui::FontId::proportional(size),
        color,
        ..Default::default()
    };
    let (chip, chip_color) = step_kind_chip(step.kind);
    let name_color = if selected {
        theme::blue()
    } else if step.enabled {
        theme::label()
    } else {
        theme::tertiary_label()
    };
    job.append(
        &format!("{:02}  ", index + 1),
        0.0,
        format(12.0, theme::tertiary_label()),
    );
    job.append(chip, 0.0, format(12.0, chip_color));
    job.append(&format!("  {}", step.name), 0.0, format(13.5, name_color));
    if !step.enabled {
        job.append(" [已停用]", 0.0, format(11.0, theme::tertiary_label()));
    } else if let Some(warning) = step_warning(step, templates, profile_scan_secs) {
        job.append(
            &format!("  [警告] {}", warning),
            0.0,
            format(10.5, theme::orange()),
        );
    }
    job.append("\n", 0.0, format(4.0, theme::tertiary_label()));
    job.append(
        &step_summary(step, templates),
        0.0,
        format(11.0, theme::tertiary_label()),
    );
    job
}

// ---------------------------------------------------------------------------
// 3. Preflight Evaluation, Issue Tracking & Safe Fixes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightSeverity {
    Blocker,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightGroup {
    Blocker,
    AutoFix,
    Suggestion,
}

impl PreflightGroup {
    pub const ALL: [Self; 3] = [Self::Blocker, Self::AutoFix, Self::Suggestion];

    pub fn label(self) -> &'static str {
        match self {
            Self::Blocker => "阻断问题",
            Self::AutoFix => "可自动修复",
            Self::Suggestion => "建议确认",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreflightTarget {
    Window,
    General,
    Step {
        step_id: u64,
        step_name: String,
    },
    Template {
        template_path: String,
        template_name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SafeFix {
    /// Safely adjust step timeout to at least 3x scan interval or 10 seconds.
    AdjustTimeout { step_id: u64, recommended_secs: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreflightIssue {
    pub severity: PreflightSeverity,
    pub title: String,
    pub detail: String,
    pub target: PreflightTarget,
    pub safe_fix: Option<SafeFix>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowPreflightReport {
    pub issues: Vec<PreflightIssue>,
}

impl WorkflowPreflightReport {
    pub fn group_for(issue: &PreflightIssue) -> PreflightGroup {
        if issue.severity == PreflightSeverity::Blocker {
            PreflightGroup::Blocker
        } else if issue.safe_fix.is_some() {
            PreflightGroup::AutoFix
        } else {
            PreflightGroup::Suggestion
        }
    }

    pub fn grouped(&self, group: PreflightGroup) -> impl Iterator<Item = &PreflightIssue> {
        self.issues
            .iter()
            .filter(move |issue| Self::group_for(issue) == group)
    }

    pub fn group_count(&self, group: PreflightGroup) -> usize {
        self.grouped(group).count()
    }

    pub fn has_blockers(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.severity == PreflightSeverity::Blocker)
    }

    pub fn blockers(&self) -> impl Iterator<Item = &PreflightIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == PreflightSeverity::Blocker)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &PreflightIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == PreflightSeverity::Warning)
    }

    pub fn first_blocker_message(&self) -> Option<String> {
        self.blockers().next().map(|b| b.detail.clone())
    }

    pub fn apply_safe_fix(fix: &SafeFix, profile: &mut MacroProfile) -> Result<String, String> {
        match *fix {
            SafeFix::AdjustTimeout {
                step_id,
                recommended_secs,
            } => {
                if let Some(step) = profile.steps.iter_mut().find(|s| s.id == step_id) {
                    let old_timeout = step.timeout_secs;
                    step.timeout_secs = recommended_secs;
                    Ok(format!(
                        "已将步骤“{}”的超时从 {} 秒安全调整为 {} 秒",
                        step.name, old_timeout, recommended_secs
                    ))
                } else {
                    Err("未找到对应步骤".to_owned())
                }
            }
        }
    }
}

pub fn evaluate_preflight(
    profile: &MacroProfile,
    target_window: Option<&TargetWindow>,
    check_files: bool,
) -> WorkflowPreflightReport {
    let mut issues = Vec::new();

    // 1. Target window check
    if target_window.is_none() {
        issues.push(PreflightIssue {
            severity: PreflightSeverity::Blocker,
            title: "未连接游戏窗口".to_owned(),
            detail: "尚未连接游戏窗口，请先选择并连接目标窗口。".to_owned(),
            target: PreflightTarget::Window,
            safe_fix: None,
        });
    }

    // 2. Window resolution difference
    if let Some(target) = target_window
        && (target.client_width != profile.expected_client_width
            || target.client_height != profile.expected_client_height)
    {
        issues.push(PreflightIssue {
            severity: PreflightSeverity::Warning,
            title: "窗口分辨率差异".to_owned(),
            detail: format!(
                "窗口大小（{}×{}）与流程基准（{}×{}）不同，运行中将自动缩放模板。",
                target.client_width,
                target.client_height,
                profile.expected_client_width,
                profile.expected_client_height
            ),
            target: PreflightTarget::General,
            safe_fix: None,
        });
    }

    // 3. Executable profile validation via runner rules
    if let Err(runner_err) = crate::runner::validate_executable_profile(profile) {
        // Find which step caused it, if identifiable
        let matched_step = profile.steps.iter().find(|step| {
            if !step.enabled {
                return false;
            }
            match step.kind {
                StepKind::WaitAndClick => step.template.is_none(),
                StepKind::WaitAny => {
                    step.branches.is_empty()
                        || step.branches.iter().any(|b| b.trigger_template.is_none())
                }
                StepKind::VisualCondition => {
                    step.visual_condition.terms.is_empty()
                        || step
                            .visual_condition
                            .terms
                            .iter()
                            .any(|t| t.template.is_none())
                }
                StepKind::SendKeys => match step.key_mode {
                    KeyInputMode::Text => step.key_text.trim().is_empty(),
                    KeyInputMode::Combo => {
                        step.key_combo.trim().is_empty()
                            || parse_key_combo(&step.key_combo).is_err()
                    }
                },
                _ => false,
            }
        });

        let target = match matched_step {
            Some(step) => PreflightTarget::Step {
                step_id: step.id,
                step_name: step.name.clone(),
            },
            None => PreflightTarget::General,
        };

        issues.push(PreflightIssue {
            severity: PreflightSeverity::Blocker,
            title: "流程配置不完整".to_owned(),
            detail: runner_err,
            target,
            safe_fix: None,
        });
    }

    // 4. File existence check
    if check_files {
        let mut referenced_paths = std::collections::HashSet::new();
        for step in profile.steps.iter().filter(|s| s.enabled) {
            if let Some(path) = &step.template {
                referenced_paths.insert(path.clone());
            }
            for branch in &step.branches {
                if let Some(path) = &branch.trigger_template {
                    referenced_paths.insert(path.clone());
                }
                for action in &branch.actions {
                    if let Some(path) = &action.template {
                        referenced_paths.insert(path.clone());
                    }
                }
            }
            for term in &step.visual_condition.terms {
                if let Some(path) = &term.template {
                    referenced_paths.insert(path.clone());
                }
            }
        }

        for template in &profile.templates {
            if referenced_paths.contains(&template.path)
                && !std::path::Path::new(&template.path).is_file()
            {
                // Find a step referencing it for navigation
                let referencing_step = profile.steps.iter().find(|s| {
                    s.enabled
                        && (s.template.as_deref() == Some(&template.path)
                            || s.branches.iter().any(|b| {
                                b.trigger_template.as_deref() == Some(&template.path)
                                    || b.actions
                                        .iter()
                                        .any(|a| a.template.as_deref() == Some(&template.path))
                            })
                            || s.visual_condition
                                .terms
                                .iter()
                                .any(|t| t.template.as_deref() == Some(&template.path)))
                });

                let target = match referencing_step {
                    Some(s) => PreflightTarget::Step {
                        step_id: s.id,
                        step_name: s.name.clone(),
                    },
                    None => PreflightTarget::Template {
                        template_path: template.path.clone(),
                        template_name: template.name.clone(),
                    },
                };

                issues.push(PreflightIssue {
                    severity: PreflightSeverity::Blocker,
                    title: "模板图片文件缺失".to_owned(),
                    detail: format!(
                        "模板“{}”的图片文件不存在（{}），请重新截图或更新模板。",
                        template.name, template.path
                    ),
                    target,
                    safe_fix: None,
                });
            }
        }
    }

    // 5. Per-step warnings & safe fixes
    for step in profile.steps.iter().filter(|s| s.enabled) {
        if matches!(
            step.kind,
            StepKind::WaitAndClick | StepKind::WaitAny | StepKind::VisualCondition
        ) {
            let scan_secs = step
                .scan_interval_secs
                .or(profile.idle_scan_secs)
                .unwrap_or(1);
            if step.timeout_secs <= u32::from(scan_secs) {
                let recommended = (u32::from(scan_secs) * 3).max(10);
                issues.push(PreflightIssue {
                    severity: PreflightSeverity::Warning,
                    title: "超时时间过短".to_owned(),
                    detail: format!(
                        "步骤“{}”的超时时间（{}秒）不大于扫描间隔（{}秒），可能来不及重试即判定失败。",
                        step.name, step.timeout_secs, scan_secs
                    ),
                    target: PreflightTarget::Step {
                        step_id: step.id,
                        step_name: step.name.clone(),
                    },
                    safe_fix: Some(SafeFix::AdjustTimeout {
                        step_id: step.id,
                        recommended_secs: recommended,
                    }),
                });
            }
        }

        match step.kind {
            StepKind::WaitAndClick => {
                if !(0.75..=0.98).contains(&step.threshold) {
                    issues.push(PreflightIssue {
                        severity: PreflightSeverity::Warning,
                        title: "匹配阈值较极端".to_owned(),
                        detail: format!(
                            "步骤“{}”的匹配阈值（{:.2}）较极端，易导致识别失败或误判。",
                            step.name, step.threshold
                        ),
                        target: PreflightTarget::Step {
                            step_id: step.id,
                            step_name: step.name.clone(),
                        },
                        // 阈值与具体模板、画面和旧算法标定相关，不能安全地自动改写。
                        safe_fix: None,
                    });
                }
            }
            StepKind::WaitAny => {
                for branch in &step.branches {
                    if !(0.75..=0.98).contains(&branch.threshold) {
                        issues.push(PreflightIssue {
                            severity: PreflightSeverity::Warning,
                            title: "分支阈值较极端".to_owned(),
                            detail: format!(
                                "步骤“{}”的分支“{}”阈值（{:.2}）较极端，建议测试确认。",
                                step.name, branch.name, branch.threshold
                            ),
                            target: PreflightTarget::Step {
                                step_id: step.id,
                                step_name: step.name.clone(),
                            },
                            safe_fix: None,
                        });
                    }
                }
            }
            StepKind::VisualCondition => {
                for term in &step.visual_condition.terms {
                    if !(0.75..=0.98).contains(&term.threshold) {
                        issues.push(PreflightIssue {
                            severity: PreflightSeverity::Warning,
                            title: "条件阈值较极端".to_owned(),
                            detail: format!(
                                "步骤“{}”的检查项“{}”阈值（{:.2}）较极端，建议测试确认。",
                                step.name, term.name, term.threshold
                            ),
                            target: PreflightTarget::Step {
                                step_id: step.id,
                                step_name: step.name.clone(),
                            },
                            safe_fix: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    WorkflowPreflightReport { issues }
}

// ---------------------------------------------------------------------------
// 4. Actionable Template Test Diagnostics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestDiagnostic {
    pub severity: DiagnosticSeverity,
    pub title: String,
    pub detail: String,
    pub advice: String,
    pub expert_note: Option<String>,
    pub is_safe_match: bool,
    pub background_warning: Option<String>,
}

pub fn analyze_test_outcome(
    result: Option<TemplateMatch>,
    best_score: f32,
    threshold: f32,
    candidates: &[TemplateMatch],
    mostly_background: bool,
    search_strategy_label: &str,
    expert_mode: bool,
) -> TestDiagnostic {
    let background_warning = if mostly_background {
        Some("模板图像有效特征较少（背景纯色占比过高）。背景过多会削弱抗干扰能力，建议重新截图并紧贴主体目标或文字。".to_owned())
    } else {
        None
    };

    if let Some(found) = result {
        // Check if there are other candidates close to top score or above threshold
        let other_candidates: Vec<&TemplateMatch> = candidates
            .iter()
            .filter(|c| {
                (c.x != found.x || c.y != found.y) && c.score >= (found.score - 0.04).max(0.50)
            })
            .collect();

        if !other_candidates.is_empty() {
            // Ambiguous
            let count = other_candidates.len() + 1;
            TestDiagnostic {
                severity: DiagnosticSeverity::Warning,
                title: "匹配成功 · 但存在相似歧义候选".to_owned(),
                detail: format!(
                    "已命中目标（相似度 {:.3}），但在画面中同时检测到另有 {} 处高分相似候选，实际运行中存在误触风险。",
                    found.score,
                    count - 1
                ),
                advice: "建议方案：1. 将搜索范围改为“限定区域”，缩减寻找边界；2. 重新截取包含更多独特文字、边界或背景特征的画面。".to_owned(),
                expert_note: expert_mode.then(|| {
                    "（专家模式提示：可检查精确 ROI 坐标与锚点偏移；调整阈值前必须逐一确认所有候选）".to_owned()
                }),
                is_safe_match: false,
                background_warning,
            }
        } else {
            // Unique Match
            TestDiagnostic {
                severity: DiagnosticSeverity::Success,
                title: "匹配成功 · 目标唯一可靠".to_owned(),
                detail: format!(
                    "在搜索范围（{}）内精准命中目标，相似度 {:.3}（阈值 {:.2}），未发现干扰候选。",
                    search_strategy_label, found.score, threshold
                ),
                advice: "目标视觉特征鲜明且位置唯一，可直接投入流程运行。".to_owned(),
                expert_note: expert_mode
                    .then(|| "（专家模式提示：当前阈值安全裕度充足，无需调整底层参数）".to_owned()),
                is_safe_match: true,
                background_warning,
            }
        }
    } else {
        // Not matched - NEVER treat as safe match. A score above threshold can still be
        // rejected by the runtime's ambiguity or cross-scale consistency checks.
        if best_score >= threshold {
            TestDiagnostic {
                severity: DiagnosticSeverity::Warning,
                title: format!("候选达到阈值 · 因安全检查未接受（最高 {:.3}）", best_score),
                detail: format!(
                    "至少一个候选达到设定阈值 {:.2}，但识别器发现位置或尺度歧义，因此没有授权点击。",
                    threshold
                ),
                advice: "建议方案：1. 查看黄色候选框并确认干扰位置；2. 使用“限定区域”排除相似目标；3. 重新截图并保留更独特的文字或图标特征。".to_owned(),
                expert_note: expert_mode.then(|| {
                    "（专家模式提示：这是主动安全拒绝，不应通过降低阈值绕过歧义检查）".to_owned()
                }),
                is_safe_match: false,
                background_warning,
            }
        } else {
            let near_miss = best_score >= threshold - 0.08 && best_score >= 0.50;
            if near_miss {
                TestDiagnostic {
                severity: DiagnosticSeverity::Warning,
                title: format!("未达到阈值 · 存在接近候选（最高 {:.3}）", best_score),
                detail: format!(
                    "搜索范围内检测到的最高相似度为 {:.3}，未达到设定阈值 {:.2}（相差 {:.3}）。系统未判定为命中。",
                    best_score,
                    threshold,
                    threshold - best_score
                ),
                advice: "建议方案：1. 确认游戏画面当前是否处于预期状态；2. 检查窗口是否有缩放或轻微光影动态变化；3. 重新截取当前状态的模板。".to_owned(),
                expert_note: expert_mode.then(|| {
                    "（专家模式提示：请先检查候选位置和模板特征；不要仅为让分数过线而降低阈值）".to_owned()
                }),
                is_safe_match: false,
                background_warning,
            }
            } else {
                TestDiagnostic {
                    severity: DiagnosticSeverity::Error,
                    title: format!("未找到目标（最高相似度 {:.3}）", best_score),
                    detail: format!(
                        "在搜索范围（{}）内未检测到目标画面，最高相似度仅为 {:.3}，远低于设定阈值 {:.2}。",
                        search_strategy_label, best_score, threshold
                    ),
                    advice: "建议方案：1. 检查游戏当前是否正处于该步骤画面；2. 检查是否限定了过小的 ROI 导致目标处于区域外；3. 检查游戏分辨率是否有变化；4. 重新截取模板。".to_owned(),
                    expert_note: expert_mode.then(|| {
                        "（专家模式提示：相似度远低于阈值说明特征完全不匹配，调低阈值无法解决本质问题，请勿强行降阈值）".to_owned()
                    }),
                    is_safe_match: false,
                    background_warning,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 5. Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ConditionExpectation, ConditionOutcome, KeyInputMode, WorkflowBranch};

    #[test]
    fn test_step_summaries_for_all_kinds() {
        let templates = vec![
            (1, "开始按钮".to_owned(), "assets/start.png".to_owned()),
            (2, "确认按钮".to_owned(), "assets/ok.png".to_owned()),
        ];

        // WaitAndClick
        let mut click_step = WorkflowStep::new(1, "点击开始", StepKind::WaitAndClick, 0);
        click_step.template = Some("assets/start.png".to_owned());
        click_step.timeout_secs = 45;
        click_step.click_count = 2;
        assert_eq!(
            step_summary(&click_step, &templates),
            "开始按钮，全屏搜索，继承扫描频率，超时 45s，连点 2 次"
        );

        // Disabled
        click_step.enabled = false;
        assert!(step_summary(&click_step, &templates).ends_with("· 已停用"));

        // WaitAny
        let mut wait_any = WorkflowStep::new(2, "等待分支", StepKind::WaitAny, 0);
        wait_any.branches.push(WorkflowBranch::new(1, "分支1"));
        wait_any.branches.push(WorkflowBranch::new(2, "分支2"));
        wait_any.timeout_secs = 30;
        assert_eq!(
            step_summary(&wait_any, &templates),
            "2 个目标分支，继承扫描频率，超时 30s"
        );

        // VisualCondition
        let mut cond_step = WorkflowStep::new(3, "条件判定", StepKind::VisualCondition, 0);
        cond_step
            .visual_condition
            .terms
            .push(crate::model::VisualConditionTerm::new(
                1,
                "项1",
                ConditionExpectation::Present,
            ));
        cond_step.visual_condition.outcome = ConditionOutcome::ContinueFlow;
        assert_eq!(
            step_summary(&cond_step, &templates),
            "1 个检查项，继承扫描频率，满足后继续后续步骤"
        );

        // Delay
        let mut delay_step = WorkflowStep::new(4, "延时", StepKind::Delay, 0);
        delay_step.delay_ms = 1500;
        assert_eq!(step_summary(&delay_step, &templates), "等待 1.5s");

        // SendKeys Text
        let mut key_text = WorkflowStep::new(5, "打字", StepKind::SendKeys, 0);
        key_text.key_mode = KeyInputMode::Text;
        key_text.key_text = "确认退出".to_owned();
        key_text.key_interval_ms = 50;
        assert_eq!(
            step_summary(&key_text, &templates),
            "键入“确认退出” · 间隔 50ms"
        );

        // SendKeys Combo
        let mut key_combo = WorkflowStep::new(6, "组合键", StepKind::SendKeys, 0);
        key_combo.key_mode = KeyInputMode::Combo;
        key_combo.key_combo = "ctrl+enter".to_owned();
        assert_eq!(step_summary(&key_combo, &templates), "按键 ctrl+enter");

        // RoundEnd
        let round_end = WorkflowStep::new(7, "结算", StepKind::RoundEnd, 0);
        assert_eq!(step_summary(&round_end, &templates), "结算本局并开始下一轮");
    }

    #[test]
    fn test_filter_matches_name_and_summary_without_reordering() {
        let templates = vec![(1, "确认按钮".to_owned(), "confirm.png".to_owned())];
        let mut first = WorkflowStep::new(7, "等待确认", StepKind::WaitAndClick, 0);
        first.template = Some("confirm.png".to_owned());
        let second = WorkflowStep::new(3, "暂停", StepKind::Delay, 0);
        let steps = vec![first, second];

        assert_eq!(
            filtered_step_indices(&steps, &templates, "确认按钮"),
            vec![0]
        );
        assert_eq!(filtered_step_indices(&steps, &templates, "暂停"), vec![1]);
        assert_eq!(filtered_step_indices(&steps, &templates, ""), vec![0, 1]);
        assert_eq!(steps[0].id, 7);
        assert_eq!(steps[1].id, 3);
    }

    #[test]
    fn test_step_health_and_test_availability() {
        let templates = vec![];
        let mut step = WorkflowStep::new(1, "识别", StepKind::WaitAndClick, 0);
        assert!(matches!(
            step_health(&step, &templates, Some(3)),
            StepHealth::NeedsAttention(_)
        ));
        assert_eq!(
            step_test_unavailable_reason(&step, &templates, true),
            Some("尚未选择图片模板")
        );
        step.template = Some("missing.png".to_owned());
        assert_eq!(
            step_test_unavailable_reason(&step, &templates, true),
            Some("引用的图片模板不存在")
        );
        let templates = vec![(1, "识别模板".to_owned(), "missing.png".to_owned())];
        assert_eq!(
            step_test_unavailable_reason(&step, &templates, false),
            Some("尚未连接游戏窗口")
        );
        assert_eq!(step_test_unavailable_reason(&step, &templates, true), None);
        step.enabled = false;
        assert_eq!(
            step_health(&step, &templates, Some(3)),
            StepHealth::Disabled
        );
        assert_eq!(step_test_unavailable_reason(&step, &templates, true), None);
    }

    #[test]
    fn test_preflight_grouping_keeps_safe_fix_separate_from_suggestions() {
        let report = WorkflowPreflightReport {
            issues: vec![
                PreflightIssue {
                    severity: PreflightSeverity::Blocker,
                    title: "阻断".to_owned(),
                    detail: String::new(),
                    target: PreflightTarget::General,
                    safe_fix: None,
                },
                PreflightIssue {
                    severity: PreflightSeverity::Warning,
                    title: "超时".to_owned(),
                    detail: String::new(),
                    target: PreflightTarget::General,
                    safe_fix: Some(SafeFix::AdjustTimeout {
                        step_id: 1,
                        recommended_secs: 10,
                    }),
                },
                PreflightIssue {
                    severity: PreflightSeverity::Warning,
                    title: "阈值".to_owned(),
                    detail: String::new(),
                    target: PreflightTarget::General,
                    safe_fix: None,
                },
            ],
        };
        assert_eq!(report.group_count(PreflightGroup::Blocker), 1);
        assert_eq!(report.group_count(PreflightGroup::AutoFix), 1);
        assert_eq!(report.group_count(PreflightGroup::Suggestion), 1);
    }

    #[test]
    fn test_save_tracker_lifecycle() {
        let mut profile = MacroProfile::default();
        let mut tracker = SaveTracker::new(&profile);

        assert_eq!(tracker.status(&profile), SaveStatus::Saved);
        assert!(!tracker.is_dirty(&profile));

        // Modify profile
        profile.name = "新名称".to_owned();
        assert_eq!(tracker.status(&profile), SaveStatus::Modified);
        assert!(tracker.is_dirty(&profile));

        // Record save
        tracker.record_saved(&profile);
        assert_eq!(tracker.status(&profile), SaveStatus::Saved);
        assert!(!tracker.is_dirty(&profile));

        // Record failure
        tracker.record_failed("磁盘写保护");
        assert_eq!(
            tracker.status(&profile),
            SaveStatus::Failed("磁盘写保护".to_owned())
        );

        // Clear error on successful save
        tracker.record_saved(&profile);
        assert_eq!(tracker.status(&profile), SaveStatus::Saved);
    }

    #[test]
    fn test_preflight_evaluation_and_safe_fixes() {
        let mut profile = MacroProfile::default();
        profile.steps.clear();

        // Add a step with short timeout and extreme threshold
        let mut step = WorkflowStep::new(1, "开始", StepKind::WaitAndClick, 0);
        step.template = Some("path.png".to_owned());
        step.timeout_secs = 1;
        step.scan_interval_secs = Some(3);
        step.threshold = 0.99;
        profile.steps.push(step);

        let report = evaluate_preflight(&profile, None, false);
        // Has blocker because target window is None
        assert!(report.has_blockers());
        assert!(report.first_blocker_message().unwrap().contains("游戏窗口"));

        // Has warnings for timeout and extreme threshold
        let warnings: Vec<_> = report.warnings().collect();
        assert!(warnings.iter().any(|w| w.title == "超时时间过短"));
        assert!(warnings.iter().any(|w| w.title == "匹配阈值较极端"));

        // Find safe fixes
        let timeout_fix = warnings
            .iter()
            .find_map(|w| match &w.safe_fix {
                Some(fix @ SafeFix::AdjustTimeout { .. }) => Some(fix),
                _ => None,
            })
            .expect("存在超时安全修复");

        let res = WorkflowPreflightReport::apply_safe_fix(timeout_fix, &mut profile);
        assert!(res.is_ok());
        assert_eq!(profile.steps[0].timeout_secs, 10);

        let threshold_warning = warnings
            .iter()
            .find(|warning| warning.title == "匹配阈值较极端")
            .expect("存在阈值警告");
        assert!(
            threshold_warning.safe_fix.is_none(),
            "模板阈值不能脱离真实画面自动改写"
        );
    }

    #[test]
    fn test_template_test_diagnostics_unique_match() {
        let matched = TemplateMatch {
            x: 100,
            y: 100,
            width: 50,
            height: 50,
            score: 0.96,
        };
        let diag = analyze_test_outcome(
            Some(matched),
            0.96,
            0.90,
            &[matched],
            false,
            "全屏寻找",
            false,
        );
        assert_eq!(diag.severity, DiagnosticSeverity::Success);
        assert!(diag.is_safe_match);
        assert!(diag.title.contains("目标唯一"));
        assert!(diag.expert_note.is_none());
    }

    #[test]
    fn test_template_test_diagnostics_ambiguous_match() {
        let matched = TemplateMatch {
            x: 100,
            y: 100,
            width: 50,
            height: 50,
            score: 0.95,
        };
        let second = TemplateMatch {
            x: 300,
            y: 100,
            width: 50,
            height: 50,
            score: 0.93,
        };
        let diag = analyze_test_outcome(
            Some(matched),
            0.95,
            0.90,
            &[matched, second],
            false,
            "全屏寻找",
            false,
        );
        assert_eq!(diag.severity, DiagnosticSeverity::Warning);
        assert!(!diag.is_safe_match);
        assert!(diag.title.contains("歧义"));
        assert!(diag.advice.contains("限定区域"));
    }

    #[test]
    fn test_template_test_diagnostics_rejected_above_threshold_is_ambiguous() {
        let candidate = TemplateMatch {
            x: 100,
            y: 100,
            width: 50,
            height: 50,
            score: 0.94,
        };
        let diag = analyze_test_outcome(None, 0.94, 0.90, &[candidate], false, "全屏寻找", false);
        assert_eq!(diag.severity, DiagnosticSeverity::Warning);
        assert!(!diag.is_safe_match);
        assert!(diag.title.contains("安全检查未接受"));
        assert!(diag.detail.contains("歧义"));
        assert!(!diag.detail.contains("未达到"));
    }

    #[test]
    fn test_template_test_diagnostics_near_miss_not_safe() {
        let diag_simple = analyze_test_outcome(None, 0.86, 0.90, &[], false, "全屏寻找", false);
        assert_eq!(diag_simple.severity, DiagnosticSeverity::Warning);
        assert!(
            !diag_simple.is_safe_match,
            "Near miss must NEVER be safe match"
        );
        assert!(diag_simple.title.contains("接近候选"));
        assert!(diag_simple.expert_note.is_none());

        let diag_expert = analyze_test_outcome(None, 0.86, 0.90, &[], false, "全屏寻找", true);
        assert!(diag_expert.expert_note.is_some());
        assert!(diag_expert.expert_note.unwrap().contains("专家模式提示"));
    }

    #[test]
    fn test_template_test_diagnostics_total_miss() {
        let diag = analyze_test_outcome(None, 0.45, 0.90, &[], false, "全屏寻找", false);
        assert_eq!(diag.severity, DiagnosticSeverity::Error);
        assert!(!diag.is_safe_match);
        assert!(diag.title.contains("未找到目标"));
        assert!(diag.advice.contains("分辨率"));
    }

    #[test]
    fn test_template_test_diagnostics_mostly_background_flag() {
        let matched = TemplateMatch {
            x: 10,
            y: 10,
            width: 30,
            height: 30,
            score: 0.98,
        };
        let diag = analyze_test_outcome(
            Some(matched),
            0.98,
            0.90,
            &[matched],
            true, // mostly_background
            "全屏寻找",
            false,
        );
        assert!(diag.background_warning.is_some());
        assert!(
            diag.background_warning
                .unwrap()
                .contains("背景纯色占比过高")
        );
    }
}
