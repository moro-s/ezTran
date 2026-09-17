//! 快捷键系统 — 统一管理应用内快捷键的解析、匹配和回调。
//!
//! 支持的修饰键: Ctrl, Shift, Alt, Win
//! 支持的格式: "Ctrl+Shift+T", "Ctrl+Enter", "Alt+F1" 等

use std::sync::atomic::{AtomicBool, Ordering};

// ── 全局标志：是否触发输入翻译 ──

static TRIGGER_INPUT_TRANSLATE: AtomicBool = AtomicBool::new(false);

/// 标记需要触发输入翻译（由快捷键回调设置，update 中消费）
pub fn request_input_translate() {
    TRIGGER_INPUT_TRANSLATE.store(true, Ordering::SeqCst);
}

/// 消费输入翻译请求（返回 true 表示本帧需要执行翻译）
pub fn consume_input_translate() -> bool {
    TRIGGER_INPUT_TRANSLATE.swap(false, Ordering::SeqCst)
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
                        return None; // 多个非修饰键，无效
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

/// 在 egui 上下文中检测快捷键是否被按下
pub fn check_egui_hotkey(ctx: &egui::Context, hotkey: &HotKey) -> bool {
    ctx.input(|i| {
        let modifiers = &i.modifiers;
        if modifiers.ctrl != hotkey.ctrl
            || modifiers.shift != hotkey.shift
            || modifiers.alt != hotkey.alt
            || modifiers.mac_cmd != hotkey.win
        {
            return false;
        }

        // 尝试匹配按键
        let key_lower = &hotkey.key;
        if let Some(egui_key) = parse_egui_key(key_lower) {
            return i.key_pressed(egui_key);
        }

        // 单字符匹配
        if key_lower.len() == 1 {
            let c = key_lower.chars().next().unwrap();
            if let Some(egui_key) = char_to_egui_key(c) {
                return i.key_pressed(egui_key);
            }
        }

        false
    })
}

/// 将字符串键名转为 egui::Key
fn parse_egui_key(s: &str) -> Option<egui::Key> {
    match s.to_lowercase().as_str() {
        "enter" | "return" => Some(egui::Key::Enter),
        "escape" | "esc" => Some(egui::Key::Escape),
        "tab" => Some(egui::Key::Tab),
        "space" => Some(egui::Key::Space),
        "backspace" => Some(egui::Key::Backspace),
        "delete" | "del" => Some(egui::Key::Delete),
        "insert" => Some(egui::Key::Insert),
        "home" => Some(egui::Key::Home),
        "end" => Some(egui::Key::End),
        "pageup" => Some(egui::Key::PageUp),
        "pagedown" => Some(egui::Key::PageDown),
        "up" | "arrowup" => Some(egui::Key::ArrowUp),
        "down" | "arrowdown" => Some(egui::Key::ArrowDown),
        "left" | "arrowleft" => Some(egui::Key::ArrowLeft),
        "right" | "arrowright" => Some(egui::Key::ArrowRight),
        "f1" => Some(egui::Key::F1),
        "f2" => Some(egui::Key::F2),
        "f3" => Some(egui::Key::F3),
        "f4" => Some(egui::Key::F4),
        "f5" => Some(egui::Key::F5),
        "f6" => Some(egui::Key::F6),
        "f7" => Some(egui::Key::F7),
        "f8" => Some(egui::Key::F8),
        "f9" => Some(egui::Key::F9),
        "f10" => Some(egui::Key::F10),
        "f11" => Some(egui::Key::F11),
        "f12" => Some(egui::Key::F12),
        _ => None,
    }
}

/// 将单字符转为 egui::Key
fn char_to_egui_key(c: char) -> Option<egui::Key> {
    match c {
        'a' => Some(egui::Key::A),
        'b' => Some(egui::Key::B),
        'c' => Some(egui::Key::C),
        'd' => Some(egui::Key::D),
        'e' => Some(egui::Key::E),
        'f' => Some(egui::Key::F),
        'g' => Some(egui::Key::G),
        'h' => Some(egui::Key::H),
        'i' => Some(egui::Key::I),
        'j' => Some(egui::Key::J),
        'k' => Some(egui::Key::K),
        'l' => Some(egui::Key::L),
        'm' => Some(egui::Key::M),
        'n' => Some(egui::Key::N),
        'o' => Some(egui::Key::O),
        'p' => Some(egui::Key::P),
        'q' => Some(egui::Key::Q),
        'r' => Some(egui::Key::R),
        's' => Some(egui::Key::S),
        't' => Some(egui::Key::T),
        'u' => Some(egui::Key::U),
        'v' => Some(egui::Key::V),
        'w' => Some(egui::Key::W),
        'x' => Some(egui::Key::X),
        'y' => Some(egui::Key::Y),
        'z' => Some(egui::Key::Z),
        '0' => Some(egui::Key::Num0),
        '1' => Some(egui::Key::Num1),
        '2' => Some(egui::Key::Num2),
        '3' => Some(egui::Key::Num3),
        '4' => Some(egui::Key::Num4),
        '5' => Some(egui::Key::Num5),
        '6' => Some(egui::Key::Num6),
        '7' => Some(egui::Key::Num7),
        '8' => Some(egui::Key::Num8),
        '9' => Some(egui::Key::Num9),
        _ => None,
    }
}

// ── 应用内快捷键检测 ──

/// 检测应用内快捷键并触发对应动作（在 update 中调用）
pub fn check_app_hotkeys(ctx: &egui::Context) {
    let config = crate::ui::state::STATE.lock().unwrap().config_edit.clone();

    // 输入翻译快捷键（如 Ctrl+Shift+I）
    if config.enable_input_translate {
        if let Some(hk) = HotKey::parse(&config.input_hotkey) {
            if check_egui_hotkey(ctx, &hk) {
                log::info!("[hotkey] 输入翻译快捷键触发: {}", hk.display());
                request_input_translate();
            }
        }
    }
}

// ── 快捷键输入框组件 ──

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
        // 捕获修饰键 + 普通键
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
