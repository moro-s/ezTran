use crate::config::AppConfig;
use crate::icon;
use crate::theme;
use crate::ui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

/// 全局 egui Context，用于在托盘事件中唤醒事件循环
static EGUI_CTX: std::sync::LazyLock<std::sync::Mutex<Option<egui::Context>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// 全局菜单事件队列，由 set_event_handler 推入，update 中消费
static PENDING_MENU_EVENTS: std::sync::LazyLock<std::sync::Mutex<Vec<MenuEvent>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

/// 托盘图标点击产生的动作
enum TrayAction {
    ShowTranslate,
    ShowSettings,
}

static PENDING_TRAY_ACTIONS: std::sync::LazyLock<std::sync::Mutex<Vec<TrayAction>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

/// 上次单击时间（用于区分单击/双击）
static LAST_CLICK_TIME: std::sync::LazyLock<std::sync::Mutex<Option<Instant>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// 是否有待确认的单击（延迟窗口内未收到双击则执行）
static PENDING_SINGLE_CLICK: AtomicBool = AtomicBool::new(false);

/// 下一帧需要隐藏窗口（最小化到托盘）
static PENDING_HIDE: AtomicBool = AtomicBool::new(false);

/// 圆角是否已设置（仅执行一次）
#[cfg(windows)]
static ROUND_CORNERS_SET: AtomicBool = AtomicBool::new(false);

const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(350);

fn wake_event_loop() {
    if let Some(ctx) = EGUI_CTX.lock().unwrap().as_ref() {
        ctx.request_repaint();
    }
}

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

        // 托盘菜单事件
        MenuEvent::set_event_handler(Some(|event: MenuEvent| {
            PENDING_MENU_EVENTS.lock().unwrap().push(event);
            wake_event_loop();
        }));

        // 托盘图标点击事件：单击 → 翻译工作台，双击 → 设置
        TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
            match event {
                TrayIconEvent::Click { .. } => {
                    let now = Instant::now();
                    let mut last = LAST_CLICK_TIME.lock().unwrap();
                    if let Some(prev) = *last {
                        if now.duration_since(prev) < DOUBLE_CLICK_THRESHOLD {
                            *last = Some(now);
                            return;
                        }
                    }
                    *last = Some(now);
                    PENDING_SINGLE_CLICK.store(true, Ordering::SeqCst);
                    std::thread::spawn(|| {
                        std::thread::sleep(DOUBLE_CLICK_THRESHOLD);
                        if PENDING_SINGLE_CLICK.swap(false, Ordering::SeqCst) {
                            PENDING_TRAY_ACTIONS
                                .lock()
                                .unwrap()
                                .push(TrayAction::ShowTranslate);
                            wake_event_loop();
                        }
                    });
                }
                TrayIconEvent::DoubleClick { .. } => {
                    PENDING_SINGLE_CLICK.store(false, Ordering::SeqCst);
                    PENDING_TRAY_ACTIONS
                        .lock()
                        .unwrap()
                        .push(TrayAction::ShowSettings);
                    wake_event_loop();
                }
                _ => {}
            }
        }));

        start_repaint_thread();

        EzTranApp {
            _tray: tray,
            translate_id,
            settings_id,
            quit_id,
            should_quit: false,
        }
    }
}

/// 后台守护线程：定期 request_repaint，防止窗口最小化/隐藏后事件循环暂停
fn start_repaint_thread() {
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Some(ctx) = EGUI_CTX.lock().unwrap().as_ref() {
                ctx.request_repaint();
            }
        }
    });
}

impl eframe::App for EzTranApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        *EGUI_CTX.lock().unwrap() = Some(ctx.clone());

        // Windows 圆角（仅一次）
        #[cfg(windows)]
        try_set_round_corners();

        theme::setup_fonts(ctx);
        theme::setup_style(ctx);

        // 1. 处理托盘图标点击动作（单击/双击）
        let tray_actions: Vec<TrayAction> =
            PENDING_TRAY_ACTIONS.lock().unwrap().drain(..).collect();
        for action in tray_actions {
            match action {
                TrayAction::ShowTranslate => show_window(ctx),
                TrayAction::ShowSettings => ui::show_settings_window(),
            }
        }

        // 2. 处理托盘菜单事件
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

        // 3. 主窗口关闭按钮 → 最小化到托盘
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            PENDING_HIDE.store(true, Ordering::SeqCst);
        }

        if PENDING_HIDE.load(Ordering::SeqCst) {
            PENDING_HIDE.store(false, Ordering::SeqCst);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            return;
        }

        // 4. 绘制自绘标题栏
        draw_titlebar(ctx);

        // 5. 绘制翻译工作台
        ui::draw_translate(ctx);

        // 6. 设置窗口（主窗口内的浮动面板）
        if ui::is_settings_visible() {
            ui::draw_settings(ctx);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self._tray.take();
    }
}

fn show_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
}

/// Windows: 按标题查找窗口句柄
#[cfg(windows)]
fn find_hwnd() -> *mut std::ffi::c_void {
    use std::ffi::c_void;
    extern "system" {
        fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> *mut c_void;
    }
    let title: Vec<u16> = "EzTran - 翻译\0".encode_utf16().collect();
    unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) }
}

/// Windows 11: 通过 DWM API 设置窗口圆角（仅执行一次）
#[cfg(windows)]
fn try_set_round_corners() {
    use std::ffi::c_void;
    if ROUND_CORNERS_SET.swap(true, Ordering::SeqCst) {
        return;
    }
    extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: *mut c_void,
            attr: u32,
            value: *const c_void,
            size: u32,
        ) -> i32;
    }
    unsafe {
        let hwnd = find_hwnd();
        if !hwnd.is_null() {
            let pref: i32 = 2; // DWMWCP_ROUND
            DwmSetWindowAttribute(
                hwnd,
                33, // DWMWA_WINDOW_CORNER_PREFERENCE
                &pref as *const i32 as *const c_void,
                std::mem::size_of::<i32>() as u32,
            );
        }
    }
}

/// Windows: 通过 WM_NCLBUTTONDOWN 让系统接管窗口拖动（无抖动）
#[cfg(windows)]
fn start_drag_window() {
    use std::ffi::c_void;
    const WM_NCLBUTTONDOWN: u32 = 0xA1;
    const HTCAPTION: usize = 2;
    extern "system" {
        fn SendMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn ReleaseCapture() -> i32;
    }
    unsafe {
        let hwnd = find_hwnd();
        if !hwnd.is_null() {
            ReleaseCapture();
            SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION, 0);
        }
    }
}

/// 绘制自绘标题栏（最小化 / 最大化 / 关闭）
fn draw_titlebar(ctx: &egui::Context) {
    let titlebar_bg = egui::Color32::from_rgb(37, 37, 38);
    egui::TopBottomPanel::top("custom_titlebar")
        .exact_height(34.0)
        .frame(
            egui::Frame::default()
                .fill(titlebar_bg)
                .inner_margin(egui::Margin::same(0.0))
                .rounding(egui::Rounding {
                    nw: 8.0,
                    ne: 8.0,
                    sw: 0.0,
                    se: 0.0,
                }),
        )
        .show(ctx, |ui| {
            ui.set_min_height(34.0);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

            ui.horizontal(|ui| {
                // ── 左侧标题 ──
                ui.add_space(12.0);
                let title_resp = ui.label(
                    egui::RichText::new("EzTran - 翻译")
                        .color(egui::Color32::from_gray(200))
                        .strong(),
                );

                // 标题区域可拖动
                let title_drag = ui.interact(
                    title_resp.rect,
                    ui.id().with("title_drag"),
                    egui::Sense::drag(),
                );

                // ── 中间空白（可拖动）──
                let btn_w = 46.0 * 3.0;
                let drag_w = (ui.available_width() - btn_w).max(0.0);
                let drag_resp = ui.allocate_response(
                    egui::vec2(drag_w, 34.0),
                    egui::Sense::drag(),
                );

                // 处理窗口拖动 — 拖动开始时交由系统接管，避免逐帧位移抖动
                for resp in [title_drag, drag_resp] {
                    if resp.drag_started() {
                        #[cfg(windows)]
                        start_drag_window();
                    }
                }

                // ── 右侧按钮 ──
                // 最小化
                let min_resp = titlebar_button(ui, "\u{2014}", egui::Color32::from_rgb(60, 60, 60));
                if min_resp.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }

                // 最大化/还原
                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                let max_icon = if maximized { "\u{2750}" } else { "\u{25A2}" };
                let max_resp = titlebar_button(ui, max_icon, egui::Color32::from_rgb(60, 60, 60));
                if max_resp.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                }

                // 关闭
                let close_resp = titlebar_button(ui, "\u{2715}", egui::Color32::from_rgb(232, 17, 35));
                if close_resp.clicked() {
                    PENDING_HIDE.store(true, Ordering::SeqCst);
                }
            });
        });
}

/// 标题栏按钮（透明背景，hover 时变色）
fn titlebar_button(
    ui: &mut egui::Ui,
    icon: &str,
    hover_color: egui::Color32,
) -> egui::Response {
    let resp = ui.allocate_response(egui::vec2(46.0, 34.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(resp.rect, 0.0, hover_color);
    }
    ui.painter().text(
        resp.rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(14.0),
        egui::Color32::from_gray(200),
    );
    resp
}
