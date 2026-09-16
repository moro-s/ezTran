mod config;
mod translate;
mod ui;

use config::AppConfig;
use std::sync::Arc;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// 全局 egui Context，用于在托盘菜单事件中唤醒事件循环
static EGUI_CTX: std::sync::LazyLock<std::sync::Mutex<Option<egui::Context>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// 全局菜单事件队列，由 set_event_handler 推入，update 中消费
static PENDING_MENU_EVENTS: std::sync::LazyLock<std::sync::Mutex<Vec<MenuEvent>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

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
        // 初始化全局状态
        ui::init_state(config.clone());

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

        // 设置托盘菜单事件处理器：收到菜单事件时存入队列并唤醒 egui 事件循环
        MenuEvent::set_event_handler(Some(|event: MenuEvent| {
            PENDING_MENU_EVENTS.lock().unwrap().push(event);
            if let Some(ctx) = EGUI_CTX.lock().unwrap().clone() {
                ctx.request_repaint();
            }
        }));

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

/// 生成图标 RGBA 数据（圆角蓝底 + 白色 T 字）
fn create_icon_rgba() -> (Vec<u8>, u32, u32) {
    let (w, h) = (64u32, 64u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);

    // 圆角半径
    let radius = 14.0_f32;
    // T 字参数（以 64x64 画布为基准）
    // 横杠：y 范围 16~26，x 范围 16~48
    let bar_y_min = 16.0;
    let bar_y_max = 26.0;
    let bar_x_min = 16.0;
    let bar_x_max = 48.0;
    // 竖杠：x 范围 28~36，y 范围 26~50
    let stem_x_min = 28.0;
    let stem_x_max = 36.0;
    let stem_y_min = 26.0;
    let stem_y_max = 50.0;

    for y in 0..h {
        for x in 0..w {
            let fx = x as f32;
            let fy = y as f32;

            // 圆角矩形判定
            let in_bg = {
                let dx = (fx - w as f32 / 2.0).abs();
                let dy = (fy - h as f32 / 2.0).abs();
                let half_w = w as f32 / 2.0;
                let half_h = h as f32 / 2.0;
                // 距圆角中心的距离
                let corner_dx = (dx - (half_w - radius)).max(0.0);
                let corner_dy = (dy - (half_h - radius)).max(0.0);
                corner_dx * corner_dx + corner_dy * corner_dy <= radius * radius
                    || dx <= half_w - radius
                    || dy <= half_h - radius
            };

            // T 字判定
            let in_t = (fy >= bar_y_min && fy <= bar_y_max && fx >= bar_x_min && fx <= bar_x_max)
                || (fx >= stem_x_min && fx <= stem_x_max && fy >= stem_y_min && fy <= stem_y_max);

            if in_bg && in_t {
                // T 字区域：白色
                rgba.extend_from_slice(&[245, 245, 245, 255]);
            } else if in_bg {
                // 背景：黑色
                rgba.extend_from_slice(&[30, 30, 30, 255]);
            } else {
                // 透明
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    (rgba, w, h)
}

/// 生成托盘图标（蓝色背景 + 白色 T）
fn create_tray_icon() -> Icon {
    let (rgba, w, h) = create_icon_rgba();
    Icon::from_rgba(rgba, w, h)
        .unwrap_or_else(|_| Icon::from_rgba(vec![137, 180, 250, 255], 1, 1).unwrap())
}

/// 生成窗口图标数据（与托盘图标一致）
fn create_window_icon() -> egui::IconData {
    let (rgba, w, h) = create_icon_rgba();
    egui::IconData {
        rgba: rgba.clone(),
        width: w,
        height: h,
    }
}

impl eframe::App for EzTranApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 保存 ctx 供托盘菜单事件唤醒
        {
            *EGUI_CTX.lock().unwrap() = Some(ctx.clone());
        }

        setup_fonts(ctx);
        setup_style(ctx);

        // 1. 处理托盘菜单事件
        let events: Vec<MenuEvent> = PENDING_MENU_EVENTS.lock().unwrap().drain(..).collect();
        for event in events {
            if event.id == self.translate_id {
                show_window(ctx);
            } else if event.id == self.settings_id {
                ui::show_settings_window();
            } else if event.id == self.quit_id {
                self.should_quit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        if self.should_quit {
            return;
        }

        // 2. 主窗口关闭按钮 → 隐藏到托盘（不退出程序）
        //    只有托盘菜单"退出"才会设置 should_quit 并真正退出
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }

        // 3. 绘制翻译工作台（主窗口）
        ui::draw_translate(ctx);

        // 4. 设置窗口（独立视口）
        if ui::is_settings_visible() {
            ctx.show_viewport_deferred(
                egui::ViewportId::from_hash_of("settings"),
                egui::ViewportBuilder::default()
                    .with_title("EzTran - 设置")
                    .with_inner_size([680.0, 520.0])
                    .with_min_inner_size([500.0, 400.0]),
                |ctx, _| {
                    // 设置窗口关闭 → 隐藏，不退出程序（子 viewport 无需 CancelClose）
                    if ctx.input(|i| i.viewport().close_requested()) {
                        ui::hide_settings_window();
                        return;
                    }
                    ui::draw_settings(ctx);
                },
            );
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
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

/// 加载系统中文字体并注入 egui
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

        // 加载系统符号字体，用于显示箭头等 Unicode 符号（如 ⇄）
        let symbol_candidates: &[&str] = &[
            "C:\\Windows\\Fonts\\seguisym.ttf",
            "C:\\Windows\\Fonts\\segoeui.ttf",
        ];
        for path in symbol_candidates {
            if let Ok(data) = std::fs::read(path) {
                if !is_valid_font(&data) {
                    continue;
                }
                fonts.font_data.insert(
                    "symbols".to_owned(),
                    egui::FontData::from_owned(data),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .push("symbols".to_owned());
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .push("symbols".to_owned());
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

/// 设置 egui 深色主题配色
fn setup_style(ctx: &egui::Context) {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut style = (*ctx.style()).clone();
        // 紧凑的间距
        style.spacing.item_spacing = egui::vec2(6.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.window_margin = egui::Margin::same(0.0);
        ctx.set_style(style);

        let mut vis = egui::Visuals::dark();
        // 主背景 #2B2B2B
        vis.panel_fill = egui::Color32::from_rgb(43, 43, 43);
        // 窗口/面板背景略深
        vis.window_fill = egui::Color32::from_rgb(30, 30, 30);
        // 控件背景 #3C3C3C
        vis.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(43, 43, 43);
        vis.widgets.inactive.bg_fill = egui::Color32::from_rgb(60, 60, 60);
        vis.widgets.hovered.bg_fill = egui::Color32::from_rgb(76, 76, 76);
        vis.widgets.active.bg_fill = egui::Color32::from_rgb(80, 80, 80);
        // 文字颜色
        vis.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(224, 224, 224));
        vis.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(224, 224, 224));
        // 边框
        vis.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT);
        vis.widgets.inactive.bg_stroke = egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT);
        // 选中/高亮色 — 蓝色
        vis.selection.bg_fill = egui::Color32::from_rgb(137, 180, 250);
        vis.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(137, 180, 250));
        // 超链接色
        vis.hyperlink_color = egui::Color32::from_rgb(137, 180, 250);
        ctx.set_visuals(vis);
    });
}
