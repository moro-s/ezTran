//! 开机自启动模块 — 通过注册表 HKCU\...\Run 实现开机启动。
//!
//! Windows 上将当前 exe 路径写入 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`，
//! 值名 `EzTran`。启用时写入、禁用时删除。

use std::ffi::c_void;

/// 注册表值名（即启动项名称）
const RUN_VALUE_NAME: &str = "EzTran";

/// 注册表 Run 键路径
const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

const HKEY_CURRENT_USER: usize = 0x80000001;
const KEY_SET_VALUE: u32 = 0x0002;
const KEY_QUERY_VALUE: u32 = 0x0001;
const REG_SZ: u32 = 1;

extern "system" {
    fn RegOpenKeyExW(
        hkey: *mut c_void,
        lpsubkey: *const u16,
        reserved: u32,
        samdesired: u32,
        phkresult: *mut *mut c_void,
    ) -> i32;
    fn RegSetValueExW(
        hkey: *mut c_void,
        lpvaluename: *const u16,
        reserved: u32,
        dwtype: u32,
        lpdata: *const u8,
        cbdata: u32,
    ) -> i32;
    fn RegDeleteValueW(hkey: *mut c_void, lpvaluename: *const u16) -> i32;
    fn RegCloseKey(hkey: *mut c_void) -> i32;
}

fn to_wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 打开 Run 注册表键
fn open_run_key(access: u32) -> Option<*mut c_void> {
    let path = to_wide_nul(RUN_KEY_PATH);
    let mut hkey: *mut c_void = std::ptr::null_mut();
    let rc = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER as *mut c_void, path.as_ptr(), 0, access, &mut hkey) };
    if rc != 0 || hkey.is_null() {
        log::warn!("[autostart] 打开注册表 Run 键失败: rc={}", rc);
        return None;
    }
    Some(hkey)
}

/// 当前 exe 的完整路径（带引号，防止路径含空格被截断）
fn exe_command_line() -> Option<String> {
    std::env::current_exe()
        .ok()
        .map(|p| format!("\"{}\"", p.display()))
}

/// 设置开机自启动（写入注册表）
pub fn enable() -> bool {
    let exe = match exe_command_line() {
        Some(e) => e,
        None => {
            log::warn!("[autostart] 无法获取当前 exe 路径");
            return false;
        }
    };

    let hkey = match open_run_key(KEY_SET_VALUE) {
        Some(h) => h,
        None => return false,
    };

    // REG_SZ 数据需包含结尾的 \0，以 UTF-16 字节形式写入
    let mut data_wide = to_wide_nul(&exe);
    // to_wide_nul 已含一个 \0，RegSetValueExW 要求 cbdata 包含结尾 null 的字节数
    let byte_len = (data_wide.len() * 2) as u32;
    let rc = unsafe {
        let name = to_wide_nul(RUN_VALUE_NAME);
        RegSetValueExW(
            hkey,
            name.as_ptr(),
            0,
            REG_SZ,
            data_wide.as_mut_ptr() as *const u8,
            byte_len,
        )
    };
    unsafe { RegCloseKey(hkey) };

    if rc != 0 {
        log::warn!("[autostart] RegSetValueExW 失败: rc={}", rc);
        // 移除多余的 \0 避免重复（data_wide 已含一个 null）
        let _ = data_wide.pop();
        return false;
    }
    let _ = data_wide.pop();
    log::info!("[autostart] 已启用开机自启动: {}", exe);
    true
}

/// 取消开机自启动（删除注册表值）
pub fn disable() -> bool {
    let hkey = match open_run_key(KEY_SET_VALUE) {
        Some(h) => h,
        None => return false,
    };
    let name = to_wide_nul(RUN_VALUE_NAME);
    let rc = unsafe { RegDeleteValueW(hkey, name.as_ptr()) };
    unsafe { RegCloseKey(hkey) };

    // rc=2 (ERROR_FILE_NOT_FOUND) 表示值不存在，视为已禁用成功
    if rc != 0 && rc != 2 {
        log::warn!("[autostart] RegDeleteValueW 失败: rc={}", rc);
        return false;
    }
    log::info!("[autostart] 已禁用开机自启动");
    true
}

/// 查询当前注册表中是否已存在自启动项
pub fn is_enabled() -> bool {
    extern "system" {
        fn RegQueryValueExW(
            hkey: *mut c_void,
            lpvaluename: *const u16,
            lpreserved: *const u32,
            lptype: *mut u32,
            lpdata: *mut u8,
            lpcbdata: *mut u32,
        ) -> i32;
    }
    let hkey = match open_run_key(KEY_QUERY_VALUE) {
        Some(h) => h,
        None => return false,
    };
    let name = to_wide_nul(RUN_VALUE_NAME);
    let rc = unsafe {
        RegQueryValueExW(
            hkey,
            name.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    unsafe { RegCloseKey(hkey) };
    // rc=0 表示值存在
    rc == 0
}

/// 根据期望状态同步注册表（true=启用, false=禁用），返回是否成功
pub fn set_enabled(enabled: bool) -> bool {
    if enabled {
        enable()
    } else {
        disable()
    }
}
