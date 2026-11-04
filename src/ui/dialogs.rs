//! Message dialogs (TaskDialog, Esc closes them), the explanation overlay
//! and the settings dialog (SPEC-003 §6.3, §7).

use super::store::{self, hotkey_text, key_name, APP_NAME};
use super::util::*;
use super::{s, st};
use crate::core::i18n::{self, tr, trf};
use crate::core::model::matches;
use std::ffi::c_void;
use std::mem::size_of;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::Dialogs::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const HELP_CLASS: PCWSTR = w!("OpenTmuxTabsHelp");
const SETTINGS_CLASS: PCWSTR = w!("OpenTmuxTabsSettings");

pub unsafe fn register_classes(inst: HINSTANCE, icon: HICON, cursor: HCURSOR) {
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(help_proc),
        hInstance: inst,
        hCursor: cursor,
        lpszClassName: HELP_CLASS,
        style: CS_DROPSHADOW,
        ..Default::default()
    };
    RegisterClassExW(&wc);
    let sc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(settings_proc),
        hInstance: inst,
        hCursor: cursor,
        hIcon: icon,
        hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
        lpszClassName: SETTINGS_CLASS,
        ..Default::default()
    };
    RegisterClassExW(&sc);
}

fn owner() -> HWND {
    let s = st().settings_wnd;
    if !s.is_invalid() {
        s
    } else {
        st().main
    }
}

unsafe fn task_dialog(title: &str, text: &str, icon: PCWSTR, buttons: &[(i32, String)], default: i32) -> i32 {
    let t = wide(title);
    let c = wide(text);
    let app = wide(APP_NAME);
    let labels: Vec<Vec<u16>> = buttons.iter().map(|(_, l)| wide(l)).collect();
    let btns: Vec<TASKDIALOG_BUTTON> =
        buttons.iter().zip(&labels).map(|((id, _), l)| TASKDIALOG_BUTTON { nButtonID: *id, pszButtonText: PCWSTR(l.as_ptr()) }).collect();
    let mut flags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW;
    if i18n::is_rtl() {
        flags |= TDF_RTL_LAYOUT;
    }
    let cfg = TASKDIALOGCONFIG {
        cbSize: size_of::<TASKDIALOGCONFIG>() as u32,
        hwndParent: owner(),
        hInstance: st().inst,
        dwFlags: flags,
        pszWindowTitle: PCWSTR(app.as_ptr()),
        Anonymous1: TASKDIALOGCONFIG_0 { pszMainIcon: icon },
        pszMainInstruction: PCWSTR(t.as_ptr()),
        pszContent: PCWSTR(c.as_ptr()),
        cButtons: btns.len() as u32,
        pButtons: btns.as_ptr(),
        nDefaultButton: default,
        ..Default::default()
    };
    let mut pressed = IDCANCEL.0;
    let _ = TaskDialogIndirect(&cfg, Some(&mut pressed), None, None);
    pressed
}

/// Yes/No question; Esc counts as No.
pub unsafe fn confirm(question: &str) -> bool {
    let r = task_dialog(question, "", TD_WARNING_ICON, &[(IDYES.0, tr("Yes")), (IDNO.0, tr("No"))], IDNO.0);
    r == IDYES.0
}

pub unsafe fn error(title: &str, text: &str) {
    task_dialog(title, text, TD_ERROR_ICON, &[(IDOK.0, tr("OK"))], IDOK.0);
}

pub unsafe fn info(title: &str, text: &str) {
    task_dialog(title, text, TD_INFORMATION_ICON, &[(IDOK.0, tr("OK"))], IDOK.0);
}

// ----------------------------------------------------------------------------
// Explanation overlay
// ----------------------------------------------------------------------------
static mut HELP_TEXT: String = String::new();

fn help_text() -> &'static str {
    unsafe { &*std::ptr::addr_of!(HELP_TEXT) }
}

unsafe fn help_format() -> DRAW_TEXT_FORMAT {
    let mut f = DT_LEFT | DT_WORDBREAK | DT_NOPREFIX | DT_EXPANDTABS;
    if i18n::is_rtl() {
        f |= DT_RTLREADING | DT_RIGHT;
    }
    f
}

/// Shows the overlay above the footer. `anchor` is a footer item in client
/// coordinates of the main window; `None` = centered (F1).
pub unsafe fn show_overlay(text: &str, anchor: Option<RECT>) {
    *std::ptr::addr_of_mut!(HELP_TEXT) = text.to_string();
    let main = st().main;
    if st().help_wnd.is_invalid() {
        let ex = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | if i18n::is_rtl() { WS_EX_LAYOUTRTL } else { WINDOW_EX_STYLE(0) };
        st().help_wnd = CreateWindowExW(ex, HELP_CLASS, PCWSTR::null(), WS_POPUP | WS_BORDER, 0, 0, 10, 10, Some(main), None, Some(st().inst), None)
            .unwrap_or_default();
    }
    let h = st().help_wnd;
    let mut client = RECT::default();
    let _ = GetClientRect(main, &mut client);
    let pad = s(12);
    let width = if anchor.is_some() { s(420) } else { s(640) }.min(client.right - s(40)).max(s(200));
    let dc = GetDC(Some(h));
    let of = SelectObject(dc, st().font.into());
    let mut w: Vec<u16> = text.encode_utf16().collect();
    let mut calc = rect(0, 0, width - 2 * pad, 0);
    DrawTextW(dc, &mut w, &mut calc, help_format() | DT_CALCRECT);
    SelectObject(dc, of);
    ReleaseDC(Some(h), dc);
    let footer_top = st().rc_footer.top;
    let height = (calc.bottom + 2 * pad + 2).min(footer_top - s(20)).max(s(40));
    let mut r = match anchor {
        Some(a) => {
            let x = a.left.clamp(s(10), (client.right - width - s(10)).max(s(10)));
            rect(x, footer_top - height - s(6), x + width, footer_top - s(6))
        }
        None => {
            let x = (client.right - width) / 2;
            rect(x, footer_top - height - s(6), x + width, footer_top - s(6))
        }
    };
    let mut pts = [POINT { x: r.left, y: r.top }, POINT { x: r.right, y: r.bottom }];
    MapWindowPoints(Some(main), None, &mut pts);
    r = rect(pts[0].x.min(pts[1].x), pts[0].y, pts[0].x.max(pts[1].x), pts[1].y);
    let _ = SetWindowPos(h, Some(HWND_TOP), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    let _ = InvalidateRect(Some(h), None, true);
}

unsafe extern "system" fn help_proc(h: HWND, m: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match m {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(h, &mut ps);
            let th = st().th;
            let mut rc = RECT::default();
            let _ = GetClientRect(h, &mut rc);
            fill(dc, &rc, th.panel);
            fill(dc, &rect(rc.left, rc.top, rc.right, rc.top + s(3)), th.accent);
            let of = SelectObject(dc, st().font.into());
            let pad = s(12);
            draw_text(dc, help_text(), rect(pad, pad, rc.right - pad, rc.bottom - pad), th.text, help_format());
            SelectObject(dc, of);
            let _ = EndPaint(h, &ps);
            return LRESULT(0);
        }
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        WM_LBUTTONDOWN => {
            super::hide_help();
            return LRESULT(0);
        }
        _ => {}
    }
    DefWindowProcW(h, m, wp, lp)
}

// ----------------------------------------------------------------------------
// Settings dialog (system look, as in WinPick)
// ----------------------------------------------------------------------------
const IDS_CTRL: i32 = 3001;
const IDS_ALT: i32 = 3002;
const IDS_SHIFT: i32 = 3003;
const IDS_WIN: i32 = 3004;
const IDS_KEY: i32 = 3005;
const IDS_SWATCH: i32 = 3006;
const IDS_COLORBTN: i32 = 3007;
const IDS_AUTOSTART: i32 = 3008;
const IDS_LANG_FILTER: i32 = 3009;
const IDS_LANG_LIST: i32 = 3010;

#[derive(Default)]
struct Dlg {
    font: HFONT,
    swatch: HWND,
    br_swatch: HBRUSH,
    color: COLORREF,
    lang_filter: HWND,
    lang_list: HWND,
    /// Codes of the listed languages (`None` = automatic).
    listed: Vec<Option<&'static str>>,
    chosen: Option<Option<&'static str>>,
}

static mut DLG: Option<Dlg> = None;

fn dlg() -> &'static mut Dlg {
    unsafe { (*std::ptr::addr_of_mut!(DLG)).get_or_insert_with(Dlg::default) }
}

unsafe fn set_swatch(c: COLORREF) {
    let d = dlg();
    d.color = c;
    if !d.br_swatch.is_invalid() {
        let _ = DeleteObject(d.br_swatch.into());
    }
    d.br_swatch = CreateSolidBrush(c);
    if !d.swatch.is_invalid() {
        let _ = InvalidateRect(Some(d.swatch), None, true);
    }
}

fn lang_label(code: Option<&str>) -> String {
    match code {
        None => tr("Automatic (system language)"),
        Some(c) => format!("{} ({c})", i18n::native_name(c)),
    }
}

unsafe fn fill_languages() {
    let d = dlg();
    let filter = get_text(d.lang_filter);
    let mut all: Vec<Option<&'static str>> = vec![None];
    all.extend(i18n::LANGUAGES.iter().map(|(c, _, _)| Some(*c)));
    d.listed = all.into_iter().filter(|c| matches(&filter, &lang_label(*c))).collect();
    send(d.lang_list, LB_RESETCONTENT, 0, 0);
    let chosen = d.chosen.unwrap_or(None);
    for (i, c) in d.listed.iter().enumerate() {
        let t = wide(&lang_label(*c));
        send(d.lang_list, LB_ADDSTRING, 0, t.as_ptr() as isize);
        if *c == chosen {
            send(d.lang_list, LB_SETCURSEL, i, 0);
        }
    }
}

unsafe fn checked(d: HWND, id: i32) -> bool {
    IsDlgButtonChecked(d, id) == BST_CHECKED.0
}

unsafe fn close_settings() {
    let d = st().settings_wnd;
    if d.is_invalid() {
        return;
    }
    enable(st().main, true);
    let _ = DestroyWindow(d);
    st().settings_wnd = HWND::default();
    let _ = SetForegroundWindow(st().main);
}

unsafe fn apply_settings() {
    let d = st().settings_wnd;
    let mut ns = st().set.clone();
    ns.mods = 0;
    for (id, m) in [(IDS_CTRL, MOD_CONTROL.0), (IDS_ALT, MOD_ALT.0), (IDS_SHIFT, MOD_SHIFT.0), (IDS_WIN, MOD_WIN.0)] {
        if checked(d, id) {
            ns.mods |= m;
        }
    }
    if ns.mods == 0 {
        error(&tr("Settings"), &tr("Please select at least one modifier (Ctrl, Alt, Shift or Win)."));
        return;
    }
    let key_cb = GetDlgItem(Some(d), IDS_KEY).unwrap_or_default();
    let ks = send(key_cb, CB_GETCURSEL, 0, 0);
    if ks < 0 {
        error(&tr("Settings"), &tr("Please select a key."));
        return;
    }
    ns.vk = send(key_cb, CB_GETITEMDATA, ks as usize, 0) as u32;
    ns.bg = dlg().color;
    let ls = send(dlg().lang_list, LB_GETCURSEL, 0, 0);
    if ls >= 0 {
        if let Some(c) = dlg().listed.get(ls as usize) {
            ns.language = c.map(String::from);
        }
    }
    let main = st().main;
    let cur = st().set.clone();
    if ns.mods != cur.mods || ns.vk != cur.vk || !st().hotkey_ok {
        if st().hotkey_ok {
            let _ = UnregisterHotKey(Some(main), 1);
        }
        if super::register_hotkey(ns.mods, ns.vk) {
            st().hotkey_ok = true;
        } else {
            st().hotkey_ok = st().hotkey_ok && super::register_hotkey(cur.mods, cur.vk);
            let msg = trf(
                "The hotkey {hotkey} is already used by Windows or another program.\n\nPlease choose a different one.",
                &[("hotkey", &hotkey_text(ns.mods, ns.vk))],
            );
            error(&tr("Settings"), &msg);
            return;
        }
    }
    let lang_changed = ns.language != cur.language;
    let bg_changed = ns.bg != cur.bg;
    st().set = ns;
    store::set_autostart(checked(d, IDS_AUTOSTART));
    let s_ = st().set.clone();
    st().store.set_settings(&s_);
    if !st().store.save() {
        error(&tr("Error"), &tr("The settings could not be saved."));
    }
    close_settings();
    if bg_changed {
        super::apply_theme();
        super::views::apply_colors();
    }
    if lang_changed {
        super::apply_language();
    }
    super::update_tray(windows::Win32::UI::Shell::NIM_MODIFY);
}

/// Keys inside the settings dialog before IsDialogMessage.
pub unsafe fn settings_pre_translate(msg: &MSG) -> bool {
    if msg.message == WM_KEYDOWN {
        let vk = VIRTUAL_KEY(msg.wParam.0 as u16);
        let d = dlg();
        if vk == VK_DOWN && msg.hwnd == d.lang_filter {
            let _ = SetFocus(Some(d.lang_list));
            return true;
        }
        if vk == VK_F && GetKeyState(VK_CONTROL.0 as i32) < 0 {
            let _ = SetFocus(Some(d.lang_filter));
            return true;
        }
    }
    false
}

unsafe extern "system" fn settings_proc(h: HWND, m: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match m {
        WM_COMMAND => {
            let id = loword(wp.0) as i32;
            let code = hiword(wp.0);
            match id {
                x if x == IDOK.0 => apply_settings(),
                x if x == IDCANCEL.0 => close_settings(),
                IDS_COLORBTN => {
                    static mut CUSTOM: [COLORREF; 16] = [COLORREF(0); 16];
                    let custom = &mut *std::ptr::addr_of_mut!(CUSTOM);
                    if custom[0].0 == 0 {
                        let presets = [
                            rgb(28, 30, 36),
                            rgb(18, 18, 20),
                            rgb(32, 40, 56),
                            rgb(40, 28, 48),
                            rgb(24, 44, 36),
                            rgb(60, 60, 64),
                            rgb(236, 238, 242),
                            rgb(250, 250, 250),
                        ];
                        custom[..8].copy_from_slice(&presets);
                    }
                    let mut cc = CHOOSECOLORW {
                        lStructSize: size_of::<CHOOSECOLORW>() as u32,
                        hwndOwner: h,
                        rgbResult: dlg().color,
                        lpCustColors: custom.as_mut_ptr(),
                        Flags: CC_RGBINIT | CC_FULLOPEN,
                        ..Default::default()
                    };
                    if ChooseColorW(&mut cc).as_bool() {
                        set_swatch(cc.rgbResult);
                    }
                }
                IDS_LANG_FILTER if code == EN_CHANGE => fill_languages(),
                IDS_LANG_LIST if code == LBN_SELCHANGE => {
                    let i = send(dlg().lang_list, LB_GETCURSEL, 0, 0);
                    if i >= 0 {
                        dlg().chosen = dlg().listed.get(i as usize).copied();
                    }
                }
                IDS_LANG_LIST if code == LBN_DBLCLK => apply_settings(),
                _ => {}
            }
            return LRESULT(0);
        }
        WM_CTLCOLORSTATIC => {
            if HWND(lp.0 as *mut c_void) == dlg().swatch {
                return LRESULT(dlg().br_swatch.0 as isize);
            }
            let dc = HDC(wp.0 as *mut c_void);
            SetBkColor(dc, COLORREF(GetSysColor(COLOR_BTNFACE)));
            SetTextColor(dc, COLORREF(GetSysColor(COLOR_WINDOWTEXT)));
            return LRESULT(GetSysColorBrush(COLOR_BTNFACE).0 as isize);
        }
        WM_CLOSE => {
            close_settings();
            return LRESULT(0);
        }
        WM_DESTROY => {
            let d = dlg();
            if !d.font.is_invalid() {
                let _ = DeleteObject(d.font.into());
            }
            if !d.br_swatch.is_invalid() {
                let _ = DeleteObject(d.br_swatch.into());
            }
            *d = Dlg::default();
            return LRESULT(0);
        }
        _ => {}
    }
    DefWindowProcW(h, m, wp, lp)
}

pub unsafe fn show_settings(focus_language: bool) {
    if !st().settings_wnd.is_invalid() {
        let _ = SetForegroundWindow(st().settings_wnd);
        if focus_language {
            let _ = SetFocus(Some(dlg().lang_filter));
        }
        return;
    }
    super::hide_help();
    let main = st().main;
    let dpi = st().dpi;
    let sc = |v: i32| mul_div(v, dpi as i32, 96);
    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
    let ex = WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT | if i18n::is_rtl() { WS_EX_LAYOUTRTL } else { WINDOW_EX_STYLE(0) };
    let (cw, ch) = (420, 470);
    let mut rc = rect(0, 0, sc(cw), sc(ch));
    let _ = windows::Win32::UI::HiDpi::AdjustWindowRectExForDpi(&mut rc, style, false, ex, dpi);
    let (ww, wh) = (rc.right - rc.left, rc.bottom - rc.top);
    let mut mr = RECT::default();
    let _ = GetWindowRect(main, &mut mr);
    let (x, y) = (mr.left + (mr.right - mr.left - ww) / 2, mr.top + (mr.bottom - mr.top - wh) / 2);
    let inst = st().inst;
    let title = format!("{APP_NAME} – {}", tr("Settings"));
    let Ok(d) = CreateWindowExW(ex, SETTINGS_CLASS, &hs(&title), style, x, y, ww, wh, Some(main), None, Some(inst), None) else {
        return;
    };
    st().settings_wnd = d;
    *dlg() = Dlg::default();
    let mut ncm = NONCLIENTMETRICSW { cbSize: size_of::<NONCLIENTMETRICSW>() as u32, ..Default::default() };
    let _ = SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS.0, ncm.cbSize, Some(&mut ncm as *mut _ as *mut c_void), 0, dpi);
    dlg().font = CreateFontIndirectW(&ncm.lfMessageFont);
    let font = dlg().font;
    let mk = |cls: PCWSTR, txt: &str, st_: u32, x0: i32, y0: i32, w0: i32, h0: i32, id: i32, ex: u32| -> HWND {
        let c = CreateWindowExW(
            WINDOW_EX_STYLE(ex),
            cls,
            &hs(txt),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(st_),
            sc(x0),
            sc(y0),
            sc(w0),
            sc(h0),
            Some(d),
            Some(HMENU(id as isize as *mut c_void)),
            Some(inst),
            None,
        )
        .unwrap_or_default();
        send(c, WM_SETFONT, font.0 as usize, 1);
        c
    };
    let tab = WS_TABSTOP.0;
    let cb = BS_AUTOCHECKBOX as u32 | tab;
    // Language first: it is what most people come here for.
    mk(w!("STATIC"), &tr("Language:"), SS_LEFT, 12, 12, 396, 20, -1, 0);
    let lf = mk(w!("EDIT"), "", ES_AUTOHSCROLL as u32 | tab, 12, 34, 396, 24, IDS_LANG_FILTER, WS_EX_CLIENTEDGE.0);
    let cue = wide(&tr("Filter…  (Ctrl+F)"));
    send(lf, EM_SETCUEBANNER, 1, cue.as_ptr() as isize);
    let ll = mk(w!("LISTBOX"), "", (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32 | WS_VSCROLL.0 | tab, 12, 62, 396, 150, IDS_LANG_LIST, WS_EX_CLIENTEDGE.0);
    dlg().lang_filter = lf;
    dlg().lang_list = ll;
    mk(w!("BUTTON"), &tr("Hotkey"), BS_GROUPBOX as u32, 12, 222, 396, 88, -1, 0);
    mk(w!("BUTTON"), &tr("Ctrl"), cb | WS_GROUP.0, 26, 246, 70, 20, IDS_CTRL, 0);
    mk(w!("BUTTON"), "Alt", cb, 102, 246, 60, 20, IDS_ALT, 0);
    mk(w!("BUTTON"), &tr("Shift"), cb, 168, 246, 90, 20, IDS_SHIFT, 0);
    mk(w!("BUTTON"), "Win", cb, 264, 246, 60, 20, IDS_WIN, 0);
    mk(w!("STATIC"), &tr("Key:"), SS_LEFT, 26, 278, 70, 20, -1, 0);
    let combo = CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | tab;
    let key_cb = mk(w!("COMBOBOX"), "", combo, 102, 274, 150, 300, IDS_KEY, 0);
    mk(w!("STATIC"), &tr("Background color:"), SS_LEFT, 12, 330, 200, 20, -1, 0);
    let sw = mk(w!("STATIC"), "", SS_SUNKEN, 230, 326, 40, 24, IDS_SWATCH, 0);
    dlg().swatch = sw;
    mk(w!("BUTTON"), &(tr("Change") + " …"), BS_PUSHBUTTON as u32 | tab, 278, 325, 130, 26, IDS_COLORBTN, 0);
    mk(w!("BUTTON"), &tr("Start with Windows"), cb, 12, 370, 396, 20, IDS_AUTOSTART, 0);
    mk(w!("BUTTON"), &tr("OK"), BS_DEFPUSHBUTTON as u32 | tab, 232, 426, 84, 28, IDOK.0, 0);
    mk(w!("BUTTON"), &tr("Cancel"), BS_PUSHBUTTON as u32 | tab, 324, 426, 84, 28, IDCANCEL.0, 0);

    let set = st().set.clone();
    let chk = |id: i32, on: bool| {
        let _ = CheckDlgButton(d, id, if on { BST_CHECKED } else { BST_UNCHECKED });
    };
    chk(IDS_CTRL, set.mods & MOD_CONTROL.0 != 0);
    chk(IDS_ALT, set.mods & MOD_ALT.0 != 0);
    chk(IDS_SHIFT, set.mods & MOD_SHIFT.0 != 0);
    chk(IDS_WIN, set.mods & MOD_WIN.0 != 0);
    let mut keys: Vec<u32> = (0x41..=0x5A).chain(0x30..=0x39).chain(VK_F1.0 as u32..=VK_F12.0 as u32).collect();
    keys.extend([VK_SPACE.0 as u32, VK_OEM_PERIOD.0 as u32, VK_OEM_COMMA.0 as u32, VK_OEM_MINUS.0 as u32, VK_OEM_PLUS.0 as u32]);
    if !keys.contains(&set.vk) {
        keys.push(set.vk);
    }
    for k in keys {
        let name = wide(&key_name(k));
        let idx = send(key_cb, CB_ADDSTRING, 0, name.as_ptr() as isize);
        send(key_cb, CB_SETITEMDATA, idx as usize, k as isize);
        if k == set.vk {
            send(key_cb, CB_SETCURSEL, idx as usize, 0);
        }
    }
    set_swatch(set.bg);
    chk(IDS_AUTOSTART, store::get_autostart().is_some());
    dlg().chosen = Some(set.language.as_deref().and_then(i18n::supported));
    fill_languages();
    enable(main, false);
    let _ = ShowWindow(d, SW_SHOW);
    let _ = SetForegroundWindow(d);
    let _ = SetFocus(Some(if focus_language { ll } else { key_cb }));
}
