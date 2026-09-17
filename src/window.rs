//! 窗口管理模块 — 统一封装主窗口和设置窗口的显示/隐藏/最小化/最大化操作。
//!
//! 无边框窗口下 eframe 的 ViewportCommand 不可靠，所有操作直接用 Win32 API。
//! 窗口隐藏（SW_HIDE）后 winit 事件循环暂停，需通过 PostMessage(WM_NULL) 唤醒。

use std::sync::atomic::{AtomicBool, Ordering};

// ── 全局状态 ──

/// 主窗口是否已隐藏到托盘
static MAIN_HIDDEN: AtomicBool = AtomicBool::new(false);

/// 窗口是否置顶
static PINNED: AtomicBool = AtomicBool::new(false);

/// 下一帧需要隐藏主窗口
static PENDING_HIDE: AtomicBool = AtomicBool::new(false);

/// 全局 egui Context（托盘事件回调中唤醒事件循环用）
static EGUI_CTX: std::sync::LazyLock<std::sync::Mutex<Option<egui::Context>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// 圆角是否已设置（仅执行一次）
#[cfg(windows)]
static ROUND_SET: AtomicBool = AtomicBool::new(false);

/// 主窗口隐藏前的位置（移到屏幕外前保存，恢复时用）
#[cfg(windows)]
static SAVED_RECT: std::sync::LazyLock<std::sync::Mutex<Option<WinRect>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Win32 RECT 结构体
#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct WinRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

// ── 公共 API ──

/// 注册 egui Context（每帧 update 开头调用）
pub fn set_ctx(ctx: &egui::Context) {
    *EGUI_CTX.lock().unwrap() = Some(ctx.clone());
}

/// 唤醒 winit 事件循环（窗口隐藏后 request_repaint 无效，需强制触发 WM_PAINT）
pub fn wake() {
    if let Some(ctx) = EGUI_CTX.lock().unwrap().as_ref() {
        ctx.request_repaint();
    }
    #[cfg(windows)]
    {
        post_message(WM_NULL, 0, 0);
        force_redraw();
    }
}

/// 后台守护线程入口：定期唤醒事件循环，防止窗口隐藏后托盘事件无法处理
pub fn start_repaint_thread() {
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
            wake();
        }
    });
}

/// 显示主窗口（从托盘恢复）
#[allow(dead_code)]
pub fn show_main() {
    log::info!("[window] show_main — 恢复主窗口");
    #[cfg(windows)]
    restore_from_offscreen();
    MAIN_HIDDEN.store(false, Ordering::SeqCst);
}

/// 隐藏主窗口到托盘
pub fn hide_main() {
    log::info!("[window] hide_main — 请求隐藏主窗口到托盘");
    PENDING_HIDE.store(true, Ordering::SeqCst);
}

/// 主窗口是否已隐藏
pub fn is_main_hidden() -> bool {
    MAIN_HIDDEN.load(Ordering::SeqCst)
}

/// 如果主窗口处于隐藏状态则恢复（供托盘事件回调调用）
pub fn show_main_if_hidden() {
    if MAIN_HIDDEN.swap(false, Ordering::SeqCst) {
        log::info!("[window] show_main_if_hidden — 主窗口已隐藏，恢复中");
        #[cfg(windows)]
        restore_from_offscreen();
    } else {
        log::debug!("[window] show_main_if_hidden — 主窗口未隐藏，仅置顶");
        #[cfg(windows)]
        bring_to_front();
    }
}

/// 将主窗口强制置于最前（绕过 Windows 前台窗口限制）
#[cfg(windows)]
pub fn bring_to_front() {
    use std::ffi::c_void;
    extern "system" {
        fn GetForegroundWindow() -> *mut c_void;
        fn GetWindowThreadProcessId(hwnd: *mut c_void, lpdw_process_id: *mut u32) -> u32;
        fn GetCurrentThreadId() -> u32;
        fn AttachThreadInput(id_attach: u32, id_attach_to: u32, f_attach: i32) -> i32;
        fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
        fn BringWindowToTop(hwnd: *mut c_void) -> i32;
        fn ShowWindow(hwnd: *mut c_void, cmd: i32) -> i32;
    }

    let hwnd = find_hwnd();
    if hwnd.is_null() {
        log::warn!("[window] bring_to_front — find_hwnd 返回空句柄");
        return;
    }

    unsafe {
        // 如果窗口被最小化，先恢复
        ShowWindow(hwnd, SW_RESTORE);

        // AttachThreadInput 技巧：将当前线程的输入队列与前台窗口的线程关联，
        // 这样 SetForegroundWindow 就能绕过 Windows 的前台窗口限制
        let fg_hwnd = GetForegroundWindow();
        let fg_tid = if !fg_hwnd.is_null() {
            GetWindowThreadProcessId(fg_hwnd, std::ptr::null_mut())
        } else {
            0
        };
        let cur_tid = GetCurrentThreadId();

        if fg_tid != 0 && fg_tid != cur_tid {
            AttachThreadInput(cur_tid, fg_tid, 1);
            SetForegroundWindow(hwnd);
            BringWindowToTop(hwnd);
            AttachThreadInput(cur_tid, fg_tid, 0);
        } else {
            SetForegroundWindow(hwnd);
            BringWindowToTop(hwnd);
        }
        log::info!("[window] bring_to_front — 已请求置顶 (fg_tid={})", fg_tid);
    }
}

/// 最小化主窗口
pub fn minimize_main() {
    log::info!("[window] minimize_main — 最小化主窗口");
    #[cfg(windows)]
    show_window(SW_MINIMIZE);
}

/// 最大化/还原主窗口
pub fn toggle_maximize(maximized: bool) {
    log::info!("[window] toggle_maximize — maximized={}", maximized);
    #[cfg(windows)]
    {
        if maximized {
            show_window(SW_RESTORE);
        } else {
            send_message(WM_SYSCOMMAND, SC_MAXIMIZE, 0);
        }
    }
}

/// 切换置顶状态
pub fn toggle_pin() -> bool {
    let new_pinned = !PINNED.load(Ordering::SeqCst);
    PINNED.store(new_pinned, Ordering::SeqCst);
    log::info!("[window] toggle_pin — 置顶状态切换为 {}", new_pinned);
    new_pinned
}

/// 当前是否置顶
pub fn is_pinned() -> bool {
    PINNED.load(Ordering::SeqCst)
}

/// Alt+F4 / 关闭按钮 → 隐藏到托盘（在 update 中调用）
pub fn handle_close_request(ctx: &egui::Context) -> bool {
    if ctx.input(|i| i.viewport().close_requested()) {
        log::info!("[window] handle_close_request — 收到关闭请求，转为隐藏到托盘");
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        if !MAIN_HIDDEN.load(Ordering::SeqCst) {
            PENDING_HIDE.store(true, Ordering::SeqCst);
        }
        return true;
    }
    false
}

/// 执行延迟隐藏（在 update 中调用，返回 true 表示已处理）
pub fn process_pending_hide() -> bool {
    if PENDING_HIDE.swap(false, Ordering::SeqCst) {
        log::info!("[window] process_pending_hide — 执行隐藏（移屏外）");
        #[cfg(windows)]
        move_offscreen();
        MAIN_HIDDEN.store(true, Ordering::SeqCst);
        true
    } else {
        false
    }
}

/// Windows 11: 设置窗口圆角（仅执行一次）
pub fn try_set_round_corners() {
    #[cfg(windows)]
    {
        if ROUND_SET.swap(true, Ordering::SeqCst) {
            return;
        }
        let hwnd = find_hwnd();
        if !hwnd.is_null() {
            let pref: i32 = 2; // DWMWCP_ROUND
            dwm_set_window_attribute(hwnd, 33, &pref);
        }
    }
}

/// 通过 WM_NCLBUTTONDOWN 让系统接管窗口拖动（无抖动）
pub fn start_drag() {
    #[cfg(windows)]
    {
        let hwnd = find_hwnd();
        if !hwnd.is_null() {
            release_capture();
            send_message(WM_NCLBUTTONDOWN, HTCAPTION, 0);
        }
    }
}

// ── Win32 FFI 底层 ──

#[cfg(windows)]
const WM_NULL: u32 = 0x0000;
#[cfg(windows)]
const WM_SYSCOMMAND: u32 = 0x0112;
#[cfg(windows)]
const WM_NCLBUTTONDOWN: u32 = 0xA1;
#[cfg(windows)]
const SC_MAXIMIZE: usize = 0xF030;
#[cfg(windows)]
const HTCAPTION: usize = 2;

#[cfg(windows)]
const SW_MINIMIZE: i32 = 6;
#[cfg(windows)]
const SW_RESTORE: i32 = 9;

#[cfg(windows)]
const GWL_EXSTYLE: i32 = -20;
#[cfg(windows)]
const WS_EX_TOOLWINDOW: u32 = 0x00000080;
#[cfg(windows)]
const WS_EX_APPWINDOW: u32 = 0x00040000;
#[cfg(windows)]
const SWP_NOZORDER: u32 = 0x0004;
#[cfg(windows)]
const SWP_NOACTIVATE: u32 = 0x0010;
#[cfg(windows)]
const SWP_FRAMECHANGE: u32 = 0x0020;

#[cfg(windows)]
fn find_hwnd() -> *mut std::ffi::c_void {
    use std::ffi::c_void;
    extern "system" {
        fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> *mut c_void;
    }
    let title: Vec<u16> = "EzTran - 翻译\0".encode_utf16().collect();
    unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) }
}

#[cfg(windows)]
fn show_window(cmd: i32) {
    use std::ffi::c_void;
    extern "system" {
        fn ShowWindow(hwnd: *mut c_void, cmd: i32) -> i32;
    }
    let hwnd = find_hwnd();
    if !hwnd.is_null() {
        unsafe { ShowWindow(hwnd, cmd) };
    }
}

/// 隐藏主窗口：移到屏幕外 + 从任务栏隐藏（不用分层窗口，避免 OpenGL 黑屏）
#[cfg(windows)]
fn move_offscreen() {
    use std::ffi::c_void;
    extern "system" {
        fn GetWindowRect(hwnd: *mut c_void, rect: *mut WinRect) -> i32;
        fn SetWindowPos(
            hwnd: *mut c_void,
            insert_after: *mut c_void,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            flags: u32,
        ) -> i32;
        fn GetWindowLongPtrW(hwnd: *mut c_void, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: *mut c_void, index: i32, value: isize) -> isize;
    }

    let hwnd = find_hwnd();
    if hwnd.is_null() {
        log::warn!("[window] hide — find_hwnd 返回空句柄，放弃");
        return;
    }

    unsafe {
        // 保存当前窗口位置
        let mut rect = WinRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetWindowRect(hwnd, &mut rect);
        log::info!(
            "[window] hide — 保存窗口位置 rect=({},{},{},{})",
            rect.left, rect.top, rect.right, rect.bottom
        );
        *SAVED_RECT.lock().unwrap() = Some(rect);

        // 添加 WS_EX_TOOLWINDOW + 移除 WS_EX_APPWINDOW：从任务栏和 Alt+Tab 隐藏
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let new_ex = (ex & !WS_EX_APPWINDOW) | WS_EX_TOOLWINDOW;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex as isize);
        log::info!("[window] hide — ex_style: 0x{:08X} -> 0x{:08X}", ex, new_ex);

        // 移到屏幕外（-32000 是 Windows 约定的屏幕外坐标）
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            -32000,
            -32000,
            w,
            h,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGE,
        );
        log::info!("[window] hide — 已移至屏幕外 + WS_EX_TOOLWINDOW");
    }
}

/// 恢复主窗口：从屏幕外移回原位置 + 恢复任务栏显示 + 置于最前
#[cfg(windows)]
fn restore_from_offscreen() {
    use std::ffi::c_void;
    extern "system" {
        fn SetWindowPos(
            hwnd: *mut c_void,
            insert_after: *mut c_void,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            flags: u32,
        ) -> i32;
        fn GetWindowLongPtrW(hwnd: *mut c_void, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: *mut c_void, index: i32, value: isize) -> isize;
        fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
        fn BringWindowToTop(hwnd: *mut c_void) -> i32;
    }

    let hwnd = find_hwnd();
    if hwnd.is_null() {
        log::warn!("[window] restore — find_hwnd 返回空句柄，放弃");
        return;
    }

    unsafe {
        // 移除 WS_EX_TOOLWINDOW，恢复 WS_EX_APPWINDOW
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let new_ex = (ex & !WS_EX_TOOLWINDOW) | WS_EX_APPWINDOW;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex as isize);
        log::info!("[window] restore — ex_style: 0x{:08X} -> 0x{:08X}", ex, new_ex);

        // 移回原位置
        if let Some(rect) = *SAVED_RECT.lock().unwrap() {
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                rect.left,
                rect.top,
                w,
                h,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGE,
            );
            log::info!(
                "[window] restore — 恢复至 ({},{},{},{})",
                rect.left, rect.top, rect.right, rect.bottom
            );
        } else {
            log::warn!("[window] restore — SAVED_RECT 为空，无位置可恢复");
        }

        // 强制置于最前（用 AttachThreadInput 绕过 Windows 前台窗口限制）
        {
            extern "system" {
                fn GetForegroundWindow() -> *mut c_void;
                fn GetWindowThreadProcessId(hwnd: *mut c_void, lpdw_process_id: *mut u32) -> u32;
                fn GetCurrentThreadId() -> u32;
                fn AttachThreadInput(id_attach: u32, id_attach_to: u32, f_attach: i32) -> i32;
            }
            let fg_hwnd = GetForegroundWindow();
            let fg_tid = if !fg_hwnd.is_null() {
                GetWindowThreadProcessId(fg_hwnd, std::ptr::null_mut())
            } else {
                0
            };
            let cur_tid = GetCurrentThreadId();
            if fg_tid != 0 && fg_tid != cur_tid {
                AttachThreadInput(cur_tid, fg_tid, 1);
                SetForegroundWindow(hwnd);
                BringWindowToTop(hwnd);
                AttachThreadInput(cur_tid, fg_tid, 0);
            } else {
                SetForegroundWindow(hwnd);
                BringWindowToTop(hwnd);
            }
        }
    }
}

#[cfg(windows)]
fn post_message(msg: u32, wparam: usize, lparam: isize) {
    use std::ffi::c_void;
    extern "system" {
        fn PostMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> i32;
    }
    let hwnd = find_hwnd();
    if !hwnd.is_null() {
        unsafe { PostMessageW(hwnd, msg, wparam, lparam) };
    } else {
        log::warn!("[window] post_message — find_hwnd 返回空句柄，无法 PostMessage");
    }
}

#[cfg(windows)]
fn send_message(msg: u32, wparam: usize, lparam: isize) {
    use std::ffi::c_void;
    extern "system" {
        fn SendMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
    }
    let hwnd = find_hwnd();
    if !hwnd.is_null() {
        unsafe { SendMessageW(hwnd, msg, wparam, lparam) };
    }
}

/// 强制触发 WM_PAINT（RDW_INTERNALPAINT），让 winit 产生 RedrawRequested → eframe update
#[cfg(windows)]
fn force_redraw() {
    use std::ffi::c_void;
    extern "system" {
        fn RedrawWindow(
            hwnd: *mut c_void,
            lprc_update: *const c_void,
            hrgn_update: *const c_void,
            flags: u32,
        ) -> i32;
    }
    const RDW_INTERNALPAINT: u32 = 0x0002;
    let hwnd = find_hwnd();
    if !hwnd.is_null() {
        unsafe {
            RedrawWindow(
                hwnd,
                std::ptr::null(),
                std::ptr::null(),
                RDW_INTERNALPAINT,
            )
        };
    }
}

#[cfg(windows)]
fn release_capture() {
    extern "system" {
        fn ReleaseCapture() -> i32;
    }
    unsafe { ReleaseCapture() };
}

#[cfg(windows)]
fn dwm_set_window_attribute(hwnd: *mut std::ffi::c_void, attr: u32, value: &i32) {
    use std::ffi::c_void;
    extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: *mut c_void,
            attr: u32,
            value: *const c_void,
            size: u32,
        ) -> i32;
    }
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attr,
            value as *const i32 as *const c_void,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

/// 获取主显示器尺寸（物理像素），用于窗口居中计算
#[cfg(windows)]
pub fn get_screen_size() -> (i32, i32) {
    extern "system" {
        fn GetSystemMetrics(nindex: i32) -> i32;
    }
    const SM_CXSCREEN: i32 = 0;
    const SM_CYSCREEN: i32 = 1;
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        log::debug!("[window] GetSystemMetrics: screen={}x{}", w, h);
        (w, h)
    }
}

#[cfg(not(windows))]
pub fn get_screen_size() -> (i32, i32) {
    (1920, 1080)
}
