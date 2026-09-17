//! 快捷键系统 — Windows 全局热键 + 应用内快捷键捕获组件。
//!
//! 全局热键使用 `RegisterHotKey` API，在独立线程的消息队列中接收 `WM_HOTKEY`。
// 支持的修饰键: Ctrl, Shift, Alt, Win
// 支持的格式: "Ctrl+Shift+T", "Ctrl+Enter", "Alt+F1" 等

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

// ── 全局触发标志 ──

static TRIGGER_INPUT_TRANSLATE: AtomicBool = AtomicBool::new(false);
static TRIGGER_SELECTION_TRANSLATE: AtomicBool = AtomicBool::new(false);

/// 划词翻译获取到的选中文本（由热键线程写入，update 中消费）
static SELECTION_TEXT: Mutex<Option<String>> = Mutex::new(None);

/// 标记需要触发输入翻译（由全局热键线程设置，update 中消费）
pub fn request_input_translate() {
    TRIGGER_INPUT_TRANSLATE.store(true, Ordering::SeqCst);
}

/// 消费输入翻译请求（返回 true 表示需要执行）
pub fn consume_input_translate() -> bool {
    TRIGGER_INPUT_TRANSLATE.swap(false, Ordering::SeqCst)
}

/// 标记需要触发划词翻译（由全局热键线程设置，update 中消费）
pub fn request_selection_translate() {
    TRIGGER_SELECTION_TRANSLATE.store(true, Ordering::SeqCst);
}

/// 消费划词翻译请求（返回 true 表示需要执行）
pub fn consume_selection_translate() -> bool {
    TRIGGER_SELECTION_TRANSLATE.swap(false, Ordering::SeqCst)
}

/// 取出热键线程已获取的选中文本（消费后清空）
pub fn take_selection_text() -> Option<String> {
    SELECTION_TEXT.lock().unwrap().take()
}

// ── 快捷键解析 ──

/// 解析后的快捷键
#[derive(Debug, Clone, PartialEq)]
pub struct HotKey {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
    pub key: String,
}

impl HotKey {
    /// 从字符串解析快捷键，如 "Ctrl+Shift+T"
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut win = false;
        let mut key = String::new();

        for part in s.split('+') {
            let part = part.trim();
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => ctrl = true,
                "shift" => shift = true,
                "alt" => alt = true,
                "win" | "super" | "meta" => win = true,
                _ => {
                    if !key.is_empty() {
                        return None;
                    }
                    key = part.to_lowercase();
                }
            }
        }

        if key.is_empty() {
            return None;
        }

        Some(HotKey {
            ctrl,
            shift,
            alt,
            win,
            key,
        })
    }

    /// 格式化为显示字符串
    pub fn display(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".into());
        }
        if self.shift {
            parts.push("Shift".into());
        }
        if self.alt {
            parts.push("Alt".into());
        }
        if self.win {
            parts.push("Win".into());
        }
        parts.push(self.key.to_uppercase());
        parts.join("+")
    }
}

// ── Windows 全局热键 ──

#[cfg(windows)]
mod win {
    pub const WM_HOTKEY: u32 = 0x0312;
    pub const WM_QUIT: u32 = 0x0012;
    pub const WM_USER_REREGISTER: u32 = 0x0400 + 1;

    pub const MOD_ALT: u32 = 0x0001;
    pub const MOD_CONTROL: u32 = 0x0002;
    pub const MOD_SHIFT: u32 = 0x0004;
    pub const MOD_WIN: u32 = 0x0008;
    pub const MOD_NOREPEAT: u32 = 0x4000;

    pub const HOTKEY_ID_INPUT: i32 = 1;
    pub const HOTKEY_ID_SELECTION: i32 = 2;

    pub const INPUT_KEYBOARD: u32 = 1;
    pub const KEYEVENTF_KEYUP: u32 = 0x0002;
    pub const VK_CONTROL: u16 = 0x11;
    pub const VK_SHIFT: u16 = 0x10;
    pub const VK_MENU: u16 = 0x12;   // Alt
    pub const VK_LWIN: u16 = 0x5B;
    pub const VK_C: u16 = 0x43;

    #[repr(C)]
    #[derive(Default)]
    pub struct MSG {
        pub hwnd: isize,
        pub message: u32,
        pub w_param: usize,
        pub l_param: isize,
        pub time: u32,
        pub pt_x: i32,
        pub pt_y: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct KEYBDINPUT {
        pub w_vk: u16,
        pub w_scan: u16,
        pub dw_flags: u32,
        pub time: u32,
        pub dw_extra_info: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct MOUSEINPUT {
        pub dx: i32,
        pub dy: i32,
        pub mouse_data: u32,
        pub dw_flags: u32,
        pub time: u32,
        pub dw_extra_info: usize,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct HARDWAREINPUT {
        pub u_msg: u32,
        pub w_param_l: u16,
        pub w_param_h: u16,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union INPUT_UNION {
        pub ki: KEYBDINPUT,
        pub mi: MOUSEINPUT,
        pub hi: HARDWAREINPUT,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct INPUT {
        pub type_: u32,
        pub u: INPUT_UNION,
    }

    #[link(name = "user32")]
    extern "system" {
        pub fn GetMessageW(
            lp_msg: *mut MSG,
            hwnd: isize,
            w_msg_filter_min: u32,
            w_msg_filter_max: u32,
        ) -> i32;
        pub fn PostThreadMessageW(
            thread_id: u32,
            msg: u32,
            w_param: usize,
            l_param: isize,
        ) -> i32;
        pub fn RegisterHotKey(hwnd: isize, id: i32, fs_modifiers: u32, vk: u32) -> i32;
        pub fn UnregisterHotKey(hwnd: isize, id: i32) -> i32;
        pub fn SendInput(c_inputs: u32, p_inputs: *const INPUT, cb_size: i32) -> u32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetCurrentThreadId() -> u32;
    }
}

/// 热键线程 ID（0 表示未启动）
#[cfg(windows)]
static HOTKEY_THREAD_ID: AtomicU32 = AtomicU32::new(0);

/// 将 HotKey 转为 Windows 修饰键 + VK 码
#[cfg(windows)]
fn hotkey_to_win(hk: &HotKey) -> Option<(u32, u32)> {
    let mut mods = 0u32;
    if hk.ctrl {
        mods |= win::MOD_CONTROL;
    }
    if hk.shift {
        mods |= win::MOD_SHIFT;
    }
    if hk.alt {
        mods |= win::MOD_ALT;
    }
    if hk.win {
        mods |= win::MOD_WIN;
    }
    mods |= win::MOD_NOREPEAT;
    let vk = key_to_vk(&hk.key)?;
    Some((mods, vk))
}

/// 将键名（小写）转为 Windows VK 码
#[cfg(windows)]
fn key_to_vk(key: &str) -> Option<u32> {
    match key.to_lowercase().as_str() {
        "a" => Some(0x41), "b" => Some(0x42), "c" => Some(0x43), "d" => Some(0x44),
        "e" => Some(0x45), "f" => Some(0x46), "g" => Some(0x47), "h" => Some(0x48),
        "i" => Some(0x49), "j" => Some(0x4A), "k" => Some(0x4B), "l" => Some(0x4C),
        "m" => Some(0x4D), "n" => Some(0x4E), "o" => Some(0x4F), "p" => Some(0x50),
        "q" => Some(0x51), "r" => Some(0x52), "s" => Some(0x53), "t" => Some(0x54),
        "u" => Some(0x55), "v" => Some(0x56), "w" => Some(0x57), "x" => Some(0x58),
        "y" => Some(0x59), "z" => Some(0x5A),
        "0" => Some(0x30), "1" => Some(0x31), "2" => Some(0x32), "3" => Some(0x33),
        "4" => Some(0x34), "5" => Some(0x35), "6" => Some(0x36), "7" => Some(0x37),
        "8" => Some(0x38), "9" => Some(0x39),
        "f1" => Some(0x70), "f2" => Some(0x71), "f3" => Some(0x72), "f4" => Some(0x73),
        "f5" => Some(0x74), "f6" => Some(0x75), "f7" => Some(0x76), "f8" => Some(0x77),
        "f9" => Some(0x78), "f10" => Some(0x79), "f11" => Some(0x7A), "f12" => Some(0x7B),
        "enter" | "return" => Some(0x0D),
        "escape" | "esc" => Some(0x1B),
        "tab" => Some(0x09),
        "space" => Some(0x20),
        "backspace" => Some(0x08),
        "delete" | "del" => Some(0x2E),
        "insert" => Some(0x2D),
        "home" => Some(0x24),
        "end" => Some(0x23),
        "pageup" => Some(0x21),
        "pagedown" => Some(0x22),
        "up" | "arrowup" => Some(0x26),
        "down" | "arrowdown" => Some(0x28),
        "left" | "arrowleft" => Some(0x25),
        "right" | "arrowright" => Some(0x27),
        _ => None,
    }
}

/// 注册热键（读取当前配置）
#[cfg(windows)]
unsafe fn register_hotkeys() {
    let config = crate::ui::state::STATE.lock().unwrap().config_edit.clone();

    if config.enable_input_translate {
        if let Some(hk) = HotKey::parse(&config.input_hotkey) {
            if let Some((mods, vk)) = hotkey_to_win(&hk) {
                let ret = win::RegisterHotKey(0, win::HOTKEY_ID_INPUT, mods, vk);
                if ret != 0 {
                    log::info!(
                        "[hotkey] 注册输入翻译热键: {} (mods=0x{:X}, vk=0x{:X})",
                        hk.display(), mods, vk
                    );
                } else {
                    log::warn!("[hotkey] 注册输入翻译热键失败: {}", hk.display());
                }
            } else {
                log::warn!("[hotkey] 输入翻译热键 VK 码无法识别: '{}'", hk.key);
            }
        } else {
            log::warn!("[hotkey] 输入翻译热键解析失败: '{}'", config.input_hotkey);
        }
    }

    if config.enable_selection_translate {
        if let Some(hk) = HotKey::parse(&config.selection_hotkey) {
            if let Some((mods, vk)) = hotkey_to_win(&hk) {
                let ret = win::RegisterHotKey(0, win::HOTKEY_ID_SELECTION, mods, vk);
                if ret != 0 {
                    log::info!(
                        "[hotkey] 注册划词翻译热键: {} (mods=0x{:X}, vk=0x{:X})",
                        hk.display(), mods, vk
                    );
                } else {
                    log::warn!("[hotkey] 注册划词翻译热键失败: {}", hk.display());
                }
            } else {
                log::warn!("[hotkey] 划词翻译热键 VK 码无法识别: '{}'", hk.key);
            }
        } else {
            log::warn!("[hotkey] 划词翻译热键解析失败: '{}'", config.selection_hotkey);
        }
    }
}

/// 注销所有热键
#[cfg(windows)]
unsafe fn unregister_all() {
    win::UnregisterHotKey(0, win::HOTKEY_ID_INPUT);
    win::UnregisterHotKey(0, win::HOTKEY_ID_SELECTION);
    log::info!("[hotkey] 已注销所有全局热键");
}

/// 全局热键线程入口
#[cfg(windows)]
fn hotkey_thread() {
    let thread_id = unsafe { win::GetCurrentThreadId() };
    HOTKEY_THREAD_ID.store(thread_id, Ordering::SeqCst);
    log::info!("[hotkey] 全局热键线程已启动 (thread_id={})", thread_id);

    unsafe {
        register_hotkeys();

        let mut msg: win::MSG = std::mem::zeroed();
        loop {
            let ret = win::GetMessageW(&mut msg, 0, 0, 0);
            if ret <= 0 {
                break;
            }

            match msg.message {
                win::WM_HOTKEY => {
                    let id = msg.w_param as i32;
                    match id {
                        win::HOTKEY_ID_INPUT => {
                            log::info!("[hotkey] WM_HOTKEY — 输入翻译");
                            request_input_translate();
                        }
                        win::HOTKEY_ID_SELECTION => {
                            log::info!("[hotkey] WM_HOTKEY — 划词翻译");
                            // 在热键线程中立即执行模拟复制（此时前台窗口仍是用户选中文本的应用）
                            let text = simulate_copy_and_get_clipboard();
                            if let Some(ref t) = text {
                                log::info!("[hotkey] 划词翻译 — 已获取选中文本 ({} 字符)", t.len());
                            } else {
                                log::warn!("[hotkey] 划词翻译 — 未获取到选中文本");
                            }
                            *SELECTION_TEXT.lock().unwrap() = text;
                            request_selection_translate();
                        }
                        _ => {
                            log::debug!("[hotkey] WM_HOTKEY — 未知 id={}", id);
                        }
                    }
                }
                win::WM_USER_REREGISTER => {
                    log::info!("[hotkey] 收到重新注册信号");
                    unregister_all();
                    register_hotkeys();
                }
                win::WM_QUIT => {
                    log::info!("[hotkey] 收到 WM_QUIT，退出线程");
                    break;
                }
                _ => {}
            }
        }

        unregister_all();
    }

    HOTKEY_THREAD_ID.store(0, Ordering::SeqCst);
    log::info!("[hotkey] 全局热键线程已退出");
}

/// 启动全局热键线程（在 app 初始化时调用一次）
pub fn start_global_hotkey_thread() {
    #[cfg(windows)]
    {
        std::thread::spawn(hotkey_thread);
    }
    #[cfg(not(windows))]
    {
        log::warn!("[hotkey] 全局热键仅支持 Windows 平台");
    }
}

/// 重新注册热键（配置变更后调用，通知线程先注销再注册）
pub fn reregister_hotkeys() {
    #[cfg(windows)]
    {
        let tid = HOTKEY_THREAD_ID.load(Ordering::SeqCst);
        if tid != 0 {
            unsafe {
                win::PostThreadMessageW(tid, win::WM_USER_REREGISTER, 0, 0);
            }
            log::info!("[hotkey] 已发送重新注册信号到线程 {}", tid);
        } else {
            log::warn!("[hotkey] 热键线程未运行，无法重新注册");
        }
    }
}

// ── 划词翻译：模拟 Ctrl+C 获取选中文本 ──

/// 构造一个键盘 INPUT 结构体
#[cfg(windows)]
fn make_key_input(vk: u16, flags: u32) -> win::INPUT {
    win::INPUT {
        type_: win::INPUT_KEYBOARD,
        u: win::INPUT_UNION {
            ki: win::KEYBDINPUT {
                w_vk: vk,
                w_scan: 0,
                dw_flags: flags,
                time: 0,
                dw_extra_info: 0,
            },
        },
    }
}

/// 模拟 Ctrl+C 复制当前选中文本，从剪贴板读取并返回。
/// 保留用户原有剪贴板内容，操作完成后恢复。
///
/// 注意：热键触发时用户仍按住修饰键（如 Ctrl+Shift），需要先释放所有修饰键，
/// 再模拟 Ctrl+C，否则 Shift 会干扰复制操作。
pub fn simulate_copy_and_get_clipboard() -> Option<String> {
    // 保存旧剪贴板内容
    let old_clip = arboard::Clipboard::new()
        .ok()
        .and_then(|mut c| c.get_text().ok());

    // 清空剪贴板（用于判断 Ctrl+C 是否真的复制了新内容）
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text("");
    }

    #[cfg(windows)]
    unsafe {
        // 1. 释放所有可能按下的修饰键
        let release_mods: [win::INPUT; 4] = [
            make_key_input(win::VK_CONTROL, win::KEYEVENTF_KEYUP),
            make_key_input(win::VK_SHIFT, win::KEYEVENTF_KEYUP),
            make_key_input(win::VK_MENU, win::KEYEVENTF_KEYUP),   // Alt
            make_key_input(win::VK_LWIN, win::KEYEVENTF_KEYUP),   // Win
        ];
        win::SendInput(4, release_mods.as_ptr(), std::mem::size_of::<win::INPUT>() as i32);

        // 短暂等待，确保修饰键释放生效
        std::thread::sleep(std::time::Duration::from_millis(50));

        // 2. 模拟 Ctrl+C
        let copy_inputs: [win::INPUT; 4] = [
            make_key_input(win::VK_CONTROL, 0),
            make_key_input(win::VK_C, 0),
            make_key_input(win::VK_C, win::KEYEVENTF_KEYUP),
            make_key_input(win::VK_CONTROL, win::KEYEVENTF_KEYUP),
        ];
        let sent = win::SendInput(4, copy_inputs.as_ptr(), std::mem::size_of::<win::INPUT>() as i32);
        log::debug!("[hotkey] SendInput Ctrl+C 发送 {} 个按键事件", sent);
    }

    // 等待剪贴板更新
    std::thread::sleep(std::time::Duration::from_millis(150));

    // 读取新内容
    let new_text = arboard::Clipboard::new()
        .ok()
        .and_then(|mut c| c.get_text().ok())
        .filter(|s| !s.is_empty());

    // 恢复旧剪贴板内容
    if let Some(ref old) = old_clip {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(old);
        }
    }

    if new_text.is_some() {
        log::info!("[hotkey] 划词翻译 — 已获取选中文本");
    } else {
        log::warn!("[hotkey] 划词翻译 — 未获取到选中文本");
    }

    new_text
}

// ── 快捷键输入框组件（设置页使用）──

/// 绘制快捷键捕获输入框（在设置页使用）
/// 点击后捕获按键，显示当前快捷键
pub fn hotkey_input(ui: &mut egui::Ui, _id: &str, value: &mut String) {
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(160.0, ui.spacing().interact_size.y),
        egui::Sense::click(),
    );

    // 显示当前值或占位符
    let display = if value.is_empty() {
        "点击设置快捷键".to_string()
    } else if resp.has_focus() {
        "按下快捷键...".to_string()
    } else {
        value.clone()
    };

    let bg = if resp.has_focus() {
        egui::Color32::from_rgb(80, 80, 80)
    } else if resp.hovered() {
        egui::Color32::from_rgb(76, 76, 76)
    } else {
        egui::Color32::from_rgb(60, 60, 60)
    };

    ui.painter().rect_filled(rect, 4.0, bg);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        display,
        egui::FontId::proportional(14.0),
        egui::Color32::from_gray(200),
    );

    // 获取焦点后捕获按键
    if resp.has_focus() {
        let mods = ui.input(|i| i.modifiers);
        let mut parts: Vec<String> = Vec::new();
        if mods.ctrl {
            parts.push("Ctrl".into());
        }
        if mods.shift {
            parts.push("Shift".into());
        }
        if mods.alt {
            parts.push("Alt".into());
        }
        if mods.mac_cmd {
            parts.push("Win".into());
        }

        // 检测是否有非修饰键按下
        let key_pressed = ui.input(|i| {
            for key in [
                egui::Key::A, egui::Key::B, egui::Key::C, egui::Key::D, egui::Key::E,
                egui::Key::F, egui::Key::G, egui::Key::H, egui::Key::I, egui::Key::J,
                egui::Key::K, egui::Key::L, egui::Key::M, egui::Key::N, egui::Key::O,
                egui::Key::P, egui::Key::Q, egui::Key::R, egui::Key::S, egui::Key::T,
                egui::Key::U, egui::Key::V, egui::Key::W, egui::Key::X, egui::Key::Y,
                egui::Key::Z, egui::Key::Num0, egui::Key::Num1, egui::Key::Num2,
                egui::Key::Num3, egui::Key::Num4, egui::Key::Num5, egui::Key::Num6,
                egui::Key::Num7, egui::Key::Num8, egui::Key::Num9, egui::Key::F1,
                egui::Key::F2, egui::Key::F3, egui::Key::F4, egui::Key::F5,
                egui::Key::F6, egui::Key::F7, egui::Key::F8, egui::Key::F9,
                egui::Key::F10, egui::Key::F11, egui::Key::F12, egui::Key::Enter,
                egui::Key::Space, egui::Key::Tab,
            ] {
                if i.key_pressed(key) {
                    return Some(key);
                }
            }
            None
        });

        if let Some(key) = key_pressed {
            let key_name = egui_key_to_string(&key);
            parts.push(key_name);
            *value = parts.join("+");
            ui.memory_mut(|m| m.surrender_focus(resp.id));
        }

        // Escape 取消
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.memory_mut(|m| m.surrender_focus(resp.id));
        }
    }
}

/// egui::Key 转为显示字符串
fn egui_key_to_string(key: &egui::Key) -> String {
    match key {
        egui::Key::A => "A".into(),
        egui::Key::B => "B".into(),
        egui::Key::C => "C".into(),
        egui::Key::D => "D".into(),
        egui::Key::E => "E".into(),
        egui::Key::F => "F".into(),
        egui::Key::G => "G".into(),
        egui::Key::H => "H".into(),
        egui::Key::I => "I".into(),
        egui::Key::J => "J".into(),
        egui::Key::K => "K".into(),
        egui::Key::L => "L".into(),
        egui::Key::M => "M".into(),
        egui::Key::N => "N".into(),
        egui::Key::O => "O".into(),
        egui::Key::P => "P".into(),
        egui::Key::Q => "Q".into(),
        egui::Key::R => "R".into(),
        egui::Key::S => "S".into(),
        egui::Key::T => "T".into(),
        egui::Key::U => "U".into(),
        egui::Key::V => "V".into(),
        egui::Key::W => "W".into(),
        egui::Key::X => "X".into(),
        egui::Key::Y => "Y".into(),
        egui::Key::Z => "Z".into(),
        egui::Key::Num0 => "0".into(),
        egui::Key::Num1 => "1".into(),
        egui::Key::Num2 => "2".into(),
        egui::Key::Num3 => "3".into(),
        egui::Key::Num4 => "4".into(),
        egui::Key::Num5 => "5".into(),
        egui::Key::Num6 => "6".into(),
        egui::Key::Num7 => "7".into(),
        egui::Key::Num8 => "8".into(),
        egui::Key::Num9 => "9".into(),
        egui::Key::F1 => "F1".into(),
        egui::Key::F2 => "F2".into(),
        egui::Key::F3 => "F3".into(),
        egui::Key::F4 => "F4".into(),
        egui::Key::F5 => "F5".into(),
        egui::Key::F6 => "F6".into(),
        egui::Key::F7 => "F7".into(),
        egui::Key::F8 => "F8".into(),
        egui::Key::F9 => "F9".into(),
        egui::Key::F10 => "F10".into(),
        egui::Key::F11 => "F11".into(),
        egui::Key::F12 => "F12".into(),
        egui::Key::Enter => "Enter".into(),
        egui::Key::Space => "Space".into(),
        egui::Key::Tab => "Tab".into(),
        _ => "?".into(),
    }
}
