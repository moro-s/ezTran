use crate::config::AppConfig;
use crate::icon;
use crate::theme;
use crate::ui;
use std::sync::atomic::{AtomicBool, Ordering};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};
static EGUI_CTX: std::sync::LazyLock<std::sync::Mutex<Option<egui::Context>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// 全局菜单事件队列，由 set_event_handler 推入，update 中消费
static PENDING_MENU_EVENTS: std::sync::LazyLock<std::sync::Mutex<Vec<MenuEvent>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

/// 托盘图标点击产生的动作
enum TrayAction {
    ShowTranslate,
}

static PENDING_TRAY_ACTIONS: std::sync::LazyLock<std::sync::Mutex<Vec<TrayAction>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

/// 下一帧需要隐藏窗口
static PENDING_HIDE: AtomicBool = AtomicBool::new(false);

/// 窗口当前是否已隐藏
static WINDOW_HIDDEN: AtomicBool = AtomicBool::new(false);

/// 窗口是否置顶
static STATE_PINNED: AtomicBool = AtomicBool::new(false);

/// 圆角是否已设置（仅执行一次）
#[cfg(windows)]
static ROUND_CORNERS_SET: AtomicBool = AtomicBool::new(false);

fn wake_event_loop() {
    if let Some(ctx) = EGUI_CTX.lock().unwrap().as_ref() {
        ctx.request_repaint();
    }
    #[cfg(windows)]
    post_wakeup();
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
            #[cfg(windows)]
            restore_window_if_hidden();
            wake_event_loop();
        }));

        // 托盘图标点击事件：双击 → 恢复窗口并显示翻译工作台
        TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
            if let TrayIconEvent::DoubleClick { .. } = event {
                #[cfg(windows)]
                restore_window_if_hidden();
                PENDING_TRAY_ACTIONS
                    .lock()
                    .unwrap()
                    .push(TrayAction::ShowTranslate);
                wake_event_loop();
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

/// 后台守护线程：定期 request_repaint + PostMessage，防止窗口隐藏后事件循环暂停
fn start_repaint_thread() {
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Some(ctx) = EGUI_CTX.lock().unwrap().as_ref() {
                ctx.request_repaint();
            }
            #[cfg(windows)]
            post_wakeup();
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

        // 3. Alt+F4 → 隐藏到托盘（无边框窗口无系统关闭按钮，此为后备）
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if !WINDOW_HIDDEN.load(Ordering::SeqCst) {
                PENDING_HIDE.store(true, Ordering::SeqCst);
            }
        }

        // 4. 设置窗口（独立视口，主窗口隐藏时也需渲染）
        if ui::is_settings_visible() {
            ui::draw_settings(ctx);
        }

        if PENDING_HIDE.load(Ordering::SeqCst) {
            PENDING_HIDE.store(false, Ordering::SeqCst);
            #[cfg(windows)]
            win_show_window(SW_HIDE);
            WINDOW_HIDDEN.store(true, Ordering::SeqCst);
            return;
        }

        // 窗口隐藏后事件循环仍被后台线程唤醒，跳过主窗口绘制
        if WINDOW_HIDDEN.load(Ordering::SeqCst) {
            return;
        }

        // 窗口尺寸未就绪时跳过绘制（首帧或最小化时可用区域为 0）
        let screen = ctx.screen_rect();
        if screen.height() < 80.0 || screen.width() < 10.0 {
            return;
        }

        // 5. 绘制自绘标题栏
        draw_titlebar(ctx);

        // 6. 绘制翻译工作台
        ui::draw_translate(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self._tray.take();
    }
}

fn show_window(_ctx: &egui::Context) {
    #[cfg(windows)]
    {
        win_show_window(SW_RESTORE);
        win_show_window(SW_SHOW);
    }
    WINDOW_HIDDEN.store(false, Ordering::SeqCst);
}

/// 窗口被 SW_HIDE 隐藏后 winit 事件循环暂停，update 不会被调用。
/// 托盘事件中先用 Win32 API 直接恢复窗口，绕过事件循环。
#[cfg(windows)]
fn restore_window_if_hidden() {
    if WINDOW_HIDDEN.swap(false, Ordering::SeqCst) {
        win_show_window(SW_RESTORE);
        win_show_window(SW_SHOW);
    }
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

/// Windows: 向窗口发送 WM_NULL 唤醒 winit 事件循环（窗口隐藏后 request_repaint 无效）
#[cfg(windows)]
fn post_wakeup() {
    use std::ffi::c_void;
    const WM_NULL: u32 = 0x0000;
    extern "system" {
        fn PostMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> i32;
    }
    unsafe {
        let hwnd = find_hwnd();
        if !hwnd.is_null() {
            PostMessageW(hwnd, WM_NULL, 0, 0);
        }
    }
}

/// Windows: 通过 ShowWindow 直接控制窗口显示状态
/// 无边框窗口下 eframe 的 ViewportCommand 不可靠，直接用 Win32 API
#[cfg(windows)]
fn win_show_window(cmd: i32) {
    use std::ffi::c_void;
    extern "system" {
        fn ShowWindow(hwnd: *mut c_void, cmd: i32) -> i32;
    }
    unsafe {
        let hwnd = find_hwnd();
        if !hwnd.is_null() {
            ShowWindow(hwnd, cmd);
        }
    }
}

// ShowWindow 命令常量
#[cfg(windows)]
const SW_HIDE: i32 = 0;
#[cfg(windows)]
const SW_SHOW: i32 = 5;
#[cfg(windows)]
const SW_MINIMIZE: i32 = 6;
#[cfg(windows)]
const SW_RESTORE: i32 = 9;

/// 获取图标纹理（缓存）
fn get_icon_texture(ctx: &egui::Context) -> egui::TextureHandle {
    use std::sync::OnceLock;
    static TEX: OnceLock<egui::TextureHandle> = OnceLock::new();
    if let Some(tex) = TEX.get() {
        return tex.clone();
    }
    let (rgba, w, h) = icon::icon_rgba();
    let tex = ctx.load_texture(
        "titlebar_icon",
        egui::ColorImage {
            size: [w as usize, h as usize],
            pixels: rgba
                .chunks_exact(4)
                .map(|c| egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
                .collect(),
        },
        egui::TextureOptions::LINEAR,
    );
    let _ = TEX.set(tex.clone());
    tex
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
                // ── 左侧图标 ──
                ui.add_space(8.0);
                let tex = get_icon_texture(ctx);
                let icon_size = 18.0;
                let img_resp = ui.add(
                    egui::Image::from_texture(&tex)
                        .fit_to_exact_size(egui::vec2(icon_size, icon_size)),
                );

                // 标题区域可拖动
                let title_drag = ui.interact(
                    img_resp.rect,
                    ui.id().with("title_drag"),
                    egui::Sense::drag(),
                );

                // ── 中间空白（可拖动）──
                let btn_w = 46.0 * 4.0;
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
                // 置顶
                let pinned = STATE_PINNED.load(Ordering::SeqCst);
                let pin_icon = if pinned { "\u{1F4CC}" } else { "\u{1F4CC}" };
                let pin_color = if pinned {
                    egui::Color32::from_rgb(76, 76, 76)
                } else {
                    egui::Color32::from_rgb(60, 60, 60)
                };
                let pin_resp = titlebar_button(ui, pin_icon, pin_color);
                if pinned {
                    ui.painter().rect_filled(
                        pin_resp.rect,
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(128, 128, 128, 64),
                    );
                    // 重绘图标确保在背景之上
                    ui.painter().text(
                        pin_resp.rect.center(),
                        egui::Align2::CENTER_CENTER,
                        pin_icon,
                        egui::FontId::proportional(14.0),
                        egui::Color32::from_gray(200),
                    );
                }
                if pin_resp.clicked() {
                    let new_pinned = !STATE_PINNED.load(Ordering::SeqCst);
                    STATE_PINNED.store(new_pinned, Ordering::SeqCst);
                    let level = if new_pinned {
                        egui::WindowLevel::AlwaysOnTop
                    } else {
                        egui::WindowLevel::Normal
                    };
                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
                }

                // 最小化
                let min_resp = titlebar_button(ui, "\u{2014}", egui::Color32::from_rgb(60, 60, 60));
                if min_resp.clicked() {
                    #[cfg(windows)]
                    win_show_window(SW_MINIMIZE);
                }

                // 最大化/还原
                let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                let max_icon = if maximized { "\u{2750}" } else { "\u{25A2}" };
                let max_resp = titlebar_button(ui, max_icon, egui::Color32::from_rgb(60, 60, 60));
                if max_resp.clicked() {
                    #[cfg(windows)]
                    {
                        if maximized {
                            win_show_window(SW_RESTORE);
                        } else {
                            // 用 Win32 SC_MAXIMIZE 最大化
                            use std::ffi::c_void;
                            const WM_SYSCOMMAND: u32 = 0x0112;
                            const SC_MAXIMIZE: usize = 0xF030;
                            extern "system" {
                                fn SendMessageW(
                                    hwnd: *mut c_void,
                                    msg: u32,
                                    wparam: usize,
                                    lparam: isize,
                                ) -> isize;
                            }
                            unsafe {
                                let hwnd = find_hwnd();
                                if !hwnd.is_null() {
                                    SendMessageW(hwnd, WM_SYSCOMMAND, SC_MAXIMIZE, 0);
                                }
                            }
                        }
                    }
                }

                // 关闭
                let close_resp =
                    titlebar_button(ui, "\u{2715}", egui::Color32::from_rgb(232, 17, 35));
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
