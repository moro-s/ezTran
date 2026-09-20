#![windows_subsystem = "windows"]

mod app;
mod autostart;
mod config;
mod history;
mod hotkey;
mod icon;
mod theme;
mod translate;
mod ui;
mod window;

use app::EzTranApp;
use config::AppConfig;
use icon::create_window_icon;
use simplelog::{ConfigBuilder, LevelFilter, WriteLogger};
use std::fs::{self, OpenOptions};

/// 日志保留天数
const LOG_MAX_DAYS: i64 = 7;

/// 初始化日志，按天存储到 exe 同目录下的 logs/ 子目录，最多保留 7 天
fn init_logger() {
    let log_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("logs")))
        .unwrap_or_else(|| std::path::PathBuf::from("logs"));

    // 确保日志目录存在
    if fs::create_dir_all(&log_dir).is_err() {
        return;
    }

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let log_path = log_dir.join(format!("eztran_{}.log", today));

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path);

    if let Ok(file) = file {
        // 只记录 eztran 自身模块的日志，过滤 eframe/egui 等第三方库噪音
        let config = ConfigBuilder::new()
            .set_target_level(LevelFilter::Off)
            .set_max_level(LevelFilter::Info)
            .add_filter_allow_str("eztran")
            .build();
        let _ = WriteLogger::init(LevelFilter::Trace, config, file);
        log::info!("========== EzTran 启动 ==========");
        log::info!("日志文件: {}", log_path.display());

        // 清理超过 7 天的旧日志（在 logger 初始化后执行，确保清理日志可记录）
        clean_old_logs(&log_dir);
    }
}

/// 删除日志目录中超过 LOG_MAX_DAYS 天的日志文件
fn clean_old_logs(log_dir: &std::path::Path) {
    let cutoff = chrono::Local::now().date_naive() - chrono::Duration::days(LOG_MAX_DAYS);
    if let Ok(entries) = fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("log") {
                continue;
            }
            // 从文件名 eztran_YYYY-MM-DD.log 提取日期
            let name = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => continue,
            };
            let date_str = match name.rsplit_once('_') {
                Some((_, d)) => d,
                None => continue,
            };
            if let Ok(file_date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                if file_date < cutoff {
                    let _ = fs::remove_file(&path);
                    log::info!("[log] 清理过期日志: {}", path.display());
                }
            }
        }
    }
}

fn main() -> eframe::Result {
    init_logger();

    let config = AppConfig::load().unwrap_or_default();

    // 启动时同步开机自启动注册表项与配置一致
    sync_autostart_on_startup(&config);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([820.0, 520.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title("EzTran - 翻译")
            .with_decorations(false)
            .with_icon(std::sync::Arc::new(create_window_icon())),
        ..Default::default()
    };
    eframe::run_native(
        "EzTran",
        options,
        Box::new(move |_cc| Ok(Box::new(EzTranApp::new(config)))),
    )
}

/// 启动时同步开机自启动注册表项与配置一致
/// （配置说开但注册表没开 → 补开；配置说关但注册表残留 → 清除）
fn sync_autostart_on_startup(config: &AppConfig) {
    let reg_enabled = autostart::is_enabled();
    if config.auto_start && !reg_enabled {
        log::info!("[main] 配置已启用开机自启动但注册表缺失，补写注册表");
        let _ = autostart::enable();
    } else if !config.auto_start && reg_enabled {
        log::info!("[main] 配置已禁用开机自启动但注册表残留，清除注册表");
        let _ = autostart::disable();
    }
}
