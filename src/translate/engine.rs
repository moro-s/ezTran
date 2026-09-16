use serde::{Deserialize, Serialize};

/// 支持的翻译引擎类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EngineKind {
    /// 有道翻译
    Youdao,
    /// 百度翻译
    Baidu,
    /// DeepL
    DeepL,
    /// 自定义 API（兼容 OpenAI 格式等）
    Custom,
}

impl Default for EngineKind {
    fn default() -> Self {
        EngineKind::Youdao
    }
}

impl EngineKind {
    pub fn label(&self) -> &'static str {
        match self {
            EngineKind::Youdao => "有道翻译",
            EngineKind::Baidu => "百度翻译",
            EngineKind::DeepL => "DeepL",
            EngineKind::Custom => "自定义",
        }
    }

    pub fn all() -> &'static [EngineKind] {
        &[
            EngineKind::Youdao,
            EngineKind::Baidu,
            EngineKind::DeepL,
            EngineKind::Custom,
        ]
    }
}

/// 单个引擎的配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub kind: EngineKind,
    pub name: String,
    pub enabled: bool,
    /// API Key / App Key
    pub api_key: String,
    /// API Secret / App Secret
    pub api_secret: String,
    /// 自定义 API 的 URL
    pub endpoint: String,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            kind: EngineKind::Youdao,
            name: "有道翻译".into(),
            enabled: true,
            api_key: String::new(),
            api_secret: String::new(),
            endpoint: String::new(),
        }
    }
}

/// 引擎管理器：管理多个引擎配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineManager {
    pub engines: Vec<EngineConfig>,
    /// 默认使用的引擎 index
    pub default_index: usize,
}

impl Default for EngineManager {
    fn default() -> Self {
        EngineManager {
            engines: vec![EngineConfig::default()],
            default_index: 0,
        }
    }
}

impl EngineManager {
    pub fn default_engine(&self) -> Option<&EngineConfig> {
        self.engines.get(self.default_index)
    }

    pub fn enabled_engines(&self) -> Vec<&EngineConfig> {
        self.engines.iter().filter(|e| e.enabled).collect()
    }
}
