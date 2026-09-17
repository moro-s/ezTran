//! 翻译历史模块 — 管理最近 50 条翻译记录（内存存储）。

use std::sync::LazyLock;

/// 单条翻译历史
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// 源文本
    pub source: String,
    /// 译文
    pub translated: String,
    /// 源语言
    pub from: String,
    /// 目标语言
    pub to: String,
    /// 引擎名称
    pub engine: String,
    /// 记录时间戳（Unix 秒）
    #[allow(dead_code)]
    pub timestamp: u64,
}

/// 全局历史列表（最多 50 条，新的在前）
static HISTORY: LazyLock<std::sync::Mutex<Vec<HistoryEntry>>> =
    LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

const MAX_HISTORY: usize = 50;

/// 添加一条翻译记录（超限时移除最旧的）
pub fn add_entry(source: &str, translated: &str, from: &str, to: &str, engine: &str) {
    let entry = HistoryEntry {
        source: source.to_string(),
        translated: translated.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        engine: engine.to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };
    let mut hist = HISTORY.lock().unwrap();
    hist.insert(0, entry);
    if hist.len() > MAX_HISTORY {
        hist.truncate(MAX_HISTORY);
    }
}

/// 获取所有历史记录的快照
pub fn get_all() -> Vec<HistoryEntry> {
    HISTORY.lock().unwrap().clone()
}

/// 清空所有历史
pub fn clear() {
    HISTORY.lock().unwrap().clear();
}
