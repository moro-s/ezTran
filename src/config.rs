use crate::translate::EngineManager;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 应用主题
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum AppTheme {
    #[default]
    Dark,
    Light,
    System,
}

impl AppTheme {
    pub fn label(&self) -> &'static str {
        match self {
            AppTheme::Dark => "深色",
            AppTheme::Light => "浅色",
            AppTheme::System => "跟随系统",
        }
    }

    pub fn all() -> &'static [AppTheme] {
        &[AppTheme::Dark, AppTheme::Light, AppTheme::System]
    }
}

/// 应用字号
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum FontSize {
    Small,
    #[default]
    Standard,
    Large,
}

impl FontSize {
    pub fn label(&self) -> &'static str {
        match self {
            FontSize::Small => "小",
            FontSize::Standard => "标准",
            FontSize::Large => "大",
        }
    }

    pub fn all() -> &'static [FontSize] {
        &[FontSize::Small, FontSize::Standard, FontSize::Large]
    }

    pub fn size(&self) -> f32 {
        match self {
            FontSize::Small => 13.0,
            FontSize::Standard => 15.0,
            FontSize::Large => 17.0,
        }
    }
}

/// 应用全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 引擎管理
    pub engines: EngineManager,
    /// 默认源语言
    pub default_from: String,
    /// 默认目标语言
    pub default_to: String,
    /// 划词翻译快捷键
    pub selection_hotkey: String,
    /// 输入翻译快捷键
    pub input_hotkey: String,
    /// 应用主题
    pub theme: AppTheme,
    /// 应用字号
    pub font_size: FontSize,
    /// 是否启用输入翻译
    pub enable_input_translate: bool,
    /// 是否启用划词翻译
    pub enable_selection_translate: bool,
    /// 是否开机自启动
    pub auto_start: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            engines: EngineManager::default(),
            default_from: "auto".into(),
            default_to: "zh".into(),
            selection_hotkey: "Ctrl+Shift+T".into(),
            input_hotkey: "Ctrl+Shift+I".into(),
            theme: AppTheme::default(),
            font_size: FontSize::default(),
            enable_input_translate: true,
            enable_selection_translate: true,
            auto_start: false,
        }
    }
}

impl AppConfig {
    /// 配置文件路径
    fn config_path() -> anyhow::Result<PathBuf> {
        let dir = dirs::config_dir()
            .or_else(dirs::home_dir)
            .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
        let app_dir = dir.join("eztran");
        std::fs::create_dir_all(&app_dir)?;
        Ok(app_dir.join("config.json"))
    }

    /// 从磁盘加载配置（旧格式不兼容，直接用默认覆盖）
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            match serde_json::from_str::<AppConfig>(&content) {
                Ok(config) => Ok(config),
                Err(_) => {
                    let config = AppConfig::default();
                    config.save()?;
                    Ok(config)
                }
            }
        } else {
            let config = AppConfig::default();
            config.save()?;
            Ok(config)
        }
    }

    /// 保存配置到磁盘
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

/// 常用语言列表
pub const LANGUAGES: &[(&str, &str)] = &[
    ("auto", "自动检测"),
    ("zh", "中文"),
    ("en", "英语"),
    ("ja", "日语"),
    ("ko", "韩语"),
    ("fr", "法语"),
    ("de", "德语"),
    ("es", "西班牙语"),
    ("ru", "俄语"),
    ("pt", "葡萄牙语"),
    ("it", "意大利语"),
    ("th", "泰语"),
    ("vi", "越南语"),
    ("ar", "阿拉伯语"),
];
