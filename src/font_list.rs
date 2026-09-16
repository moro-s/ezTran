use std::collections::HashSet;
use std::sync::LazyLock;

#[repr(C)]
#[allow(non_snake_case)]
#[derive(Default)]
struct LOGFONTW {
    lfHeight: i32,
    lfWidth: i32,
    lfEscapement: i32,
    lfOrientation: i32,
    lfWeight: i32,
    lfItalic: u8,
    lfUnderline: u8,
    lfStrikeOut: u8,
    lfCharSet: u8,
    lfOutPrecision: u8,
    lfClipPrecision: u8,
    lfQuality: u8,
    lfPitchAndFamily: u8,
    lfFaceName: [u16; 32],
}

const DEFAULT_CHARSET: u8 = 1;

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetDC(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn ReleaseDC(hwnd: *mut std::ffi::c_void, hdc: *mut std::ffi::c_void) -> i32;
}

#[cfg(windows)]
#[link(name = "gdi32")]
extern "system" {
    fn EnumFontFamiliesExW(
        hdc: *mut std::ffi::c_void,
        lplogfont: *const LOGFONTW,
        lpproc: Option<unsafe extern "system" fn(*const LOGFONTW, *const u8, u32, isize) -> i32>,
        lparam: isize,
    ) -> i32;
}

#[cfg(windows)]
static SYSTEM_FONTS: LazyLock<Vec<String>> = LazyLock::new(enum_system_fonts);

#[cfg(windows)]
unsafe extern "system" fn collect_font(
    logfont: *const LOGFONTW,
    _metric: *const u8,
    _font_type: u32,
    lparam: isize,
) -> i32 {
    let set = &mut *(lparam as *mut HashSet<String>);
    let lf = &*logfont;
    let end = lf.lfFaceName.iter().position(|&c| c == 0).unwrap_or(32);
    let name = String::from_utf16_lossy(&lf.lfFaceName[..end]);
    if !name.is_empty() && !name.starts_with('@') {
        set.insert(name);
    }
    1
}

#[cfg(windows)]
fn enum_system_fonts() -> Vec<String> {
    unsafe {
        let hdc = GetDC(std::ptr::null_mut());
        if hdc.is_null() {
            return Vec::new();
        }

        let logfont = LOGFONTW {
            lfCharSet: DEFAULT_CHARSET,
            ..Default::default()
        };

        let mut set: HashSet<String> = HashSet::new();
        EnumFontFamiliesExW(
            hdc,
            &logfont,
            Some(collect_font),
            &mut set as *mut HashSet<String> as isize,
        );

        ReleaseDC(std::ptr::null_mut(), hdc);

        let mut list: Vec<String> = set.into_iter().collect();
        list.sort();
        list
    }
}

#[cfg(not(windows))]
fn enum_system_fonts() -> Vec<String> {
    Vec::new()
}

pub fn get_system_fonts() -> &'static [String] {
    #[cfg(windows)]
    {
        &SYSTEM_FONTS
    }
    #[cfg(not(windows))]
    {
        &[]
    }
}
