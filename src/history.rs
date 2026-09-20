//! 翻译历史模块 — 管理最近 50 条翻译记录，持久化到磁盘。

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// 单条翻译历史
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub timestamp: u64,
}

/// 全局历史列表（最多 50 条，新的在前）
static HISTORY: LazyLock<std::sync::Mutex<Vec<HistoryEntry>>> =
    LazyLock::new(|| std::sync::Mutex::new(load_from_disk()));

const MAX_HISTORY: usize = 50;

/// 历史文件路径（与 config.json 同目录）
fn history_path() -> Option<std::path::PathBuf> {
    let dir = dirs::config_dir().or_else(dirs::home_dir)?;
    let app_dir = dir.join("eztran");
    let _ = std::fs::create_dir_all(&app_dir);
    Some(app_dir.join("history.json"))
}

/// 从磁盘加载历史记录（文件不存在或解析失败时返回空列表）
fn load_from_disk() -> Vec<HistoryEntry> {
    let path = match history_path() {
        Some(p) => p,
        None => return Vec::new(),
    };
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) => {
            log::warn!("[history] 加载历史记录失败: {}", e);
            Vec::new()
        }
    }
}

/// 将历史记录保存到磁盘（原子写入：先写临时文件再 rename）
fn save_to_disk(history: &[HistoryEntry]) {
    let path = match history_path() {
        Some(p) => p,
        None => {
            log::warn!("[history] 无法获取历史文件路径");
            return;
        }
    };
    match serde_json::to_string_pretty(history) {
        Ok(content) => {
            let tmp_path = path.with_extension("json.tmp");
            if let Err(e) = std::fs::write(&tmp_path, &content) {
                log::warn!("[history] 写入临时文件失败: {}", e);
                return;
            }
            if let Err(e) = std::fs::rename(&tmp_path, &path) {
                // rename 失败时尝试直接写入作为兜底
                log::warn!("[history] rename 失败: {}, 尝试直接写入", e);
                let _ = std::fs::write(&path, &content);
            }
        }
        Err(e) => log::warn!("[history] 序列化历史记录失败: {}", e),
    }
}

/// 添加一条翻译记录（超限时移除最旧的），并自动持久化
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
    // 锁内只改内存，锁外做磁盘 IO（避免持久化期间阻塞 get_all 等读取）
    let snapshot = {
        let mut hist = HISTORY.lock().unwrap();
        hist.insert(0, entry);
        if hist.len() > MAX_HISTORY {
            hist.truncate(MAX_HISTORY);
        }
        hist.clone()
    };
    save_to_disk(&snapshot);
}

/// 获取所有历史记录的快照
pub fn get_all() -> Vec<HistoryEntry> {
    HISTORY.lock().unwrap().clone()
}

/// 清空所有历史，并删除磁盘文件
pub fn clear() {
    {
        let mut hist = HISTORY.lock().unwrap();
        hist.clear();
    }
    save_to_disk(&[]);
}
