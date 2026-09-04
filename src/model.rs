use serde::{Deserialize, Serialize};

use crate::vision::MatchAlgorithm;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Run,
    Flow,
    Templates,
    Logs,
    Settings,
}

impl AppTab {
    pub const ALL: [Self; 5] = [
        Self::Run,
        Self::Flow,
        Self::Templates,
        Self::Logs,
        Self::Settings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Run => "运行",
            Self::Flow => "流程",
            Self::Templates => "模板",
            Self::Logs => "日志",
            Self::Settings => "设置",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    Count,
    Deadline,
    Continuous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ClickMethod {
    #[default]
    Foreground,
    Background,
}

impl ClickMethod {
    pub const ALL: [Self; 2] = [Self::Foreground, Self::Background];

    pub fn label(self) -> &'static str {
        match self {
            Self::Foreground => "前台点击（移动鼠标，兼容性最好）",
            Self::Background => "后台点击（不动鼠标，部分游戏不响应）",
        }
    }
}

fn default_click_jitter() -> bool {
    true
}

/// Where on the matched template box the click lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ClickAnchor {
    #[default]
    Center,
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl ClickAnchor {
    pub const ALL: [Self; 9] = [
        Self::Center,
        Self::TopLeft,
        Self::Top,
        Self::TopRight,
        Self::Left,
        Self::Right,
        Self::BottomLeft,
        Self::Bottom,
        Self::BottomRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Center => "中心",
            Self::TopLeft => "左上",
            Self::Top => "上",
            Self::TopRight => "右上",
            Self::Left => "左",
            Self::Right => "右",
            Self::BottomLeft => "左下",
            Self::Bottom => "下",
            Self::BottomRight => "右下",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum KeyInputMode {
    #[default]
    Text,
    Combo,
}

impl KeyInputMode {
    pub const ALL: [Self; 2] = [Self::Text, Self::Combo];

    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "文本键入",
            Self::Combo => "按键组合",
        }
    }
}

fn default_key_interval_ms() -> u32 {
    60
}

fn default_capture_hotkey() -> String {
    "f6".to_owned()
}

fn default_stop_hotkey() -> String {
    "f8".to_owned()
}

fn default_stable_confirm() -> bool {
    true
}

/// Parses both configured global hotkeys; fails when either is invalid or
/// both resolve to the same combination.
pub fn parse_hotkeys(capture: &str, stop: &str) -> Result<(KeyCombo, KeyCombo), String> {
    let capture = parse_key_combo(capture).map_err(|error| format!("截图热键无效：{error}"))?;
    let stop = parse_key_combo(stop).map_err(|error| format!("停止热键无效：{error}"))?;
    if capture == stop {
        return Err("截图热键和停止热键不能相同".to_owned());
    }
    Ok((capture, stop))
}

/// Portable key identifiers for `SendKeys` steps; mapped to platform virtual
/// keys in the platform layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Enter,
    Esc,
    Space,
    Tab,
    Backspace,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    F(u8),
    Letter(char),
    Digit(char),
}

impl KeyCode {
    fn name(self) -> String {
        match self {
            Self::Enter => "Enter".to_owned(),
            Self::Esc => "Esc".to_owned(),
            Self::Space => "Space".to_owned(),
            Self::Tab => "Tab".to_owned(),
            Self::Backspace => "Backspace".to_owned(),
            Self::Delete => "Delete".to_owned(),
            Self::Home => "Home".to_owned(),
            Self::End => "End".to_owned(),
            Self::PageUp => "PageUp".to_owned(),
            Self::PageDown => "PageDown".to_owned(),
            Self::Up => "↑".to_owned(),
            Self::Down => "↓".to_owned(),
            Self::Left => "←".to_owned(),
            Self::Right => "→".to_owned(),
            Self::F(number) => format!("F{number}"),
            Self::Letter(letter) => letter.to_ascii_uppercase().to_string(),
            Self::Digit(digit) => digit.to_string(),
        }
    }
}

/// A modifier-plus-key combination such as Ctrl+C or Alt+F4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCombo {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: KeyCode,
}

impl KeyCombo {
    /// Human-readable rendering like `Ctrl+Shift+Enter`, shown next to the
    /// combo text box as live parse feedback.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".to_owned());
        }
        if self.shift {
            parts.push("Shift".to_owned());
        }
        if self.alt {
            parts.push("Alt".to_owned());
        }
        parts.push(self.key.name());
        parts.join("+")
    }
}

/// Parses a combo string like `ctrl+c` or `Alt+F4` (case-insensitive, parts
/// joined by `+`) into a `KeyCombo`.
pub fn parse_key_combo(input: &str) -> Result<KeyCombo, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("按键组合不能为空".to_owned());
    }
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut key: Option<KeyCode> = None;
    for part in input.split('+') {
        let token = part.trim().to_lowercase();
        if token.is_empty() {
            return Err("按键组合格式不正确，存在空的按键".to_owned());
        }
        match token.as_str() {
            "ctrl" => {
                if ctrl {
                    return Err("修饰键 Ctrl 重复".to_owned());
                }
                ctrl = true;
            }
            "shift" => {
                if shift {
                    return Err("修饰键 Shift 重复".to_owned());
                }
                shift = true;
            }
            "alt" => {
                if alt {
                    return Err("修饰键 Alt 重复".to_owned());
                }
                alt = true;
            }
            _ => {
                if key.is_some() {
                    return Err("按键组合只能包含一个主键".to_owned());
                }
                key = Some(parse_main_key(&token)?);
            }
        }
    }
    let key = key.ok_or_else(|| "按键组合缺少主键，例如 enter、ctrl+c".to_owned())?;
    Ok(KeyCombo {
        ctrl,
        shift,
        alt,
        key,
    })
}

fn parse_main_key(token: &str) -> Result<KeyCode, String> {
    let key = match token {
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "space" => KeyCode::Space,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        _ => {
            if let Some(digits) = token.strip_prefix('f')
                && let Ok(number) = digits.parse::<u8>()
                && (1..=12).contains(&number)
            {
                return Ok(KeyCode::F(number));
            }
            let mut chars = token.chars();
            if let (Some(single), None) = (chars.next(), chars.next()) {
                if single.is_ascii_lowercase() {
                    return Ok(KeyCode::Letter(single));
                }
                if single.is_ascii_digit() {
                    return Ok(KeyCode::Digit(single));
                }
            }
            return Err(format!("无法识别的按键“{token}”"));
        }
    };
    Ok(key)
}

fn default_ui_scale() -> f32 {
    1.0
}

impl LoopMode {
    pub const ALL: [Self; 3] = [Self::Count, Self::Deadline, Self::Continuous];

    pub fn label(self) -> &'static str {
        match self {
            Self::Count => "固定次数",
            Self::Deadline => "截止时间",
            Self::Continuous => "持续运行",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerStatus {
    Ready,
    Running,
    Paused,
    Finishing,
}

impl RunnerStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "准备就绪",
            Self::Running => "运行中",
            Self::Paused => "已暂停",
            Self::Finishing => "正在停止",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepKind {
    WaitAndClick,
    WaitAny,
    VisualCondition,
    Branch,
    Delay,
    SendKeys,
    RoundEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionMatchMode {
    All,
    Any,
}

impl ConditionMatchMode {
    pub const ALL: [Self; 2] = [Self::All, Self::Any];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "全部满足（AND）",
            Self::Any => "任一满足（OR）",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionExpectation {
    Present,
    Absent,
}

impl ConditionExpectation {
    pub const ALL: [Self; 2] = [Self::Present, Self::Absent];

    pub fn label(self) -> &'static str {
        match self {
            Self::Present => "出现",
            Self::Absent => "不出现",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionOutcome {
    ContinueFlow,
    ClickTemplate,
    CompleteRound,
    StopTask,
}

impl ConditionOutcome {
    pub const ALL: [Self; 4] = [
        Self::ContinueFlow,
        Self::ClickTemplate,
        Self::CompleteRound,
        Self::StopTask,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ContinueFlow => "继续后续步骤",
            Self::ClickTemplate => "点击指定模板",
            Self::CompleteRound => "完成本局",
            Self::StopTask => "停止任务",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualConditionTerm {
    pub id: u64,
    pub name: String,
    pub template: Option<String>,
    pub expectation: ConditionExpectation,
    pub threshold: f32,
}

impl VisualConditionTerm {
    pub fn new(id: u64, name: impl Into<String>, expectation: ConditionExpectation) -> Self {
        Self {
            id,
            name: name.into(),
            template: None,
            expectation,
            threshold: 0.90,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualConditionSpec {
    pub mode: ConditionMatchMode,
    pub stable_checks: u8,
    pub outcome: ConditionOutcome,
    #[serde(default)]
    pub click_anchor: ClickAnchor,
    #[serde(default)]
    pub click_offset_x: i32,
    #[serde(default)]
    pub click_offset_y: i32,
    #[serde(default)]
    pub terms: Vec<VisualConditionTerm>,
}

impl Default for VisualConditionSpec {
    fn default() -> Self {
        Self {
            mode: ConditionMatchMode::All,
            stable_checks: 2,
            outcome: ConditionOutcome::ContinueFlow,
            click_anchor: ClickAnchor::default(),
            click_offset_x: 0,
            click_offset_y: 0,
            terms: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchOutcome {
    ContinueFlow,
    RepeatWait,
    CompleteRound,
    StopTask,
}

impl BranchOutcome {
    pub const ALL: [Self; 4] = [
        Self::RepeatWait,
        Self::ContinueFlow,
        Self::CompleteRound,
        Self::StopTask,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ContinueFlow => "继续后续步骤",
            Self::RepeatWait => "返回当前等待",
            Self::CompleteRound => "完成本局",
            Self::StopTask => "停止任务",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchActionKind {
    WaitAndClick,
    Delay,
}

impl BranchActionKind {
    pub const ALL: [Self; 2] = [Self::WaitAndClick, Self::Delay];

    pub fn label(self) -> &'static str {
        match self {
            Self::WaitAndClick => "等待并点击",
            Self::Delay => "固定等待",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchAction {
    pub id: u64,
    pub name: String,
    pub kind: BranchActionKind,
    pub template: Option<String>,
    pub threshold: f32,
    pub timeout_secs: u32,
    pub delay_ms: u32,
    #[serde(default)]
    pub click_anchor: ClickAnchor,
    #[serde(default)]
    pub click_offset_x: i32,
    #[serde(default)]
    pub click_offset_y: i32,
    /// When true, a WaitAndClick action that times out is skipped instead of
    /// failing the whole branch (for screens that only appear sometimes).
    #[serde(default)]
    pub optional: bool,
}

impl BranchAction {
    pub fn new(id: u64, name: impl Into<String>, kind: BranchActionKind) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            template: None,
            threshold: 0.90,
            timeout_secs: 60,
            delay_ms: 500,
            click_anchor: ClickAnchor::default(),
            click_offset_x: 0,
            click_offset_y: 0,
            optional: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowBranch {
    pub id: u64,
    pub name: String,
    pub trigger_template: Option<String>,
    pub threshold: f32,
    pub click_trigger: bool,
    pub trigger_delay_ms: u32,
    pub outcome: BranchOutcome,
    #[serde(default)]
    pub click_anchor: ClickAnchor,
    #[serde(default)]
    pub click_offset_x: i32,
    #[serde(default)]
    pub click_offset_y: i32,
    #[serde(default)]
    pub actions: Vec<BranchAction>,
}

impl WorkflowBranch {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            trigger_template: None,
            threshold: 0.90,
            click_trigger: false,
            trigger_delay_ms: 400,
            outcome: BranchOutcome::RepeatWait,
            click_anchor: ClickAnchor::default(),
            click_offset_x: 0,
            click_offset_y: 0,
            actions: Vec::new(),
        }
    }
}

impl StepKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::WaitAndClick => "等待并点击",
            Self::WaitAny => "等待任一目标",
            Self::VisualCondition => "视觉条件",
            Self::Branch => "条件分支",
            Self::Delay => "固定等待",
            Self::SendKeys => "键盘输入",
            Self::RoundEnd => "本局结束",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: u64,
    pub name: String,
    pub kind: StepKind,
    pub indent: u8,
    pub enabled: bool,
    pub template: Option<String>,
    pub threshold: f32,
    pub timeout_secs: u32,
    pub delay_ms: u32,
    #[serde(default)]
    pub click_anchor: ClickAnchor,
    #[serde(default)]
    pub click_offset_x: i32,
    #[serde(default)]
    pub click_offset_y: i32,
    #[serde(default)]
    pub key_mode: KeyInputMode,
    #[serde(default)]
    pub key_text: String,
    #[serde(default)]
    pub key_combo: String,
    #[serde(default = "default_key_interval_ms")]
    pub key_interval_ms: u32,
    #[serde(default)]
    pub branches: Vec<WorkflowBranch>,
    #[serde(default)]
    pub visual_condition: VisualConditionSpec,
}

impl WorkflowStep {
    pub fn new(id: u64, name: impl Into<String>, kind: StepKind, indent: u8) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            indent,
            enabled: true,
            template: None,
            threshold: 0.90,
            timeout_secs: 60,
            delay_ms: 500,
            click_anchor: ClickAnchor::default(),
            click_offset_x: 0,
            click_offset_y: 0,
            key_mode: KeyInputMode::default(),
            key_text: String::new(),
            key_combo: String::new(),
            key_interval_ms: default_key_interval_ms(),
            branches: Vec::new(),
            visual_condition: VisualConditionSpec::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateAsset {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub reference_width: u32,
    #[serde(default)]
    pub reference_height: u32,
    #[serde(default)]
    pub search_region: Option<SearchRegionSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRegionSpec {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl SearchRegionSpec {
    /// Rescales the region from its reference resolution to the actual one.
    pub fn scaled(self, from_width: u32, from_height: u32, to_width: u32, to_height: u32) -> Self {
        let scale = |value: u32, from: u32, to: u32| {
            (u64::from(value) * u64::from(to) / u64::from(from.max(1))) as u32
        };
        Self {
            x: scale(self.x, from_width, to_width),
            y: scale(self.y, from_height, to_height),
            width: scale(self.width, from_width, to_width).max(1),
            height: scale(self.height, from_height, to_height).max(1),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroProfile {
    pub name: String,
    pub target_window: String,
    pub expected_client_width: u32,
    pub expected_client_height: u32,
    pub loop_mode: LoopMode,
    pub loop_count: u32,
    pub deadline: String,
    pub finish_current_round: bool,
    pub steps: Vec<WorkflowStep>,
    pub templates: Vec<TemplateAsset>,
    #[serde(default)]
    pub click_method: ClickMethod,
    #[serde(default = "default_click_jitter")]
    pub click_jitter: bool,
    #[serde(default)]
    pub dark_mode: bool,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    #[serde(default = "default_capture_hotkey")]
    pub capture_hotkey: String,
    #[serde(default = "default_stop_hotkey")]
    pub stop_hotkey: String,
    #[serde(default)]
    pub shared_templates: bool,
    /// Existing profiles deserialize to `Precise`; newly created profiles use
    /// `Hybrid` so old threshold calibration is never changed silently.
    #[serde(default)]
    pub match_algorithm: MatchAlgorithm,
    #[serde(default = "default_stable_confirm")]
    pub stable_confirm: bool,
    #[serde(default)]
    pub sharing: SharingMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharingMetadata {
    pub author: String,
    pub description: String,
    pub game_version: String,
    pub game_language: String,
    pub tags: String,
}

impl Default for SharingMetadata {
    fn default() -> Self {
        Self {
            author: String::new(),
            description: String::new(),
            game_version: String::new(),
            game_language: "简体中文".to_owned(),
            tags: String::new(),
        }
    }
}

impl Default for MacroProfile {
    fn default() -> Self {
        Self {
            name: "日常刷关".to_owned(),
            target_window: "忘却前夜".to_owned(),
            expected_client_width: 1280,
            expected_client_height: 720,
            loop_mode: LoopMode::Count,
            loop_count: 20,
            deadline: "23:30".to_owned(),
            finish_current_round: true,
            steps: vec![
                WorkflowStep::new(1, "开始游戏", StepKind::WaitAndClick, 0),
                WorkflowStep::new(2, "开启 Auto", StepKind::WaitAndClick, 0),
                WorkflowStep::new(3, "结算游戏", StepKind::WaitAndClick, 0),
                WorkflowStep::new(4, "本局结束", StepKind::RoundEnd, 0),
            ],
            templates: Vec::new(),
            click_method: ClickMethod::default(),
            click_jitter: default_click_jitter(),
            dark_mode: false,
            ui_scale: default_ui_scale(),
            capture_hotkey: default_capture_hotkey(),
            stop_hotkey: default_stop_hotkey(),
            shared_templates: false,
            match_algorithm: MatchAlgorithm::Hybrid,
            stable_confirm: default_stable_confirm(),
            sharing: SharingMetadata::default(),
        }
    }
}

impl MacroProfile {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut issues = Vec::new();

        if self.name.trim().is_empty() {
            issues.push("流程名称不能为空".to_owned());
        }
        if self.target_window.trim().is_empty() {
            issues.push("目标窗口名称不能为空".to_owned());
        }
        if self.expected_client_width == 0 || self.expected_client_height == 0 {
            issues.push("目标窗口尺寸必须大于零".to_owned());
        }
        if self.loop_mode == LoopMode::Count && self.loop_count == 0 {
            issues.push("固定次数必须至少为 1".to_owned());
        }
        if self.loop_mode == LoopMode::Deadline && !valid_deadline(&self.deadline) {
            issues.push("截止时间必须使用 HH:MM 格式".to_owned());
        }
        if !(0.75..=2.0).contains(&self.ui_scale) {
            issues.push("界面缩放必须在 75% - 200% 之间".to_owned());
        }
        if self.steps.is_empty() {
            issues.push("流程至少需要一个步骤".to_owned());
        }
        if self.steps.len() > 1000 {
            issues.push("流程步骤数不能超过 1000".to_owned());
        }
        for (label, value, limit) in [
            ("作者", &self.sharing.author, 100),
            ("游戏版本", &self.sharing.game_version, 100),
            ("游戏语言", &self.sharing.game_language, 100),
            ("标签", &self.sharing.tags, 500),
            ("分享说明", &self.sharing.description, 4000),
        ] {
            if value.chars().count() > limit {
                issues.push(format!("{label}不能超过 {limit} 个字符"));
            }
        }

        let mut ids = std::collections::HashSet::new();
        for step in &self.steps {
            if !ids.insert(step.id) {
                issues.push(format!("步骤 ID {} 重复", step.id));
            }
            if step.name.trim().is_empty() {
                issues.push(format!("步骤 {} 的名称不能为空", step.id));
            }
            if !(0.0..=1.0).contains(&step.threshold) {
                issues.push(format!("步骤“{}”的相似度不合法", step.name));
            }
            if step.timeout_secs == 0 {
                issues.push(format!("步骤“{}”的超时必须大于零", step.name));
            }
            if step.branches.len() > 100 {
                issues.push(format!("步骤“{}”的分支数不能超过 100", step.name));
            }
            if !(1..=10).contains(&step.visual_condition.stable_checks) {
                issues.push(format!("步骤“{}”的稳定检查次数必须为 1-10", step.name));
            }
            if step.visual_condition.terms.len() > 20 {
                issues.push(format!("步骤“{}”的视觉条件不能超过 20 条", step.name));
            }
            let mut condition_term_ids = std::collections::HashSet::new();
            for term in &step.visual_condition.terms {
                if !condition_term_ids.insert(term.id) {
                    issues.push(format!("步骤“{}”的条件 ID {} 重复", step.name, term.id));
                }
                if term.name.trim().is_empty() {
                    issues.push(format!("步骤“{}”包含未命名视觉条件", step.name));
                }
                if !(0.0..=1.0).contains(&term.threshold) {
                    issues.push(format!("视觉条件“{}”的相似度不合法", term.name));
                }
            }
            let mut branch_ids = std::collections::HashSet::new();
            for branch in &step.branches {
                if !branch_ids.insert(branch.id) {
                    issues.push(format!("步骤“{}”的分支 ID {} 重复", step.name, branch.id));
                }
                if branch.name.trim().is_empty() {
                    issues.push(format!("步骤“{}”包含未命名分支", step.name));
                }
                if !(0.0..=1.0).contains(&branch.threshold) {
                    issues.push(format!("分支“{}”的相似度不合法", branch.name));
                }
                if branch.actions.len() > 100 {
                    issues.push(format!("分支“{}”的动作数不能超过 100", branch.name));
                }

                let mut action_ids = std::collections::HashSet::new();
                for action in &branch.actions {
                    if !action_ids.insert(action.id) {
                        issues.push(format!("分支“{}”的动作 ID {} 重复", branch.name, action.id));
                    }
                    if action.name.trim().is_empty() {
                        issues.push(format!("分支“{}”包含未命名动作", branch.name));
                    }
                    if !(0.0..=1.0).contains(&action.threshold) {
                        issues.push(format!("动作“{}”的相似度不合法", action.name));
                    }
                    if action.timeout_secs == 0 {
                        issues.push(format!("动作“{}”的超时必须大于零", action.name));
                    }
                }
            }
        }

        if self.templates.len() > 500 {
            issues.push("图片模板数不能超过 500".to_owned());
        }
        let mut template_ids = std::collections::HashSet::new();
        let mut template_paths = std::collections::HashSet::new();
        for template in &self.templates {
            if !template_ids.insert(template.id) {
                issues.push(format!("模板 ID {} 重复", template.id));
            }
            if !template_paths.insert(&template.path) {
                issues.push(format!("模板路径重复：{}", template.path));
            }
            if template.name.trim().is_empty() {
                issues.push(format!("模板 {} 的名称不能为空", template.id));
            }
            if template.width == 0 || template.height == 0 {
                issues.push(format!("模板“{}”的尺寸无效", template.name));
            }
            if let Some(region) = template.search_region
                && (region.width == 0
                    || region.height == 0
                    || region.x.saturating_add(region.width) > template.reference_width
                    || region.y.saturating_add(region.height) > template.reference_height)
            {
                issues.push(format!("模板“{}”的搜索区域无效", template.name));
            }
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues)
        }
    }
}

fn valid_deadline(value: &str) -> bool {
    let Some((hours, minutes)) = value.split_once(':') else {
        return false;
    };
    if hours.len() != 2 || minutes.len() != 2 {
        return false;
    }
    matches!(
        (hours.parse::<u8>(), minutes.parse::<u8>()),
        (Ok(0..=23), Ok(0..=59))
    )
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub time: String,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_valid() {
        let profile = MacroProfile::default();
        assert!(profile.validate().is_ok());
        assert_eq!(profile.match_algorithm, MatchAlgorithm::Hybrid);
    }

    #[test]
    fn rejects_invalid_deadline() {
        let profile = MacroProfile {
            loop_mode: LoopMode::Deadline,
            deadline: "25:90".to_owned(),
            ..MacroProfile::default()
        };

        let issues = profile.validate().expect_err("deadline should be rejected");
        assert!(issues.iter().any(|issue| issue.contains("HH:MM")));
    }

    #[test]
    fn rejects_duplicate_step_ids() {
        let mut profile = MacroProfile::default();
        profile.steps[1].id = profile.steps[0].id;

        let issues = profile
            .validate()
            .expect_err("duplicate IDs should be rejected");
        assert!(issues.iter().any(|issue| issue.contains("重复")));
    }

    #[test]
    fn old_steps_load_with_empty_branch_list() {
        let json = r#"{
            "id": 9,
            "name": "old step",
            "kind": "Delay",
            "indent": 0,
            "enabled": true,
            "template": null,
            "threshold": 0.9,
            "timeout_secs": 10,
            "delay_ms": 500
        }"#;
        let step: WorkflowStep = serde_json::from_str(json).unwrap();
        assert!(step.branches.is_empty());
    }

    #[test]
    fn incomplete_wait_any_can_be_saved_as_a_draft() {
        let mut profile = MacroProfile::default();
        profile.steps[1].kind = StepKind::WaitAny;
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn old_profiles_load_with_default_sharing_metadata() {
        let mut value = serde_json::to_value(MacroProfile::default()).unwrap();
        value.as_object_mut().unwrap().remove("sharing");
        let profile: MacroProfile = serde_json::from_value(value).unwrap();
        assert_eq!(profile.sharing.game_language, "简体中文");
        assert!(profile.sharing.author.is_empty());
    }

    #[test]
    fn search_region_scales_proportionally() {
        let region = SearchRegionSpec {
            x: 640,
            y: 360,
            width: 320,
            height: 180,
        };
        let scaled = region.scaled(1280, 720, 1920, 1080);
        assert_eq!(
            scaled,
            SearchRegionSpec {
                x: 960,
                y: 540,
                width: 480,
                height: 270,
            }
        );
    }

    #[test]
    fn old_profiles_load_with_default_click_options() {
        let mut value = serde_json::to_value(MacroProfile::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("click_method");
        object.remove("click_jitter");
        let profile: MacroProfile = serde_json::from_value(value).unwrap();
        assert_eq!(profile.click_method, ClickMethod::Foreground);
        assert!(profile.click_jitter);
    }

    #[test]
    fn old_steps_load_with_default_click_anchor_and_key_fields() {
        let json = r#"{
            "id": 9,
            "name": "old step",
            "kind": "WaitAndClick",
            "indent": 0,
            "enabled": true,
            "template": null,
            "threshold": 0.9,
            "timeout_secs": 10,
            "delay_ms": 500
        }"#;
        let step: WorkflowStep = serde_json::from_str(json).unwrap();
        assert_eq!(step.click_anchor, ClickAnchor::Center);
        assert_eq!(step.click_offset_x, 0);
        assert_eq!(step.click_offset_y, 0);
        assert_eq!(step.key_mode, KeyInputMode::Text);
        assert!(step.key_text.is_empty());
        assert!(step.key_combo.is_empty());
        assert_eq!(step.key_interval_ms, 60);
    }

    #[test]
    fn parses_plain_main_key() {
        assert_eq!(
            parse_key_combo("enter").unwrap(),
            KeyCombo {
                ctrl: false,
                shift: false,
                alt: false,
                key: KeyCode::Enter,
            }
        );
    }

    #[test]
    fn parses_modifiers_case_insensitively() {
        assert_eq!(
            parse_key_combo("CTRL+C").unwrap(),
            KeyCombo {
                ctrl: true,
                shift: false,
                alt: false,
                key: KeyCode::Letter('c'),
            }
        );
        assert_eq!(
            parse_key_combo("alt+f4").unwrap(),
            KeyCombo {
                ctrl: false,
                shift: false,
                alt: true,
                key: KeyCode::F(4),
            }
        );
        assert_eq!(
            parse_key_combo("shift+delete").unwrap(),
            KeyCombo {
                ctrl: false,
                shift: true,
                alt: false,
                key: KeyCode::Delete,
            }
        );
        assert_eq!(
            parse_key_combo("Ctrl+Shift+5").unwrap(),
            KeyCombo {
                ctrl: true,
                shift: true,
                alt: false,
                key: KeyCode::Digit('5'),
            }
        );
    }

    #[test]
    fn rejects_invalid_combos() {
        assert!(parse_key_combo("").is_err());
        assert!(parse_key_combo("   ").is_err());
        assert!(parse_key_combo("ctrl+").is_err());
        assert!(parse_key_combo("foo").is_err());
        assert!(parse_key_combo("ctrl+ctrl+c").is_err());
        assert!(parse_key_combo("ctrl").is_err());
        assert!(parse_key_combo("a+b").is_err());
        assert!(parse_key_combo("f13").is_err());
        assert!(parse_key_combo("ab").is_err());
    }

    #[test]
    fn old_branches_and_actions_load_with_default_click_point() {
        let branch: WorkflowBranch = serde_json::from_str(
            r#"{
                "id": 3,
                "name": "old branch",
                "trigger_template": null,
                "threshold": 0.9,
                "click_trigger": true,
                "trigger_delay_ms": 400,
                "outcome": "RepeatWait"
            }"#,
        )
        .unwrap();
        assert_eq!(branch.click_anchor, ClickAnchor::Center);
        assert_eq!(branch.click_offset_x, 0);
        assert_eq!(branch.click_offset_y, 0);

        let action: BranchAction = serde_json::from_str(
            r#"{
                "id": 1,
                "name": "old action",
                "kind": "WaitAndClick",
                "template": null,
                "threshold": 0.9,
                "timeout_secs": 60,
                "delay_ms": 500
            }"#,
        )
        .unwrap();
        assert_eq!(action.click_anchor, ClickAnchor::Center);
        assert_eq!(action.click_offset_x, 0);
        assert!(!action.optional);

        let spec: VisualConditionSpec = serde_json::from_str(
            r#"{"mode": "All", "stable_checks": 2, "outcome": "ClickTemplate"}"#,
        )
        .unwrap();
        assert_eq!(spec.click_anchor, ClickAnchor::Center);
        assert_eq!(spec.click_offset_y, 0);
    }

    #[test]
    fn old_profiles_load_with_default_hotkeys() {
        let mut value = serde_json::to_value(MacroProfile::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("capture_hotkey");
        object.remove("stop_hotkey");
        object.remove("shared_templates");
        object.remove("match_algorithm");
        object.remove("stable_confirm");
        let profile: MacroProfile = serde_json::from_value(value).unwrap();
        assert_eq!(profile.capture_hotkey, "f6");
        assert_eq!(profile.stop_hotkey, "f8");
        assert!(!profile.shared_templates);
        assert_eq!(profile.match_algorithm, MatchAlgorithm::Precise);
        assert!(profile.stable_confirm);
    }

    #[test]
    fn parses_valid_hotkey_pair() {
        let (capture, stop) = parse_hotkeys("f6", "ctrl+f8").unwrap();
        assert_eq!(capture.key, KeyCode::F(6));
        assert!(!capture.ctrl);
        assert_eq!(stop.key, KeyCode::F(8));
        assert!(stop.ctrl);
    }

    #[test]
    fn rejects_identical_hotkeys() {
        let error = parse_hotkeys("ctrl+f6", "CTRL+F6").unwrap_err();
        assert!(error.contains("不能相同"));
    }

    #[test]
    fn rejects_unparseable_hotkeys() {
        let error = parse_hotkeys("printscreen", "f8").unwrap_err();
        assert!(error.contains("截图热键无效"));
        let error = parse_hotkeys("f6", "").unwrap_err();
        assert!(error.contains("停止热键无效"));
    }
}
