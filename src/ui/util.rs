//! Small Win32 helpers.

use windows::core::HSTRING;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
pub fn hs(s: &str) -> HSTRING {
    HSTRING::from(s)
}
pub fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}
pub fn cr_r(c: COLORREF) -> u8 {
    (c.0 & 0xFF) as u8
}
pub fn cr_g(c: COLORREF) -> u8 {
    ((c.0 >> 8) & 0xFF) as u8
}
pub fn cr_b(c: COLORREF) -> u8 {
    ((c.0 >> 16) & 0xFF) as u8
}
pub fn mix(a: COLORREF, b: COLORREF, t: f64) -> COLORREF {
    let m = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
    rgb(m(cr_r(a), cr_r(b)), m(cr_g(a), cr_g(b)), m(cr_b(a), cr_b(b)))
}
pub fn rect(l: i32, t: i32, r: i32, b: i32) -> RECT {
    RECT { left: l, top: t, right: r, bottom: b }
}
pub fn loword(v: usize) -> u32 {
    (v & 0xFFFF) as u32
}
pub fn hiword(v: usize) -> u32 {
    ((v >> 16) & 0xFFFF) as u32
}
pub fn x_lparam(l: LPARAM) -> i32 {
    (l.0 & 0xFFFF) as u16 as i16 as i32
}
pub fn y_lparam(l: LPARAM) -> i32 {
    ((l.0 >> 16) & 0xFFFF) as u16 as i16 as i32
}
pub fn mul_div(a: i32, b: i32, c: i32) -> i32 {
    ((a as i64 * b as i64) / c as i64) as i32
}
pub fn pt_in(r: &RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}
pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
pub fn arg_present(name: &str) -> bool {
    std::env::args().skip(1).any(|a| a.eq_ignore_ascii_case(name))
}

pub unsafe fn send(h: HWND, m: u32, wp: usize, lp: isize) -> isize {
    SendMessageW(h, m, Some(WPARAM(wp)), Some(LPARAM(lp))).0
}
pub unsafe fn post(h: HWND, m: u32, wp: usize, lp: isize) {
    let _ = PostMessageW(Some(h), m, WPARAM(wp), LPARAM(lp));
}
pub unsafe fn get_text(h: HWND) -> String {
    let n = GetWindowTextLengthW(h);
    if n <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; n as usize + 1];
    let got = GetWindowTextW(h, &mut buf);
    String::from_utf16_lossy(&buf[..got.max(0) as usize])
}
pub unsafe fn set_text(h: HWND, s: &str) {
    if get_text(h) != s {
        let _ = SetWindowTextW(h, &hs(s));
    }
}
/// Converts `\n` line ends to `\r\n` for multi-line edit controls.
pub fn crlf(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\n', "\r\n")
}
pub unsafe fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 64];
    let n = GetClassNameW(h, &mut buf);
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}
pub unsafe fn is_edit(h: HWND) -> bool {
    !h.is_invalid() && class_name(h).eq_ignore_ascii_case("Edit")
}
pub unsafe fn show(h: HWND, on: bool) {
    if !h.is_invalid() {
        let _ = ShowWindow(h, if on { SW_SHOWNA } else { SW_HIDE });
    }
}
pub unsafe fn enable(h: HWND, on: bool) {
    if !h.is_invalid() {
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(h, on);
    }
}
pub unsafe fn move_to(h: HWND, r: RECT) {
    if !h.is_invalid() {
        let _ = MoveWindow(h, r.left, r.top, (r.right - r.left).max(0), (r.bottom - r.top).max(0), true);
    }
}
pub unsafe fn text_size(dc: HDC, s: &str) -> SIZE {
    let w: Vec<u16> = s.encode_utf16().collect();
    let mut sz = SIZE::default();
    let _ = GetTextExtentPoint32W(dc, &w, &mut sz);
    sz
}
pub unsafe fn draw_text(dc: HDC, s: &str, r: RECT, c: COLORREF, fmt: DRAW_TEXT_FORMAT) {
    let mut w: Vec<u16> = s.encode_utf16().collect();
    let mut r = r;
    SetTextColor(dc, c);
    SetBkMode(dc, TRANSPARENT);
    DrawTextW(dc, &mut w, &mut r, fmt);
}
pub unsafe fn fill(dc: HDC, r: &RECT, c: COLORREF) {
    let b = CreateSolidBrush(c);
    FillRect(dc, r, b);
    let _ = DeleteObject(b.into());
}

pub const SS_LEFT: u32 = 0x0000;
pub const SS_NOPREFIX: u32 = 0x0080;
pub const SS_SUNKEN: u32 = 0x1000;
