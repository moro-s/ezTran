mod app;
mod config;
mod font_list;
mod icon;
mod theme;
mod translate;
mod ui;

use app::EzTranApp;
use config::AppConfig;
use icon::create_window_icon;

fn main() -> eframe::Result {
    let config = AppConfig::load().unwrap_or_default();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([820.0, 520.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title("EzTran - 翻译")
            .with_icon(std::sync::Arc::new(create_window_icon())),
        ..Default::default()
    };
    eframe::run_native(
        "EzTran",
        options,
        Box::new(move |_cc| Ok(Box::new(EzTranApp::new(config)))),
    )
}
