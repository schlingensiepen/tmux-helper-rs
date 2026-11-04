//! Controls of the main window: creation, layout with splitters, the seven
//! detail views, owner drawing and notifications.

use super::help;
use super::util::*;
use super::{perform, s, st, Action, Entry, Splitter, View};
use crate::core::display;
use crate::core::i18n::{self, tr, trf};
use crate::core::model::{self, Status};
use crate::core::validate::parse_names;
use std::ffi::c_void;
use std::mem::size_of;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// Control IDs (WM_COMMAND / WM_NOTIFY). 1 and 2 are IDOK/IDCANCEL.
const ID_HOST_FILTER: i32 = 100;
const ID_HOST_LIST: i32 = 101;
const ID_ED_NEW: i32 = 110;
const ID_BTN_ADD: i32 = 111;
const ID_BTN_DEL: i32 = 120;
const ID_BTN_RECONNECT: i32 = 121;
const ID_BTN_SETUP: i32 = 122;
const ID_ED_ERROR: i32 = 123;
const ID_SESS_FILTER: i32 = 130;
const ID_SESS_LIST: i32 = 131;
const ID_BTN_RELOAD: i32 = 132;
const ID_ED_NEWSESS: i32 = 133;
const ID_BTN_DIRECT: i32 = 134;
const ID_BTN_ALL: i32 = 140;
const ID_ED_SUMMARY: i32 = 141;
const ID_BTN_ONE: i32 = 142;
const ID_BTN_PER: i32 = 143;
const ID_DIR_FILTER: i32 = 150;
const ID_DIR_LIST: i32 = 151;
const ID_ED_NAMES: i32 = 152;
const ID_ED_DIR_SUMMARY: i32 = 153;
const ID_BTN_DIR_ONE: i32 = 154;
const ID_BTN_DIR_PER: i32 = 155;
const ID_ALL_FILTER: i32 = 160;
const ID_ALL_LIST: i32 = 161;
const ID_BTN_ALL_ONE: i32 = 162;
const ID_BTN_ALL_PER: i32 = 163;

#[derive(Default)]
pub struct Controls {
    pub host_filter: HWND,
    pub host_list: HWND,
    // New host
    pub lbl_new: HWND,
    pub ed_new: HWND,
    pub btn_add: HWND,
    // Pending / failed
    pub lbl_info: HWND,
    pub ed_error: HWND,
    pub btn_reconnect: HWND,
    pub btn_setup: HWND,
    pub btn_del: HWND,
    // Connected
    pub lbl_sessions: HWND,
    pub sess_filter: HWND,
    pub sess_list: HWND,
    pub btn_reload: HWND,
    pub lbl_newsess: HWND,
    pub ed_newsess: HWND,
    pub btn_direct: HWND,
    pub lbl_validation: HWND,
    // Open
    pub btn_all: HWND,
    pub lbl_summary: HWND,
    pub ed_summary: HWND,
    pub btn_one: HWND,
    pub btn_per: HWND,
    // Direct
    pub lbl_targets: HWND,
    pub dir_filter: HWND,
    pub dir_list: HWND,
    pub lbl_names: HWND,
    pub ed_names: HWND,
    pub lbl_dir_validation: HWND,
    pub ed_dir_summary: HWND,
    pub btn_dir_one: HWND,
    pub btn_dir_per: HWND,
    // Show all
    pub all_filter: HWND,
    pub all_list: HWND,
    pub btn_all_one: HWND,
    pub btn_all_per: HWND,
    /// Last focused control, restored when the window comes back.
    pub last_focus: HWND,
    /// Rows currently shown in the list views (keys for notifications).
    pub sess_rows: Vec<String>,
    pub dir_rows: Vec<String>,
    pub all_rows: Vec<(String, String)>,
}

impl Controls {
    fn all(&self) -> Vec<HWND> {
        vec![
            self.host_filter, self.host_list, self.lbl_new, self.ed_new, self.btn_add, self.lbl_info, self.ed_error,
            self.btn_reconnect, self.btn_setup, self.btn_del, self.lbl_sessions, self.sess_filter, self.sess_list,
            self.btn_reload, self.lbl_newsess, self.ed_newsess, self.btn_direct, self.lbl_validation, self.btn_all,
            self.lbl_summary, self.ed_summary, self.btn_one, self.btn_per, self.lbl_targets, self.dir_filter,
            self.dir_list, self.lbl_names, self.ed_names, self.lbl_dir_validation, self.ed_dir_summary,
            self.btn_dir_one, self.btn_dir_per, self.all_filter, self.all_list, self.btn_all_one, self.btn_all_per,
        ]
    }
    fn buttons(&self) -> Vec<HWND> {
        vec![
            self.btn_add, self.btn_reconnect, self.btn_setup, self.btn_del, self.btn_reload, self.btn_direct,
            self.btn_all, self.btn_one, self.btn_per, self.btn_dir_one, self.btn_dir_per, self.btn_all_one,
            self.btn_all_per,
        ]
    }
    fn filters(&self) -> [(HWND, HWND); 4] {
        [
            (self.host_filter, self.host_list),
            (self.sess_filter, self.sess_list),
            (self.dir_filter, self.dir_list),
            (self.all_filter, self.all_list),
        ]
    }
}

// ----------------------------------------------------------------------------
// Creation
// ----------------------------------------------------------------------------
unsafe fn mk(class: PCWSTR, text: &str, style: u32, ex: u32, id: i32) -> HWND {
    let stt = st();
    let h = CreateWindowExW(
        WINDOW_EX_STYLE(ex),
        class,
        &hs(text),
        WS_CHILD | WINDOW_STYLE(style),
        0,
        0,
        10,
        10,
        Some(stt.main),
        Some(HMENU(id as isize as *mut c_void)),
        Some(stt.inst),
        None,
    )
    .unwrap_or_default();
    send(h, WM_SETFONT, stt.font.0 as usize, 0);
    h
}

unsafe fn mk_label(text: &str) -> HWND {
    mk(w!("STATIC"), text, SS_LEFT | SS_NOPREFIX, 0, -1)
}
unsafe fn mk_button(text: &str, id: i32) -> HWND {
    mk(w!("BUTTON"), text, BS_OWNERDRAW as u32 | WS_TABSTOP.0, 0, id)
}
unsafe fn mk_edit(id: i32, multiline: bool, readonly: bool) -> HWND {
    let mut style = WS_TABSTOP.0 | ES_AUTOHSCROLL as u32;
    if multiline {
        style = WS_TABSTOP.0 | ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_WANTRETURN as u32 | WS_VSCROLL.0;
    }
    if readonly {
        style |= ES_READONLY as u32;
        style &= !WS_TABSTOP.0;
    }
    let h = mk(w!("EDIT"), "", style, WS_EX_CLIENTEDGE.0, id);
    send(h, EM_SETMARGINS, (EC_LEFTMARGIN | EC_RIGHTMARGIN) as usize, (s(4) | s(4) << 16) as isize);
    h
}
unsafe fn mk_filter(id: i32) -> HWND {
    let h = mk_edit(id, false, false);
    let cue = wide(&tr("Filter…  (Ctrl+F)"));
    send(h, EM_SETCUEBANNER, 1, cue.as_ptr() as isize);
    h
}
unsafe fn mk_listview(id: i32, columns: &[&str]) -> HWND {
    let mut style = WS_TABSTOP.0 | LVS_REPORT | LVS_SHOWSELALWAYS | LVS_SINGLESEL;
    if columns.len() < 2 {
        style |= LVS_NOCOLUMNHEADER;
    }
    let h = mk(w!("SysListView32"), "", style, WS_EX_CLIENTEDGE.0, id);
    let ex = LVS_EX_CHECKBOXES | LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER;
    send(h, LVM_SETEXTENDEDLISTVIEWSTYLE, ex as usize, ex as isize);
    let cols: Vec<String> = if columns.is_empty() { vec![String::new()] } else { columns.iter().map(|c| tr(c)).collect() };
    for (i, c) in cols.iter().enumerate() {
        let mut t = wide(c);
        let col = LVCOLUMNW { mask: LVCF_TEXT | LVCF_WIDTH, cx: s(200), pszText: windows::core::PWSTR(t.as_mut_ptr()), ..Default::default() };
        send(h, LVM_INSERTCOLUMNW, i, &col as *const _ as isize);
    }
    h
}

pub unsafe fn create_controls() {
    let mut c = Controls::default();
    let lb = WS_TABSTOP.0 | WS_VSCROLL.0 | (LBS_NOTIFY | LBS_OWNERDRAWFIXED | LBS_NOINTEGRALHEIGHT) as u32;
    c.host_filter = mk_filter(ID_HOST_FILTER);
    c.host_list = mk(w!("LISTBOX"), "", lb, 0, ID_HOST_LIST);
    // New host
    c.lbl_new = mk_label(&tr("SSH target (user@host or alias):"));
    c.ed_new = mk_edit(ID_ED_NEW, false, false);
    let cue = wide(&tr("root@server or ssh-alias"));
    send(c.ed_new, EM_SETCUEBANNER, 1, cue.as_ptr() as isize);
    c.btn_add = mk_button(&tr("Add host"), ID_BTN_ADD);
    // Pending / failed
    c.lbl_info = mk_label("");
    c.ed_error = mk_edit(ID_ED_ERROR, true, true);
    c.btn_reconnect = mk_button(&tr("Reconnect"), ID_BTN_RECONNECT);
    c.btn_setup = mk_button(&tr("Set up host"), ID_BTN_SETUP);
    // Connected
    c.lbl_sessions = mk_label(&tr("Running sessions:"));
    c.sess_filter = mk_filter(ID_SESS_FILTER);
    c.sess_list = mk_listview(ID_SESS_LIST, &[]);
    c.btn_reload = mk_button(&tr("Reload"), ID_BTN_RELOAD);
    c.btn_del = mk_button(&tr("Delete"), ID_BTN_DEL);
    c.lbl_newsess = mk_label(&tr("New sessions (one name per line):"));
    c.ed_newsess = mk_edit(ID_ED_NEWSESS, true, false);
    c.btn_direct = mk_button(&tr("Direct start"), ID_BTN_DIRECT);
    c.lbl_validation = mk_label("");
    // Open
    c.btn_all = mk_button(&tr("Open all detected sessions"), ID_BTN_ALL);
    c.lbl_summary = mk_label(&tr("Selection summary:"));
    c.ed_summary = mk_edit(ID_ED_SUMMARY, true, true);
    c.btn_one = mk_button(&tr("One WT – Open selection"), ID_BTN_ONE);
    c.btn_per = mk_button(&tr("Per host – Open selection"), ID_BTN_PER);
    // Direct
    c.lbl_targets = mk_label(&tr("Target hosts:"));
    c.dir_filter = mk_filter(ID_DIR_FILTER);
    c.dir_list = mk_listview(ID_DIR_LIST, &[]);
    c.lbl_names = mk_label(&tr("Session names (one per line):"));
    c.ed_names = mk_edit(ID_ED_NAMES, true, false);
    c.lbl_dir_validation = mk_label("");
    c.ed_dir_summary = mk_edit(ID_ED_DIR_SUMMARY, true, true);
    c.btn_dir_one = mk_button(&tr("One WT – Execute"), ID_BTN_DIR_ONE);
    c.btn_dir_per = mk_button(&tr("Per host – Execute"), ID_BTN_DIR_PER);
    // Show all
    c.all_filter = mk_filter(ID_ALL_FILTER);
    c.all_list = mk_listview(ID_ALL_LIST, &["Host", "Session"]);
    c.btn_all_one = mk_button(&tr("One WT – Open selection"), ID_BTN_ALL_ONE);
    c.btn_all_per = mk_button(&tr("Per host – Open selection"), ID_BTN_ALL_PER);
    st().c = c;
    show(st().c.host_filter, true);
    show(st().c.host_list, true);
    apply_fonts();
    apply_colors();
}

pub unsafe fn destroy_controls() {
    for h in st().c.all() {
        if !h.is_invalid() {
            let _ = DestroyWindow(h);
        }
    }
    st().c = Controls::default();
    st().entries.clear();
}

pub unsafe fn apply_fonts() {
    let f = st().font;
    for h in st().c.all() {
        send(h, WM_SETFONT, f.0 as usize, 1);
    }
    let ih = st().line_h + s(10);
    send(st().c.host_list, LB_SETITEMHEIGHT, 0, ih as isize);
    for lv in [st().c.sess_list, st().c.dir_list, st().c.all_list] {
        send(lv, WM_SETFONT, f.0 as usize, 1);
    }
}

pub unsafe fn apply_colors() {
    let th = st().th;
    for lv in [st().c.sess_list, st().c.dir_list, st().c.all_list] {
        send(lv, LVM_SETBKCOLOR, 0, th.field.0 as isize);
        send(lv, LVM_SETTEXTBKCOLOR, 0, th.field.0 as isize);
        send(lv, LVM_SETTEXTCOLOR, 0, th.text.0 as isize);
    }
    let theme = if th.dark { w!("DarkMode_Explorer") } else { w!("Explorer") };
    for h in [st().c.sess_list, st().c.dir_list, st().c.all_list, st().c.host_list, st().c.ed_newsess, st().c.ed_names, st().c.ed_summary, st().c.ed_dir_summary, st().c.ed_error] {
        let _ = SetWindowTheme(h, theme, PCWSTR::null());
    }
    let _ = InvalidateRect(Some(st().main), None, true);
    for h in st().c.all() {
        let _ = InvalidateRect(Some(h), None, true);
    }
}

// ----------------------------------------------------------------------------
// Host list
// ----------------------------------------------------------------------------
pub unsafe fn refresh_host_list() {
    let lb = st().c.host_list;
    if lb.is_invalid() {
        return;
    }
    let entries = super::build_entries(&st().model);
    let sel = st().sel.clone();
    let idx = entries.iter().position(|e| *e == sel).unwrap_or_else(|| entries.iter().position(|e| *e == Entry::Open).unwrap_or(0));
    let same = entries == st().entries;
    st().entries = entries.clone();
    if !same {
        send(lb, WM_SETREDRAW, 0, 0);
        send(lb, LB_RESETCONTENT, 0, 0);
        for _ in &entries {
            send(lb, LB_ADDSTRING, 0, 0);
        }
        send(lb, WM_SETREDRAW, 1, 0);
    }
    send(lb, LB_SETCURSEL, idx, 0);
    st().last_sel_index = idx as i32;
    let _ = InvalidateRect(Some(lb), None, true);
}

/// Display texts of the hosts, truncated to the list width (SPEC-001 §3.1).
unsafe fn host_texts() -> Vec<(String, String)> {
    let m = &st().model;
    let names: Vec<String> = m.hosts.iter().map(|h| h.display.clone()).collect();
    let mut rc = RECT::default();
    let _ = GetClientRect(st().c.host_list, &mut rc);
    let max_chars = ((rc.right - rc.left - s(28)) / st().char_w).max(8) as usize;
    let (t, _) = display::compute(&names, Some(max_chars));
    m.hosts.iter().map(|h| h.target.clone()).zip(t).collect()
}

unsafe fn on_host_sel_change() {
    let lb = st().c.host_list;
    let mut i = send(lb, LB_GETCURSEL, 0, 0) as i32;
    let entries = st().entries.clone();
    if i < 0 || i as usize >= entries.len() {
        return;
    }
    if entries[i as usize] == Entry::Sep {
        // Skip separators in the direction of travel (REP-007 A11).
        let dir = if i > st().last_sel_index { 1 } else { -1 };
        let mut j = i + dir;
        while j >= 0 && (j as usize) < entries.len() && entries[j as usize] == Entry::Sep {
            j += dir;
        }
        if j < 0 || j as usize >= entries.len() {
            j = st().last_sel_index.max(0);
        }
        i = j;
        send(lb, LB_SETCURSEL, i as usize, 0);
    }
    st().last_sel_index = i;
    st().sel = entries[i as usize].clone();
    show_view();
}

// ----------------------------------------------------------------------------
// Views
// ----------------------------------------------------------------------------
unsafe fn view_controls(v: View) -> Vec<HWND> {
    let c = &st().c;
    match v {
        View::NewHost => vec![c.lbl_new, c.ed_new, c.btn_add],
        View::Pending => vec![c.lbl_info, c.btn_del],
        View::Failed => vec![c.lbl_info, c.ed_error, c.btn_reconnect, c.btn_setup, c.btn_del],
        View::Connected => vec![
            c.lbl_sessions, c.sess_filter, c.sess_list, c.btn_reload, c.btn_del, c.lbl_newsess, c.ed_newsess,
            c.btn_direct, c.lbl_validation,
        ],
        View::Open => vec![c.btn_all, c.lbl_summary, c.ed_summary, c.btn_one, c.btn_per],
        View::Direct => vec![
            c.lbl_targets, c.dir_filter, c.dir_list, c.lbl_names, c.ed_names, c.lbl_dir_validation, c.ed_dir_summary,
            c.btn_dir_one, c.btn_dir_per,
        ],
        View::ShowAll => vec![c.all_filter, c.all_list, c.btn_all_one, c.btn_all_per],
    }
}

/// Shows the view for the current selection (keeps the focus where it is,
/// unless the focused control disappears).
pub unsafe fn show_view() {
    let old = st().view;
    let new = super::view_for(&st().sel, &st().model);
    let focus = GetFocus();
    st().view = new;
    let keep = view_controls(new);
    for h in st().c.all() {
        if h == st().c.host_filter || h == st().c.host_list {
            continue;
        }
        show(h, keep.contains(&h));
    }
    populate();
    layout();
    if old != new && !focus.is_invalid() && focus != st().c.host_list && focus != st().c.host_filter && !keep.contains(&focus) {
        focus_detail();
    }
    super::hide_help();
    let _ = InvalidateRect(Some(st().main), Some(&st().rc_footer), false);
}

/// Refreshes the data of the current view after a model change. If a host
/// changed status and is selected, the view itself may change.
pub unsafe fn refresh_view_data(_target: Option<&str>) {
    let want = super::view_for(&st().sel, &st().model);
    if want != st().view {
        show_view();
    } else {
        populate();
    }
}

unsafe fn lv_fill(lv: HWND, rows: &[(Vec<String>, bool)], colors_key: bool) {
    let stt = st();
    stt.syncing = true;
    let focused = send(lv, LVM_GETNEXTITEM, usize::MAX, LVNI_FOCUSED as isize) as i32;
    let count = send(lv, LVM_GETITEMCOUNT, 0, 0) as usize;
    send(lv, WM_SETREDRAW, 0, 0);
    if count != rows.len() {
        send(lv, LVM_DELETEALLITEMS, 0, 0);
    }
    for (i, (cols, checked)) in rows.iter().enumerate() {
        let mut t = wide(&cols[0]);
        let mut item = LVITEMW { mask: LVIF_TEXT, iItem: i as i32, iSubItem: 0, pszText: windows::core::PWSTR(t.as_mut_ptr()), ..Default::default() };
        if count != rows.len() {
            send(lv, LVM_INSERTITEMW, 0, &item as *const _ as isize);
        } else {
            send(lv, LVM_SETITEMTEXTW, i, &mut item as *mut _ as isize);
        }
        for (k, col) in cols.iter().enumerate().skip(1) {
            let mut t = wide(col);
            let mut sub = LVITEMW { iSubItem: k as i32, pszText: windows::core::PWSTR(t.as_mut_ptr()), ..Default::default() };
            send(lv, LVM_SETITEMTEXTW, i, &mut sub as *mut _ as isize);
        }
        let state = if *checked { 2u32 << 12 } else { 1u32 << 12 };
        let st_item = LVITEMW { stateMask: LVIS_STATEIMAGEMASK, state: LIST_VIEW_ITEM_STATE_FLAGS(state), ..Default::default() };
        send(lv, LVM_SETITEMSTATE, i, &st_item as *const _ as isize);
    }
    let n = rows.len() as i32;
    if n > 0 {
        let f = if focused >= 0 && focused < n { focused } else { 0 };
        let fs = LIST_VIEW_ITEM_STATE_FLAGS(LVIS_FOCUSED.0 | LVIS_SELECTED.0);
        let it = LVITEMW { stateMask: fs, state: fs, ..Default::default() };
        send(lv, LVM_SETITEMSTATE, f as usize, &it as *const _ as isize);
    }
    let _ = colors_key;
    send(lv, WM_SETREDRAW, 1, 0);
    let _ = InvalidateRect(Some(lv), None, true);
    st().syncing = false;
}

unsafe fn fit_columns(lv: HWND, n: usize) {
    let mut rc = RECT::default();
    let _ = GetClientRect(lv, &mut rc);
    let w = (rc.right - rc.left).max(s(60));
    if n == 1 {
        send(lv, LVM_SETCOLUMNWIDTH, 0, w as isize);
    } else {
        send(lv, LVM_SETCOLUMNWIDTH, 0, (w * 55 / 100) as isize);
        send(lv, LVM_SETCOLUMNWIDTH, 1, (w - w * 55 / 100) as isize);
    }
}

unsafe fn set_btn(h: HWND, on: bool) {
    enable(h, on);
    let _ = InvalidateRect(Some(h), None, true);
}

/// Fills the controls of the current view from the model.
pub unsafe fn populate() {
    let c_host = match &st().sel {
        Entry::Host(t) => Some(t.clone()),
        _ => None,
    };
    match st().view {
        View::NewHost => {}
        View::Pending => {
            let t = c_host.unwrap_or_default();
            let d = st().model.host(&t).map(|h| h.display.clone()).unwrap_or_default();
            set_text(st().c.lbl_info, &format!("{d}\n\n{}", tr("Connecting…")));
        }
        View::Failed => {
            let t = c_host.unwrap_or_default();
            let (d, e) = st().model.host(&t).map(|h| (h.display.clone(), h.error.clone())).unwrap_or_default();
            set_text(st().c.lbl_info, &format!("{d}\n{}", tr("Status: connection failed")));
            set_text(st().c.ed_error, &crlf(&e));
        }
        View::Connected => {
            let t = c_host.unwrap_or_default();
            let Some(h) = st().model.host(&t).cloned() else { return };
            set_text(st().c.sess_filter, &h.filter);
            let vis = h.visible_sessions();
            st().c.sess_rows = vis.clone();
            let rows: Vec<(Vec<String>, bool)> = vis.iter().map(|s| (vec![s.clone()], h.selected.contains(s))).collect();
            lv_fill(st().c.sess_list, &rows, false);
            let cur = get_text(st().c.ed_newsess).replace("\r\n", "\n");
            if cur != h.new_text {
                set_text(st().c.ed_newsess, &crlf(&h.new_text));
            }
            update_validation(&h);
        }
        View::Open => {
            let entries = st().model.open_entries();
            let mut lines: Vec<String> = Vec::new();
            if st().open_all_waiting {
                lines.push(tr("Waiting for pending connections..."));
                lines.push(String::new());
            }
            let (ex, nw) = (tr("(existing)"), tr("(new)"));
            let mut total = 0;
            for e in &entries {
                lines.push(format!("{}:", e.display));
                for s in &e.existing {
                    lines.push(format!("    - {s} {ex}"));
                }
                for s in e.new.iter().filter(|n| !e.existing.contains(n)) {
                    lines.push(format!("    - {s} {nw}"));
                }
                total += e.existing.len() + e.new.iter().filter(|n| !e.existing.contains(n)).count();
            }
            if entries.is_empty() {
                lines.push(tr("(no selection)"));
            } else {
                lines.push(trf("Total: {total} session(s) on {hosts} host(s)", &[("total", &total.to_string()), ("hosts", &entries.len().to_string())]));
            }
            set_text(st().c.ed_summary, &crlf(&lines.join("\n")));
            set_btn(st().c.btn_one, !entries.is_empty());
            set_btn(st().c.btn_per, !entries.is_empty());
        }
        View::Direct => {
            let m = &st().model;
            set_text(st().c.dir_filter, &m.direct_filter.clone());
            let vis = m.direct_visible();
            let rows: Vec<(Vec<String>, bool)> = vis
                .iter()
                .map(|t| (vec![m.host(t).map(|h| h.display.clone()).unwrap_or_default()], m.direct_hosts.contains(t)))
                .collect();
            st().c.dir_rows = vis;
            lv_fill(st().c.dir_list, &rows, true);
            let text = st().model.direct_text.clone();
            if get_text(st().c.ed_names).replace("\r\n", "\n") != text {
                set_text(st().c.ed_names, &crlf(&text));
            }
            update_direct_summary();
        }
        View::ShowAll => {
            set_text(st().c.all_filter, &st().model.all_filter.clone());
            let rows = st().model.all_visible_rows();
            let any_connected = st().model.hosts.iter().any(|h| h.status == Status::Connected);
            st().c.all_rows = rows.iter().map(|r| (r.target.clone(), r.session.clone())).collect();
            let mut data: Vec<(Vec<String>, bool)> = rows
                .iter()
                .map(|r| (vec![r.display.clone(), r.session.clone()], st().model.all_selected.contains(&(r.target.clone(), r.session.clone()))))
                .collect();
            if !any_connected {
                data = vec![(vec![tr("Waiting for connections..."), String::new()], false)];
                st().c.all_rows.clear();
            }
            lv_fill(st().c.all_list, &data, false);
            set_sort_arrows();
            let any = !st().model.all_selection().is_empty();
            set_btn(st().c.btn_all_one, any);
            set_btn(st().c.btn_all_per, any);
        }
    }
}

unsafe fn update_validation(h: &model::Host) {
    let p = parse_names(&h.new_text);
    let mut msgs: Vec<String> = p.invalid.iter().map(|n| trf("Invalid name: '{name}'", &[("name", n)])).collect();
    for n in &p.valid {
        if h.sessions.contains(n) {
            msgs.push(trf("A session named '{name}' already exists.", &[("name", n)]));
        }
    }
    set_text(st().c.lbl_validation, &msgs.join("\n"));
    set_btn(st().c.btn_direct, !p.valid.is_empty());
    let _ = InvalidateRect(Some(st().c.lbl_validation), None, true);
}

unsafe fn update_direct_summary() {
    let m = &st().model;
    let p = parse_names(&m.direct_text);
    let msgs: Vec<String> = p.invalid.iter().map(|n| trf("Invalid name: '{name}'", &[("name", n)])).collect();
    set_text(st().c.lbl_dir_validation, &msgs.join("\n"));
    let items = m.direct_items();
    let (ex, nw) = (tr("(existing)"), tr("(new)"));
    let mut lines: Vec<String> = items.iter().map(|i| format!("  {} → {} {}", i.name, i.display, if i.exists { &ex } else { &nw })).collect();
    if items.is_empty() {
        lines.push(tr("(no selection)"));
    } else {
        let hosts = m.hosts.iter().filter(|h| m.direct_hosts.contains(&h.target)).count();
        lines.push(String::new());
        lines.push(trf("Total: {total} session(s) on {hosts} host(s)", &[("total", &items.len().to_string()), ("hosts", &hosts.to_string())]));
    }
    set_text(st().c.ed_dir_summary, &crlf(&lines.join("\n")));
    set_btn(st().c.btn_dir_one, !items.is_empty());
    set_btn(st().c.btn_dir_per, !items.is_empty());
}

unsafe fn set_sort_arrows() {
    let hdr = HWND(send(st().c.all_list, LVM_GETHEADER, 0, 0) as *mut c_void);
    for col in 0..2 {
        let mut item = HDITEMW { mask: HDI_FORMAT, ..Default::default() };
        send(hdr, HDM_GETITEMW, col, &mut item as *mut _ as isize);
        let mut fmt = item.fmt.0 & !(HDF_SORTUP.0 | HDF_SORTDOWN.0);
        let key = if col == 0 { model::SortKey::Host } else { model::SortKey::Session };
        if st().model.all_sort == key {
            fmt |= if st().model.all_asc { HDF_SORTUP.0 } else { HDF_SORTDOWN.0 };
        }
        item.fmt = HEADER_CONTROL_FORMAT_FLAGS(fmt);
        send(hdr, HDM_SETITEMW, col, &item as *const _ as isize);
    }
}

// ----------------------------------------------------------------------------
// Layout and splitters (SPEC-003 §4)
// ----------------------------------------------------------------------------
fn split_key() -> Option<(String, Vec<i32>)> {
    let stt = st();
    match stt.view {
        View::Connected => {
            let t = match &stt.sel {
                Entry::Host(t) => t.clone(),
                _ => return None,
            };
            let key = format!("host:{t}");
            let v = stt.store.splits(&key).or_else(|| stt.store.splits("host:*")).unwrap_or_else(|| vec![560]);
            Some((key, v))
        }
        View::Open => Some(("open".into(), st().store.splits("open").unwrap_or_else(|| vec![200]))),
        View::Direct => Some(("direct".into(), st().store.splits("direct").unwrap_or_else(|| vec![380, 680]))),
        _ => None,
    }
}

fn clamp_splits(v: &mut Vec<i32>, n: usize) {
    v.resize(n, 500);
    for x in v.iter_mut() {
        *x = (*x).clamp(100, 900);
    }
    if n == 2 && v[1] < v[0] + 100 {
        v[1] = (v[0] + 100).min(900);
        v[0] = v[0].min(v[1] - 100);
    }
}

pub unsafe fn layout() {
    let stt = st();
    if stt.main.is_invalid() || stt.c.host_list.is_invalid() {
        return;
    }
    let mut rc = RECT::default();
    let _ = GetClientRect(stt.main, &mut rc);
    let lh = stt.line_h;
    let m = s(10);
    let gap = s(8);
    let head_h = lh + s(14);
    let foot_h = lh + s(12);
    stt.rc_header = rect(0, 0, rc.right, head_h);
    stt.rc_footer = rect(0, rc.bottom - foot_h, rc.right, rc.bottom);
    let top = head_h + s(4);
    let bottom = rc.bottom - foot_h - s(4);
    let edit_h = lh + s(10);
    let btn_h = lh + s(14);
    let lbl_h = lh + s(2);
    stt.splitters.clear();

    // Left panel (global width, logical pixels).
    let lw = s(stt.left_w).clamp(s(120), (rc.right - s(260)).max(s(120)));
    let c = &stt.c;
    move_to(c.host_filter, rect(m, top, m + lw, top + edit_h));
    move_to(c.host_list, rect(m, top + edit_h + s(4), m + lw, bottom));
    stt.splitters.push(Splitter { rect: rect(m + lw, top, m + lw + gap, bottom), vertical: true, index: 0, min: s(120), max: rc.right - s(260) });
    let dl = m + lw + gap;
    let dr = rc.right - m;
    let (dt, db) = (top, bottom);
    let row = |h: HWND, y: i32, hh: i32| move_to(h, rect(dl, y, dr, y + hh));
    let buttons = |hs_: &[HWND], y: i32| {
        let n = hs_.len() as i32;
        let w = ((dr - dl) - (n - 1) * gap) / n;
        for (i, h) in hs_.iter().enumerate() {
            let x = dl + i as i32 * (w + gap);
            move_to(*h, rect(x, y, x + w, y + btn_h));
        }
    };
    let split_y = |v: i32| dt + mul_div(db - dt, v, 1000);
    let add_split = |y: i32, idx: usize| {
        st().splitters.push(Splitter { rect: rect(dl, y - gap / 2, dr, y + gap / 2 + 1), vertical: false, index: idx, min: dt, max: db });
    };
    match stt.view {
        View::NewHost => {
            row(c.lbl_new, dt, lbl_h);
            move_to(c.ed_new, rect(dl, dt + lbl_h + s(4), dl + (dr - dl).min(s(460)), dt + lbl_h + s(4) + edit_h));
            move_to(c.btn_add, rect(dl, dt + lbl_h + edit_h + s(14), dl + (dr - dl).min(s(200)), dt + lbl_h + edit_h + s(14) + btn_h));
        }
        View::Pending => {
            row(c.lbl_info, dt, lbl_h * 3);
            move_to(c.btn_del, rect(dl, dt + lbl_h * 3 + gap, dl + (dr - dl).min(s(200)), dt + lbl_h * 3 + gap + btn_h));
        }
        View::Failed => {
            row(c.lbl_info, dt, lbl_h * 2);
            let ey = dt + lbl_h * 2 + s(4);
            let by = db - btn_h;
            move_to(c.ed_error, rect(dl, ey, dr, (by - gap).max(ey + lh)));
            buttons(&[c.btn_reconnect, c.btn_setup, c.btn_del], by);
        }
        View::Connected => {
            let mut v = split_key().map(|k| k.1).unwrap_or_else(|| vec![560]);
            clamp_splits(&mut v, 1);
            let sy = split_y(v[0]);
            row(c.lbl_sessions, dt, lbl_h);
            row(c.sess_filter, dt + lbl_h + s(2), edit_h);
            let ly = dt + lbl_h + edit_h + s(6);
            let by = sy - gap / 2 - btn_h;
            move_to(c.sess_list, rect(dl, ly, dr, (by - s(6)).max(ly + lh)));
            buttons(&[c.btn_reload, c.btn_del], by);
            add_split(sy, 1);
            let ny = sy + gap / 2 + s(2);
            row(c.lbl_newsess, ny, lbl_h);
            let val_h = lbl_h * 2;
            let ey = ny + lbl_h + s(2);
            let bw = (dr - dl).min(s(200)).max(s(120));
            move_to(c.ed_newsess, rect(dl, ey, dr - bw - gap, (db - val_h).max(ey + lh)));
            move_to(c.btn_direct, rect(dr - bw, ey, dr, ey + btn_h));
            move_to(c.lbl_validation, rect(dl, db - val_h, dr, db));
        }
        View::Open => {
            let mut v = split_key().map(|k| k.1).unwrap_or_else(|| vec![200]);
            clamp_splits(&mut v, 1);
            let sy = split_y(v[0]);
            row(c.btn_all, dt, btn_h.max(sy - gap / 2 - dt).min(btn_h * 2));
            add_split(sy, 1);
            let ty = sy + gap / 2 + s(2);
            row(c.lbl_summary, ty, lbl_h);
            let by = db - btn_h;
            move_to(c.ed_summary, rect(dl, ty + lbl_h + s(2), dr, (by - gap).max(ty + lbl_h + lh)));
            buttons(&[c.btn_one, c.btn_per], by);
        }
        View::Direct => {
            let mut v = split_key().map(|k| k.1).unwrap_or_else(|| vec![380, 680]);
            clamp_splits(&mut v, 2);
            let (s1, s2) = (split_y(v[0]), split_y(v[1]));
            row(c.lbl_targets, dt, lbl_h);
            row(c.dir_filter, dt + lbl_h + s(2), edit_h);
            let ly = dt + lbl_h + edit_h + s(6);
            move_to(c.dir_list, rect(dl, ly, dr, (s1 - gap / 2).max(ly + lh)));
            add_split(s1, 1);
            let ny = s1 + gap / 2 + s(2);
            row(c.lbl_names, ny, lbl_h);
            let val_h = lbl_h;
            move_to(c.ed_names, rect(dl, ny + lbl_h + s(2), dr, (s2 - gap / 2 - val_h).max(ny + lbl_h + lh)));
            move_to(c.lbl_dir_validation, rect(dl, s2 - gap / 2 - val_h, dr, s2 - gap / 2));
            add_split(s2, 2);
            let by = db - btn_h;
            move_to(c.ed_dir_summary, rect(dl, s2 + gap / 2 + s(2), dr, (by - gap).max(s2 + lh)));
            buttons(&[c.btn_dir_one, c.btn_dir_per], by);
        }
        View::ShowAll => {
            row(c.all_filter, dt, edit_h);
            let by = db - btn_h;
            move_to(c.all_list, rect(dl, dt + edit_h + s(6), dr, by - gap));
            buttons(&[c.btn_all_one, c.btn_all_per], by);
        }
    }
    fit_columns(st().c.sess_list, 1);
    fit_columns(st().c.dir_list, 1);
    fit_columns(st().c.all_list, 2);
    let _ = InvalidateRect(Some(st().main), None, false);
}

pub unsafe fn drag_splitter(i: usize, x: i32, y: i32, off: i32) {
    let sp = st().splitters[i];
    if sp.index == 0 {
        let m = s(10);
        let px = (x - off - m).clamp(s(120), sp.max.max(s(120)));
        st().left_w = mul_div(px, 96, st().dpi as i32);
    } else {
        let Some((key, mut v)) = split_key() else { return };
        let n = if st().view == View::Direct { 2 } else { 1 };
        clamp_splits(&mut v, n);
        let pos = y - off + (sp.rect.bottom - sp.rect.top) / 2;
        let per_mille = mul_div(pos - sp.min, 1000, (sp.max - sp.min).max(1));
        v[sp.index - 1] = per_mille;
        clamp_splits(&mut v, n);
        st().store.set_splits(&key, &v);
    }
    layout();
    refresh_host_list();
}

/// Ctrl+Shift+arrows move splitters by 5 % (SPEC-003 §4).
pub unsafe fn nudge_splitter(vk: VIRTUAL_KEY) {
    match vk {
        VK_LEFT | VK_RIGHT => {
            let d = if (vk == VK_RIGHT) != crate::core::i18n::is_rtl() { 20 } else { -20 };
            st().left_w = (st().left_w + d).clamp(120, 2000);
        }
        _ => {
            let Some((key, mut v)) = split_key() else { return };
            let n = if st().view == View::Direct { 2 } else { 1 };
            clamp_splits(&mut v, n);
            v[0] += if vk == VK_DOWN { 50 } else { -50 };
            clamp_splits(&mut v, n);
            st().store.set_splits(&key, &v);
        }
    }
    layout();
    refresh_host_list();
    save_splits();
    let _ = st().store.save();
}

pub unsafe fn save_splits() {
    let w = st().left_w;
    st().store.ini.set("Window", "HostListWidth", w.to_string());
    if let (View::Connected, Some((_, v))) = (st().view, split_key()) {
        st().store.set_splits("host:*", &v);
    }
}

pub unsafe fn save_geometry() {
    let mut wp = WINDOWPLACEMENT { length: size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
    if GetWindowPlacement(st().main, &mut wp).is_err() {
        return;
    }
    let r = wp.rcNormalPosition;
    let ini = &mut st().store.ini;
    ini.set("Window", "X", r.left.to_string());
    ini.set("Window", "Y", r.top.to_string());
    ini.set("Window", "W", (r.right - r.left).to_string());
    ini.set("Window", "H", (r.bottom - r.top).to_string());
    ini.set("Window", "Maximized", if wp.showCmd == SW_SHOWMAXIMIZED.0 as u32 { "1" } else { "0" });
    save_splits();
}

/// Restores position and size; returns whether the window was maximized.
pub unsafe fn restore_geometry() -> bool {
    let ini = &st().store.ini;
    let g = (ini.get_i32("Window", "X"), ini.get_i32("Window", "Y"), ini.get_i32("Window", "W"), ini.get_i32("Window", "H"));
    let maximized = ini.get("Window", "Maximized") == Some("1");
    if let (Some(x), Some(y), Some(w), Some(h)) = g {
        let r = rect(x, y, x + w, y + h);
        if w > 100 && h > 100 && !MonitorFromRect(&r, MONITOR_DEFAULTTONULL).is_invalid() {
            let mut wp = WINDOWPLACEMENT { length: size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
            wp.rcNormalPosition = r;
            wp.showCmd = SW_HIDE.0 as u32;
            let _ = SetWindowPlacement(st().main, &wp);
            return maximized;
        }
    }
    // First start: 1100 x 700 logical pixels, centered on the monitor with the cursor.
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
    let mut mi = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    let _ = GetMonitorInfoW(mon, &mut mi);
    let wa = mi.rcWork;
    let (w, h) = (s(1100).min(wa.right - wa.left), s(700).min(wa.bottom - wa.top));
    let _ = SetWindowPos(st().main, None, wa.left + (wa.right - wa.left - w) / 2, wa.top + (wa.bottom - wa.top - h) / 2, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
    false
}

// ----------------------------------------------------------------------------
// Focus
// ----------------------------------------------------------------------------
pub unsafe fn focus_detail() {
    let c = &st().c;
    let target = match st().view {
        View::NewHost => c.ed_new,
        View::Pending => c.host_list,
        View::Failed => c.btn_reconnect,
        View::Connected => c.sess_list,
        View::Open => c.btn_all,
        View::Direct => c.dir_list,
        View::ShowAll => c.all_list,
    };
    let _ = SetFocus(Some(target));
}

pub unsafe fn focus_filter() {
    let c = &st().c;
    let f = match st().view {
        View::Connected => c.sess_filter,
        View::Direct => c.dir_filter,
        View::ShowAll => c.all_filter,
        _ => c.host_filter,
    };
    let f = if GetFocus() == c.host_list { c.host_filter } else { f };
    let _ = SetFocus(Some(f));
    send(f, EM_SETSEL, 0, -1);
}

pub unsafe fn list_of_filter(h: HWND) -> Option<HWND> {
    st().c.filters().iter().find(|(f, _)| *f == h && !h.is_invalid()).map(|(_, l)| *l)
}

pub unsafe fn remember_focus() {
    let f = GetFocus();
    if !f.is_invalid() && IsChild(st().main, f).as_bool() {
        st().c.last_focus = f;
    }
}

pub unsafe fn restore_focus() {
    let f = st().c.last_focus;
    if !f.is_invalid() && IsWindowVisible(f).as_bool() {
        let _ = SetFocus(Some(f));
    } else {
        focus_detail();
    }
}

pub unsafe fn is_button(h: HWND) -> bool {
    !h.is_invalid() && st().c.buttons().contains(&h)
}

pub unsafe fn click(h: HWND) {
    if IsWindowEnabled(h).as_bool() {
        let id = GetDlgCtrlID(h);
        on_command(id, BN_CLICKED, h);
    }
}

// ----------------------------------------------------------------------------
// Selection in list views
// ----------------------------------------------------------------------------
unsafe fn focused_index(lv: HWND) -> Option<usize> {
    let i = send(lv, LVM_GETNEXTITEM, usize::MAX, LVNI_FOCUSED as isize);
    (i >= 0).then_some(i as usize)
}

/// (host, session) of the highlighted row (host view or Show all).
pub unsafe fn focused_pair() -> Option<(String, String)> {
    match st().view {
        View::Connected => {
            let t = match &st().sel {
                Entry::Host(t) => t.clone(),
                _ => return None,
            };
            let i = focused_index(st().c.sess_list)?;
            st().c.sess_rows.get(i).map(|s| (t, s.clone()))
        }
        View::ShowAll => {
            let i = focused_index(st().c.all_list)?;
            st().c.all_rows.get(i).cloned()
        }
        _ => None,
    }
}

pub unsafe fn selection_action(a: Action) {
    let apply = |set: &mut std::collections::BTreeSet<String>, vis: &[String], cur: Option<&String>| match a {
        Action::Toggle => {
            if let Some(k) = cur {
                model::toggle(set, k);
            }
        }
        Action::ToggleAll => model::toggle_all(set, vis),
        _ => model::invert(set, vis),
    };
    match st().view {
        View::Connected => {
            let Entry::Host(t) = st().sel.clone() else { return };
            let vis = st().c.sess_rows.clone();
            let cur = focused_index(st().c.sess_list).and_then(|i| vis.get(i).cloned());
            if let Some(h) = st().model.host_mut(&t) {
                apply(&mut h.selected, &vis, cur.as_ref());
            }
        }
        View::Direct => {
            let vis = st().c.dir_rows.clone();
            let cur = focused_index(st().c.dir_list).and_then(|i| vis.get(i).cloned());
            apply(&mut st().model.direct_hosts, &vis, cur.as_ref());
        }
        View::ShowAll => {
            let vis = st().c.all_rows.clone();
            let cur = focused_index(st().c.all_list).and_then(|i| vis.get(i).cloned());
            let set = &mut st().model.all_selected;
            match a {
                Action::Toggle => {
                    if let Some(k) = cur {
                        model::toggle(set, &k);
                    }
                }
                Action::ToggleAll => model::toggle_all(set, &vis),
                _ => model::invert(set, &vis),
            }
        }
        _ => return,
    }
    populate();
}

// ----------------------------------------------------------------------------
// Commands and notifications
// ----------------------------------------------------------------------------
pub unsafe fn on_command(id: i32, code: u32, _h: HWND) {
    if code == BN_CLICKED {
        let a = match id {
            ID_BTN_ADD => Some(Action::AddHost),
            ID_BTN_DEL => Some(Action::DeleteHost),
            ID_BTN_RECONNECT | ID_BTN_RELOAD => Some(Action::Reload),
            ID_BTN_SETUP => Some(Action::SetupHost),
            ID_BTN_DIRECT => Some(Action::DirectStart),
            ID_BTN_ALL => Some(Action::OpenAll),
            ID_BTN_ONE | ID_BTN_ALL_ONE => Some(Action::OneWindow),
            ID_BTN_DIR_ONE => Some(Action::Execute),
            ID_BTN_PER | ID_BTN_DIR_PER | ID_BTN_ALL_PER => Some(Action::PerHost),
            _ => None,
        };
        if let Some(a) = a {
            perform(a);
        }
        return;
    }
    if id == ID_HOST_LIST && code == LBN_SELCHANGE {
        on_host_sel_change();
        return;
    }
    if id == ID_HOST_LIST && (code == LBN_SETFOCUS || code == LBN_KILLFOCUS) {
        let _ = InvalidateRect(Some(st().c.host_list), None, true);
        return;
    }
    if code == EN_CHANGE {
        let c = &st().c;
        match id {
            ID_HOST_FILTER => {
                st().model.host_filter = get_text(c.host_filter);
                refresh_host_list();
            }
            ID_SESS_FILTER => {
                let f = get_text(c.sess_filter);
                if let Entry::Host(t) = st().sel.clone() {
                    if let Some(h) = st().model.host_mut(&t) {
                        if h.filter != f {
                            h.filter = f;
                            populate();
                        }
                    }
                }
            }
            ID_ED_NEWSESS => {
                let text = get_text(c.ed_newsess).replace("\r\n", "\n");
                if let Entry::Host(t) = st().sel.clone() {
                    if let Some(h) = st().model.host_mut(&t) {
                        h.new_text = text;
                        let h = h.clone();
                        update_validation(&h);
                    }
                }
            }
            ID_DIR_FILTER => {
                let f = get_text(c.dir_filter);
                if st().model.direct_filter != f {
                    st().model.direct_filter = f;
                    populate();
                }
            }
            ID_ED_NAMES => {
                st().model.direct_text = get_text(c.ed_names).replace("\r\n", "\n");
                update_direct_summary();
            }
            ID_ALL_FILTER => {
                let f = get_text(c.all_filter);
                if st().model.all_filter != f {
                    st().model.all_filter = f;
                    populate();
                }
            }
            _ => {}
        }
    }
}

pub unsafe fn on_notify(lp: LPARAM) -> LRESULT {
    let hdr = &*(lp.0 as *const NMHDR);
    let id = hdr.idFrom as i32;
    if ![ID_SESS_LIST, ID_DIR_LIST, ID_ALL_LIST].contains(&id) {
        return LRESULT(0);
    }
    match hdr.code {
        LVN_ITEMCHANGED => {
            let nm = &*(lp.0 as *const NMLISTVIEW);
            if st().syncing || (nm.uOldState ^ nm.uNewState) & LVIS_STATEIMAGEMASK.0 == 0 || nm.iItem < 0 {
                return LRESULT(0);
            }
            let checked = (nm.uNewState & LVIS_STATEIMAGEMASK.0) >> 12 == 2;
            let i = nm.iItem as usize;
            match id {
                ID_SESS_LIST => {
                    if let (Entry::Host(t), Some(sname)) = (st().sel.clone(), st().c.sess_rows.get(i).cloned()) {
                        if let Some(h) = st().model.host_mut(&t) {
                            if checked {
                                h.selected.insert(sname);
                            } else {
                                h.selected.remove(&sname);
                            }
                        }
                    }
                }
                ID_DIR_LIST => {
                    if let Some(t) = st().c.dir_rows.get(i).cloned() {
                        if checked {
                            st().model.direct_hosts.insert(t);
                        } else {
                            st().model.direct_hosts.remove(&t);
                        }
                        update_direct_summary();
                    }
                }
                _ => {
                    if let Some(k) = st().c.all_rows.get(i).cloned() {
                        if checked {
                            st().model.all_selected.insert(k);
                        } else {
                            st().model.all_selected.remove(&k);
                        }
                        let any = !st().model.all_selection().is_empty();
                        set_btn(st().c.btn_all_one, any);
                        set_btn(st().c.btn_all_per, any);
                    }
                }
            }
        }
        LVN_COLUMNCLICK if id == ID_ALL_LIST => {
            let nm = &*(lp.0 as *const NMLISTVIEW);
            perform(if nm.iSubItem == 0 { Action::SortHost } else { Action::SortSession });
        }
        NM_CUSTOMDRAW if id == ID_DIR_LIST => {
            let cd = &mut *(lp.0 as *mut NMLVCUSTOMDRAW);
            match cd.nmcd.dwDrawStage {
                CDDS_PREPAINT => return LRESULT(CDRF_NOTIFYITEMDRAW as isize),
                CDDS_ITEMPREPAINT => {
                    let i = cd.nmcd.dwItemSpec;
                    if let Some(t) = st().c.dir_rows.get(i) {
                        cd.clrText = status_color(st().model.host(t).map(|h| h.status));
                    }
                    return LRESULT(CDRF_DODEFAULT as isize);
                }
                _ => {}
            }
        }
        _ => {}
    }
    LRESULT(0)
}

fn status_color(s: Option<Status>) -> COLORREF {
    let th = st().th;
    match s {
        Some(Status::Connected) => th.green,
        Some(Status::Failed) => th.red,
        _ => th.blue,
    }
}

pub unsafe fn on_ctl_color(m: u32, dc: HDC, h: HWND) -> LRESULT {
    let th = st().th;
    SetTextColor(dc, th.text);
    if m == WM_CTLCOLORLISTBOX {
        SetBkColor(dc, th.panel);
        return LRESULT(st().br_panel.0 as isize);
    }
    if m == WM_CTLCOLOREDIT || is_edit(h) {
        SetBkColor(dc, th.field);
        return LRESULT(st().br_field.0 as isize);
    }
    if h == st().c.lbl_validation || h == st().c.lbl_dir_validation {
        let t = get_text(h);
        let invalid = tr("Invalid name: '{name}'");
        let prefix = invalid.split("{name}").next().unwrap_or("");
        SetTextColor(dc, if !prefix.is_empty() && t.contains(prefix) { th.red } else { th.warn });
    }
    SetBkColor(dc, th.bg);
    LRESULT(st().br_bg.0 as isize)
}

// ----------------------------------------------------------------------------
// Owner drawing: buttons and host list
// ----------------------------------------------------------------------------
pub unsafe fn on_draw_item(di: &DRAWITEMSTRUCT) {
    let th = st().th;
    let dc = di.hDC;
    let r = di.rcItem;
    if di.CtlType == ODT_BUTTON {
        let disabled = di.itemState.0 & ODS_DISABLED.0 != 0;
        let down = di.itemState.0 & ODS_SELECTED.0 != 0;
        let focus = di.itemState.0 & ODS_FOCUS.0 != 0;
        fill(dc, &r, th.bg);
        let face = if down { th.button_down } else { th.button };
        let pen = CreatePen(PS_SOLID, if focus { s(2) } else { 1 }, if focus { th.accent } else { th.line });
        let br = CreateSolidBrush(face);
        let op = SelectObject(dc, pen.into());
        let ob = SelectObject(dc, br.into());
        let rad = s(6);
        let _ = RoundRect(dc, r.left + 1, r.top + 1, r.right - 1, r.bottom - 1, rad, rad);
        SelectObject(dc, op);
        SelectObject(dc, ob);
        let _ = DeleteObject(pen.into());
        let _ = DeleteObject(br.into());
        let text = get_text(di.hwndItem);
        let of = SelectObject(dc, st().font.into());
        let mut tr_ = r;
        tr_.left += s(8);
        tr_.right -= s(8);
        let mut fmt = DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX;
        if i18n::is_rtl() {
            fmt |= DT_RTLREADING;
        }
        draw_text(dc, &text, tr_, if disabled { th.text_dim } else { th.text }, fmt);
        SelectObject(dc, of);
        return;
    }
    if di.CtlType == ODT_LISTBOX {
        let i = di.itemID as usize;
        let Some(e) = st().entries.get(i).cloned() else {
            fill(dc, &r, th.panel);
            return;
        };
        let selected = di.itemState.0 & ODS_SELECTED.0 != 0;
        let focused_list = GetFocus() == st().c.host_list;
        let bg = if selected && e != Entry::Sep { if focused_list { th.select } else { mix(th.panel, th.select, 0.6) } } else { th.panel };
        fill(dc, &r, bg);
        if selected && e != Entry::Sep {
            fill(dc, &rect(r.left, r.top + s(3), r.left + s(3), r.bottom - s(3)), th.accent);
        }
        let mut tr_ = r;
        tr_.left += s(10);
        tr_.right -= s(6);
        let mut fmt = DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX;
        if i18n::is_rtl() {
            fmt |= DT_RTLREADING;
        }
        let of = SelectObject(dc, st().font.into());
        match &e {
            Entry::Sep => {
                let y = (r.top + r.bottom) / 2;
                fill(dc, &rect(r.left + s(8), y, r.right - s(8), y + 1), th.line);
            }
            Entry::Host(t) => {
                let texts = host_texts();
                let txt = texts.iter().find(|(k, _)| k == t).map(|(_, v)| v.clone()).unwrap_or_else(|| t.clone());
                let status = st().model.host(t).map(|h| h.status);
                draw_text(dc, &txt, tr_, status_color(status), fmt);
            }
            other => {
                let txt = match other {
                    Entry::NewHost => tr("+ New host"),
                    Entry::Open => tr(">> Open"),
                    Entry::Direct => tr("Direct session"),
                    _ => tr("Show all"),
                };
                SelectObject(dc, st().font_bold.into());
                draw_text(dc, &txt, tr_, th.text, fmt);
            }
        }
        SelectObject(dc, of);
    }
}

// ----------------------------------------------------------------------------
// Header and footer
// ----------------------------------------------------------------------------
pub unsafe fn paint(dc: HDC) {
    let stt = st();
    let th = stt.th;
    let mut rc = RECT::default();
    let _ = GetClientRect(stt.main, &mut rc);
    // Double buffer the client area (children are clipped). In a mirrored
    // (RTL) window we draw directly, so the mirroring stays with the window DC.
    let rtl_layout = i18n::is_rtl();
    let mem_dc = CreateCompatibleDC(Some(dc));
    let bmp = CreateCompatibleBitmap(dc, rc.right.max(1), rc.bottom.max(1));
    let ob = SelectObject(mem_dc, bmp.into());
    let mem = if rtl_layout { dc } else { mem_dc };
    fill(mem, &rc, th.bg);
    let rtl = if i18n::is_rtl() { DT_RTLREADING } else { DRAW_TEXT_FORMAT(0) };
    // Header
    let h = stt.rc_header;
    fill(mem, &h, th.panel);
    fill(mem, &rect(h.left, h.bottom - 1, h.right, h.bottom), th.line);
    let of = SelectObject(mem, stt.font_bold.into());
    draw_text(mem, super::store::APP_NAME, rect(s(12), h.top, h.right - s(12), h.bottom), th.text, DT_LEFT | DT_VCENTER | DT_SINGLELINE | rtl);
    SelectObject(mem, stt.font.into());
    let code = i18n::language();
    let lang = format!("[{code}] {}", i18n::native_name(code));
    draw_text(mem, &lang, rect(s(12), h.top, h.right - s(12), h.bottom), th.text_dim, DT_RIGHT | DT_VCENTER | DT_SINGLELINE | rtl);
    // Splitter grips
    for sp in &stt.splitters {
        let r = sp.rect;
        if sp.vertical {
            let x = (r.left + r.right) / 2;
            fill(mem, &rect(x, r.top + s(4), x + 1, r.bottom - s(4)), th.line);
        } else {
            let y = (r.top + r.bottom) / 2;
            fill(mem, &rect(r.left + s(4), y, r.right - s(4), y + 1), th.line);
        }
    }
    // Footer
    let f = stt.rc_footer;
    fill(mem, &f, th.panel);
    fill(mem, &rect(f.left, f.top, f.right, f.top + 1), th.line);
    stt.footer_rects.clear();
    let mut x = s(12);
    for item in help::items(stt.view) {
        let key = help::key_label(item.key);
        let name = tr(item.name);
        SelectObject(mem, stt.font_bold.into());
        let kw = text_size(mem, &key).cx;
        SelectObject(mem, stt.font.into());
        let nw = text_size(mem, &name).cx;
        let w = kw + s(5) + nw;
        if x + w > f.right - s(12) {
            break;
        }
        let r = rect(x - s(4), f.top + 1, x + w + s(4), f.bottom);
        if stt.hover == Some(stt.footer_rects.len()) {
            fill(mem, &r, th.button_hot);
        }
        SelectObject(mem, stt.font_bold.into());
        draw_text(mem, &key, rect(x, f.top, x + kw, f.bottom), th.accent, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | rtl);
        SelectObject(mem, stt.font.into());
        draw_text(mem, &name, rect(x + kw + s(5), f.top, x + w, f.bottom), th.text_dim, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | rtl);
        stt.footer_rects.push((r, item));
        x += w + s(18);
    }
    SelectObject(mem, of);
    if !rtl_layout {
        let _ = BitBlt(dc, 0, 0, rc.right, rc.bottom, Some(mem_dc), 0, 0, SRCCOPY);
    }
    SelectObject(mem_dc, ob);
    let _ = DeleteObject(bmp.into());
    let _ = DeleteDC(mem_dc);
}
