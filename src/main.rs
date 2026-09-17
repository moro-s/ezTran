mod app;
mod config;
mod font_list;
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
use std::fs::OpenOptions;

/// 初始化日志，输出到 exe 同目录下的 eztran.log
fn init_logger() {
    let log_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("eztran.log")))
        .unwrap_or_else(|| std::path::PathBuf::from("eztran.log"));

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
    }
}

fn main() -> eframe::Result {
    init_logger();

    let config = AppConfig::load().unwrap_or_default();
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
