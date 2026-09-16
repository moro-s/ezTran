use crate::config::AppConfig;
use crate::icon;
use crate::theme;
use crate::ui;
use std::sync::atomic::{AtomicBool, Ordering};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

/// 全局 egui Context，用于在托盘菜单事件中唤醒事件循环
static EGUI_CTX: std::sync::LazyLock<std::sync::Mutex<Option<egui::Context>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// 全局菜单事件队列，由 set_event_handler 推入，update 中消费
static PENDING_MENU_EVENTS: std::sync::LazyLock<std::sync::Mutex<Vec<MenuEvent>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

/// 主窗口是否已隐藏（关闭按钮隐藏到托盘）
static WINDOW_HIDDEN: AtomicBool = AtomicBool::new(false);

pub struct EzTranApp {
    _tray: Option<TrayIcon>,
    translate_id: tray_icon::menu::MenuId,
    settings_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
    should_quit: bool,
}

impl EzTranApp {
    pub fn new(config: AppConfig) -> Self {
        ui::init_state(config);

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

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("EzTran - 轻量翻译工具")
            .with_icon(icon::create_tray_icon())
            .build()
            .ok();

        // 设置托盘菜单事件处理器
        let quit_id_clone = quit_id.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let is_quit = event.id == quit_id_clone;
            PENDING_MENU_EVENTS.lock().unwrap().push(event);
            if let Some(ctx) = EGUI_CTX.lock().unwrap().clone() {
                if is_quit {
                    ctx.request_repaint();
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.request_repaint();
                }
            }
        }));

        // 后台守护线程：窗口隐藏时定期 request_repaint，防止 eframe 事件循环暂停后无法唤醒
        std::thread::spawn(|| {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(200));
                if WINDOW_HIDDEN.load(Ordering::SeqCst) {
                    if let Some(ctx) = EGUI_CTX.lock().unwrap().as_ref() {
                        ctx.request_repaint();
                    }
                }
            }
        });

        EzTranApp {
            _tray: tray,
            translate_id,
            settings_id,
            quit_id,
            should_quit: false,
        }
    }
}

impl eframe::App for EzTranApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        *EGUI_CTX.lock().unwrap() = Some(ctx.clone());

        theme::setup_fonts(ctx);
        theme::setup_style(ctx);

        // 1. 处理托盘菜单事件
        let events: Vec<MenuEvent> = PENDING_MENU_EVENTS.lock().unwrap().drain(..).collect();
        for event in events {
            if event.id == self.translate_id {
                show_window(ctx);
            } else if event.id == self.settings_id {
                ui::show_settings_window();
            } else if event.id == self.quit_id {
                self.should_quit = true;
            }
        }

        if self.should_quit {
            self._tray.take();
            std::process::exit(0);
        }

        // 2. 主窗口关闭按钮 → 隐藏到托盘
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            WINDOW_HIDDEN.store(true, Ordering::SeqCst);
            return;
        }

        // 3. 绘制翻译工作台
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
    WINDOW_HIDDEN.store(false, Ordering::SeqCst);
}
