mod config;
mod translate;
mod ui;

use config::AppConfig;
use std::sync::Arc;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

struct EzTranApp {
    config: Arc<AppConfig>,
    _tray: Option<TrayIcon>,
    translate_id: tray_icon::menu::MenuId,
    settings_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
    should_quit: bool,
}

impl EzTranApp {
    fn new(config: AppConfig) -> Self {
        // 构建托盘菜单：翻译 / 设置 / 分隔线 / 退出
        let menu = Menu::new();
        let translate_item = MenuItem::new("翻译", true, None);
        let settings_item = MenuItem::new("设置", true, None);
        let quit_item = MenuItem::new("退出", true, None);

        let _ = menu.append(&translate_item);
        let _ = menu.append(&settings_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit_item);

        let translate_id = translate_item.id().clone();
        let settings_id = settings_item.id().clone();
        let quit_id = quit_item.id().clone();

        let icon = create_tray_icon();
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("EzTran - 轻量翻译工具")
            .with_icon(icon)
            .build()
            .ok();

        EzTranApp {
            config: Arc::new(config),
            _tray: tray,
            translate_id,
            settings_id,
            quit_id,
            should_quit: false,
        }
    }
}

/// 生成一个简单的纯色托盘图标
fn create_tray_icon() -> Icon {
    let (w, h) = (32u32, 32u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            // 蓝色方块 + 白色 "T" 形
            let cx = w as f32 / 2.0;
            let cy = h as f32 / 2.0;
            let dx = (x as f32 - cx).abs();
            let dy = (y as f32 - cy).abs();

            let is_t = (dy < 4.0 && dx < 10.0) || (dx < 3.0 && dy < 12.0);
            if is_t {
                rgba.extend_from_slice(&[245, 245, 245, 255]); // 白色 T
            } else {
                rgba.extend_from_slice(&[137, 180, 250, 255]); // 蓝色背景
            }
        }
    }
    Icon::from_rgba(rgba, w, h).unwrap_or_else(|_| {
        Icon::from_rgba(vec![137, 180, 250, 255], 1, 1).unwrap()
    })
}

impl eframe::App for EzTranApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        setup_fonts(ctx);

        // 1. 处理托盘菜单事件
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.translate_id {
                ui::switch_to_translate();
                show_window(ctx);
            } else if event.id == self.settings_id {
                ui::switch_to_settings();
                show_window(ctx);
            } else if event.id == self.quit_id {
                self.should_quit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        // 2. 主动退出时不拦截关闭
        if self.should_quit {
            return;
        }

        // 3. 拦截窗口关闭按钮 → 隐藏到托盘
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        // 4. 绘制 UI
        let config = self.config.clone();
        ui::draw(ctx, config);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // 退出时销毁托盘
        self._tray.take();
    }
}

fn show_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
}

fn main() -> eframe::Result {
    let config = AppConfig::load().unwrap_or_default();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([820.0, 520.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title("EzTran"),
        ..Default::default()
    };

    eframe::run_native(
        "EzTran",
        options,
        Box::new(move |_cc| Ok(Box::new(EzTranApp::new(config)))),
    )
}

/// 加载系统中文字体并注入 egui，解决中文显示为方框的问题
fn setup_fonts(ctx: &egui::Context) {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut fonts = egui::FontDefinitions::default();

        let candidates: &[&str] = &[
            "C:\\Windows\\Fonts\\simhei.ttf",
            "C:\\Windows\\Fonts\\Deng.ttf",
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\simsun.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Medium.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/wqy-microhei/wqy-microhei.ttc",
        ];

        for path in candidates {
            if let Ok(data) = std::fs::read(path) {
                if !is_valid_font(&data) {
                    continue;
                }
                fonts.font_data.insert(
                    "chinese".to_owned(),
                    egui::FontData::from_owned(data),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .push("chinese".to_owned());
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .push("chinese".to_owned());
                break;
            }
        }

        ctx.set_fonts(fonts);
    });
}

fn is_valid_font(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    let magic = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    matches!(
        magic,
        0x00010000 | 0x4F54544F | 0x74727565 | 0x74797031
    )
}
