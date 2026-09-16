use crate::config::AppConfig;
use crate::translate::TranslateResult;

/// 设置页面标签
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsTab {
    General,
    Engines,
    About,
}

impl Default for SettingsTab {
    fn default() -> Self {
        SettingsTab::General
    }
}

/// 应用状态
#[derive(Default)]
pub struct AppState {
    /// 输入翻译 - 源文本
    pub input_text: String,
    /// 输入翻译 - 译文结果
    pub result: Option<Result<TranslateResult, String>>,
    /// 当前选择的引擎 index
    pub engine_index: usize,
    /// 源语言
    pub from_lang: String,
    /// 目标语言
    pub to_lang: String,
    /// 是否正在翻译
    pub translating: bool,
    /// 是否显示设置窗口
    pub show_settings: bool,
    /// 保存配置后的提示消息
    pub toast: Option<String>,
    /// toast 消失时间戳
    pub toast_time: f64,
    /// 配置的可变副本（用于设置页编辑）
    pub config_edit: AppConfig,
    /// 窗口是否置顶
    pub pinned: bool,
    /// 设置窗口当前选中的标签页
    pub settings_tab: SettingsTab,
}

/// 全局状态
pub(crate) static STATE: std::sync::LazyLock<std::sync::Mutex<AppState>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(AppState::default()));

/// 初始化全局状态（程序启动时调用一次）
pub fn init_state(config: AppConfig) {
    let mut state = STATE.lock().unwrap();
    state.config_edit = config.clone();
    state.from_lang = state.config_edit.default_from.clone();
    state.to_lang = state.config_edit.default_to.clone();
    let engine_count = state.config_edit.engines.engines.len();
    if state.engine_index >= engine_count {
        state.engine_index = 0;
    }
}

/// 打开设置窗口
pub fn show_settings_window() {
    STATE.lock().unwrap().show_settings = true;
}

/// 关闭设置窗口
pub fn hide_settings_window() {
    STATE.lock().unwrap().show_settings = false;
}

/// 设置窗口是否应该显示
pub fn is_settings_visible() -> bool {
    STATE.lock().unwrap().show_settings
}
