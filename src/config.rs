use crate::translate::EngineManager;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            engines: EngineManager::default(),
            default_from: "auto".into(),
            default_to: "zh".into(),
            selection_hotkey: "Ctrl+Shift+T".into(),
        }
    }
}

impl AppConfig {
    /// 配置文件路径
    fn config_path() -> anyhow::Result<PathBuf> {
        let dir = dirs::config_dir()
            .or_else(|| dirs::home_dir())
            .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
        let app_dir = dir.join("eztran");
        std::fs::create_dir_all(&app_dir)?;
        Ok(app_dir.join("config.json"))
    }

    /// 从磁盘加载配置
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: AppConfig = serde_json::from_str(&content)?;
            Ok(config)
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
