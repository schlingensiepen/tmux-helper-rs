//! Win32 user interface (SPEC-003). All UI code runs on one thread; the
//! state lives in a heap-allocated `State` reached through `st()` (pattern
//! from WinPick). Rule: never keep a reference from `st()` across a call
//! that can send a window message to our own windows; call `st()` again.

mod dialogs;
mod help;
mod store;
mod theme;
mod util;
mod views;
mod worker;

use crate::core::i18n::{self, tr, trf};
use crate::core::model::{Model, SortKey, Source, Status};
use crate::core::sshconfig;
use crate::core::validate::parse_names;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::null_mut;
use store::{Settings, Store, APP_NAME};
use theme::Theme;
use util::*;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const MAIN_CLASS: PCWSTR = w!("OpenTmuxTabsMain");
const HOTKEY_ID: i32 = 1;
const WM_APP_TRAY: u32 = WM_APP + 1;
const WM_APP_SHOW: u32 = WM_APP + 2;
const WM_APP_SETTINGS: u32 = WM_APP + 3;
const TIMER_HOVER: usize = 1;
const IDM_OPEN: usize = 2001;
const IDM_SETTINGS: usize = 2002;
const IDM_ABOUT: usize = 2003;
const IDM_EXIT: usize = 2004;

/// What the host list on the left shows, top to bottom.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Entry {
    NewHost,
    Sep,
    Host(String),
    Open,
    Direct,
    ShowAll,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    NewHost,
    Pending,
    Failed,
    Connected,
    Open,
    Direct,
    ShowAll,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Quit,
    Hide,
    NewHost,
    AddHost,
    GotoOpen,
    OpenAll,
    GotoDirect,
    GotoShowAll,
    Settings,
    SettingsLanguage,
    DeleteHost,
    Toggle,
    ToggleAll,
    Invert,
    JumpStart,
    Reload,
    SetupHost,
    DirectStart,
    OneWindow,
    PerHost,
    Execute,
    SortHost,
    SortSession,
    Filter,
    Help,
    ReloadConfig,
}

/// A splitter the user can drag. `index` 0 is the global one between the
/// host list and the detail area; 1 and 2 are the inner ones of a view.
#[derive(Clone, Copy, Default)]
pub struct Splitter {
    pub rect: RECT,
    pub vertical: bool,
    pub index: usize,
    /// Pixel range the splitter position may take (for inner ones: the
    /// detail area's top and bottom).
    pub min: i32,
    pub max: i32,
}

pub struct State {
    pub inst: HINSTANCE,
    pub main: HWND,
    pub c: views::Controls,
    pub model: Model,
    pub store: Store,
    pub set: Settings,
    pub th: Theme,
    pub br_bg: HBRUSH,
    pub br_panel: HBRUSH,
    pub br_field: HBRUSH,
    pub dpi: u32,
    pub font: HFONT,
    pub font_bold: HFONT,
    pub line_h: i32,
    pub char_w: i32,
    pub entries: Vec<Entry>,
    pub sel: Entry,
    pub view: View,
    pub left_w: i32,
    pub rc_header: RECT,
    pub rc_footer: RECT,
    pub splitters: Vec<Splitter>,
    pub drag: Option<(usize, i32)>,
    pub footer_rects: Vec<(RECT, help::Item)>,
    pub hover: Option<usize>,
    pub help_wnd: HWND,
    pub help_all: bool,
    pub syncing: bool,
    pub open_all_waiting: bool,
    pub hotkey_ok: bool,
    pub nid: NOTIFYICONDATAW,
    pub msg_taskbar_created: u32,
    pub settings_wnd: HWND,
    pub last_sel_index: i32,
}

static mut STATE: *mut State = null_mut();

pub fn st() -> &'static mut State {
    unsafe { &mut *STATE }
}

/// Scales a value in logical pixels (96 dpi) to the current DPI.
pub fn s(v: i32) -> i32 {
    let d = st().dpi;
    mul_div(v, if d == 0 { 96 } else { d as i32 }, 96)
}

// ----------------------------------------------------------------------------
// Fonts and theme
// ----------------------------------------------------------------------------
unsafe fn create_fonts() {
    let stt = st();
    for f in [stt.font, stt.font_bold] {
        if !f.is_invalid() {
            let _ = DeleteObject(f.into());
        }
    }
    let mut ncm = NONCLIENTMETRICSW {
        cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    let _ = SystemParametersInfoForDpi(
        SPI_GETNONCLIENTMETRICS.0,
        ncm.cbSize,
        Some(&mut ncm as *mut _ as *mut c_void),
        0,
        stt.dpi,
    );
    let mut lf = ncm.lfMessageFont;
    stt.font = CreateFontIndirectW(&lf);
    lf.lfWeight = FW_SEMIBOLD.0 as i32;
    stt.font_bold = CreateFontIndirectW(&lf);
    let dc = GetDC(None);
    let old = SelectObject(dc, stt.font.into());
    let mut tm = TEXTMETRICW::default();
    let _ = GetTextMetricsW(dc, &mut tm);
    stt.line_h = tm.tmHeight;
    stt.char_w = tm.tmAveCharWidth.max(1);
    SelectObject(dc, old);
    ReleaseDC(None, dc);
}

unsafe fn apply_theme() {
    let stt = st();
    stt.th = theme::build(stt.set.bg);
    for b in [stt.br_bg, stt.br_panel, stt.br_field] {
        if !b.is_invalid() {
            let _ = DeleteObject(b.into());
        }
    }
    stt.br_bg = CreateSolidBrush(stt.th.bg);
    stt.br_panel = CreateSolidBrush(stt.th.panel);
    stt.br_field = CreateSolidBrush(stt.th.field);
    if !stt.main.is_invalid() {
        let dark = BOOL::from(stt.th.dark);
        let _ = DwmSetWindowAttribute(
            stt.main,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as *const c_void,
            size_of::<BOOL>() as u32,
        );
    }
}

use windows::core::BOOL;

// ----------------------------------------------------------------------------
// Host list entries and view selection
// ----------------------------------------------------------------------------
pub fn build_entries(m: &Model) -> Vec<Entry> {
    let mut e = vec![Entry::NewHost, Entry::Sep];
    for i in m.visible_hosts() {
        e.push(Entry::Host(m.hosts[i].target.clone()));
    }
    e.extend([Entry::Open, Entry::Sep, Entry::Direct, Entry::ShowAll]);
    e
}

pub fn view_for(sel: &Entry, m: &Model) -> View {
    match sel {
        Entry::NewHost | Entry::Sep => View::NewHost,
        Entry::Open => View::Open,
        Entry::Direct => View::Direct,
        Entry::ShowAll => View::ShowAll,
        Entry::Host(t) => match m.host(t).map(|h| h.status) {
            Some(Status::Connected) => View::Connected,
            Some(Status::Failed) => View::Failed,
            Some(Status::Pending) => View::Pending,
            None => View::Open,
        },
    }
}

/// Selects an entry of the host list and shows its view.
pub unsafe fn select(e: Entry, focus_detail: bool) {
    st().sel = e;
    views::refresh_host_list();
    views::show_view();
    if focus_detail {
        views::focus_detail();
    }
}

fn selected_host() -> Option<String> {
    match &st().sel {
        Entry::Host(t) => Some(t.clone()),
        _ => None,
    }
}

// ----------------------------------------------------------------------------
// Connections
// ----------------------------------------------------------------------------
pub unsafe fn start_connect(target: &str) {
    let Some(h) = st().model.host_mut(target) else { return };
    h.generation += 1;
    h.status = Status::Pending;
    let generation = h.generation;
    worker::connect(target.to_string(), generation);
}

unsafe fn load_config_hosts() -> Vec<String> {
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    if home.is_empty() {
        return Vec::new();
    }
    sshconfig::read_hosts(std::path::Path::new(&home))
}

/// (Re)builds the host list from the SSH config and the INI and connects
/// to all hosts.
unsafe fn reload_hosts() {
    let config = load_config_hosts();
    let manual = if st().model.hosts.is_empty() {
        st().store.manual_hosts()
    } else {
        st().model.manual_targets()
    };
    let hidden = if st().model.hosts.is_empty() {
        st().store.hidden_hosts()
    } else {
        st().model.hidden.clone()
    };
    st().model.load_hosts(&config, &manual, &hidden);
    let stt = st();
    stt.store.apply_usage(&mut stt.model);
    let targets: Vec<String> = st().model.hosts.iter().map(|h| h.target.clone()).collect();
    for t in &targets {
        worker::resolve(t.clone());
        start_connect(t);
    }
    if let Entry::Host(t) = &st().sel {
        if st().model.host(t).is_none() {
            st().sel = Entry::Open;
        }
    }
    views::refresh_host_list();
    views::show_view();
}

unsafe fn save_store() {
    let stt = st();
    stt.store.put_hosts(&stt.model);
    stt.store.set_settings(&stt.set);
    if !stt.store.save() {
        dialogs::error(&tr("Error"), &tr("The settings could not be saved."));
    }
}

unsafe fn handle_events() {
    for e in worker::drain() {
        match e {
            worker::Event::Connected {
                target,
                generation,
                result,
            } => {
                let Some(h) = st().model.host_mut(&target) else {
                    continue;
                };
                if h.generation != generation {
                    continue;
                }
                match result {
                    Ok(sessions) => {
                        h.status = Status::Connected;
                        h.selected.retain(|s| sessions.contains(s));
                        h.sessions = sessions;
                        h.error.clear();
                    }
                    Err(err) => {
                        h.status = Status::Failed;
                        h.sessions.clear();
                        h.selected.clear();
                        h.error = err;
                    }
                }
                let alive: Vec<(String, String)> = st()
                    .model
                    .all_rows()
                    .into_iter()
                    .map(|r| (r.target, r.session))
                    .collect();
                st().model.all_selected.retain(|k| k.0 != target || alive.contains(k));
                views::refresh_host_list();
                views::refresh_view_data(Some(&target));
                if st().open_all_waiting && !st().model.any_pending() {
                    st().open_all_waiting = false;
                    do_open_all();
                }
            }
            worker::Event::Resolved { target, display } => {
                if let Some(h) = st().model.host_mut(&target) {
                    h.display = display;
                }
                views::refresh_host_list();
                views::refresh_view_data(Some(&target));
            }
            worker::Event::JobDone(r) => {
                let now = now_secs();
                for t in &r.opened {
                    st().model.record_usage(t, now);
                }
                for t in &r.refresh {
                    start_connect(t);
                }
                save_store();
                views::refresh_host_list();
                views::refresh_view_data(None);
                if !r.errors.is_empty() {
                    let mut title = tr("Error");
                    let mut parts = Vec::new();
                    for (t, msg) in &r.errors {
                        let name = st()
                            .model
                            .host(t)
                            .map(|h| h.display.clone())
                            .unwrap_or_else(|| t.clone());
                        if r.errors.len() == 1 && !t.is_empty() {
                            title = trf("Error on {name}", &[("name", &name)]);
                            parts.push(msg.clone());
                        } else if t.is_empty() {
                            parts.push(msg.clone());
                        } else {
                            parts.push(format!("{}\n{msg}", trf("Error on {name}", &[("name", &name)])));
                        }
                    }
                    dialogs::error(&title, &parts.join("\n\n"));
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Actions
// ----------------------------------------------------------------------------
unsafe fn do_open_all() {
    let open = st().model.all_detected();
    let targets: Vec<String> = open.iter().map(|(t, _)| t.clone()).collect();
    st().model.reset_hosts(&targets);
    worker::launch(worker::LaunchJob {
        reconnect: vec![],
        create: vec![],
        open,
        single_window: false,
        refresh: targets,
    });
    views::refresh_view_data(None);
}

unsafe fn launch_pairs(pairs: Vec<(String, String)>, single: bool) {
    if pairs.is_empty() {
        return;
    }
    let open = crate::core::wt::group_by_host(&pairs);
    worker::launch(worker::LaunchJob {
        reconnect: vec![],
        create: vec![],
        open,
        single_window: single,
        refresh: vec![],
    });
}

unsafe fn open_selection(single: bool) {
    let entries = st().model.open_entries();
    if entries.is_empty() {
        return;
    }
    let targets: Vec<String> = entries.iter().map(|e| e.target.clone()).collect();
    let create = entries
        .iter()
        .filter(|e| !e.new.is_empty())
        .map(|e| (e.target.clone(), e.new.clone()))
        .collect();
    let open = entries
        .iter()
        .map(|e| {
            let mut v = e.existing.clone();
            v.extend(e.new.iter().filter(|n| !e.existing.contains(n)).cloned());
            (e.target.clone(), v)
        })
        .collect();
    st().model.reset_hosts(&targets);
    worker::launch(worker::LaunchJob {
        reconnect: vec![],
        create,
        open,
        single_window: single,
        refresh: targets,
    });
    views::refresh_view_data(None);
}

unsafe fn execute_direct(single: bool) {
    let items = st().model.direct_items();
    if items.is_empty() {
        return;
    }
    let mut open: Vec<(String, Vec<String>)> = Vec::new();
    let mut create: Vec<(String, Vec<String>)> = Vec::new();
    for it in &items {
        match open.iter_mut().find(|(t, _)| *t == it.target) {
            Some((_, v)) => v.push(it.name.clone()),
            None => open.push((it.target.clone(), vec![it.name.clone()])),
        }
        if !it.exists {
            match create.iter_mut().find(|(t, _)| *t == it.target) {
                Some((_, v)) => v.push(it.name.clone()),
                None => create.push((it.target.clone(), vec![it.name.clone()])),
            }
        }
    }
    let targets: Vec<String> = open.iter().map(|(t, _)| t.clone()).collect();
    let reconnect = targets
        .iter()
        .filter(|t| st().model.host(t).map(|h| h.status == Status::Failed).unwrap_or(false))
        .cloned()
        .collect();
    worker::launch(worker::LaunchJob {
        reconnect,
        create,
        open,
        single_window: single,
        refresh: targets,
    });
}

unsafe fn direct_start() {
    let Some(t) = selected_host() else { return };
    let Some(h) = st().model.host(&t) else { return };
    if h.status != Status::Connected {
        return;
    }
    let names = parse_names(&h.new_text).valid;
    if names.is_empty() {
        return;
    }
    st().model.host_mut(&t).unwrap().new_text.clear();
    worker::launch(worker::LaunchJob {
        reconnect: vec![],
        create: vec![(t.clone(), names.clone())],
        open: vec![(t.clone(), names)],
        single_window: true,
        refresh: vec![t],
    });
    views::refresh_view_data(None);
}

unsafe fn add_host() {
    let target = get_text(st().c.ed_new).trim().to_string();
    if target.is_empty() {
        return;
    }
    if !st().model.add_manual(&target) {
        select(Entry::Host(target), false);
        return;
    }
    set_text(st().c.ed_new, "");
    worker::resolve(target.clone());
    start_connect(&target);
    save_store();
    select(Entry::Host(target), false);
}

unsafe fn delete_host() {
    let Some(t) = selected_host() else { return };
    let Some(h) = st().model.host(&t) else { return };
    let q = if h.source == Source::Config {
        trf(
            "Hide host '{name}'? It stays in your SSH config file; add it again with \"New host\".",
            &[("name", &h.display)],
        )
    } else {
        trf("Delete host '{name}'?", &[("name", &h.display)])
    };
    if !dialogs::confirm(&q) {
        return;
    }
    st().model.remove(&t);
    st().store.remove_splits(&format!("host:{t}"));
    save_store();
    select(Entry::Open, false);
}

unsafe fn setup_host() {
    let Some(t) = selected_host() else { return };
    let texts = crate::core::wt::SetupTexts {
        title: trf("Setup: {host}", &[("host", &t)]),
        no_key: tr("No SSH key found in %USERPROFILE%\\.ssh\\."),
        create_hint: tr("Create one first with: ssh-keygen -t ed25519"),
        press_enter: tr("Press Enter to close"),
        deploying: tr("Deploying {key} to {host}..."),
        deployed: tr("Key deployed."),
    };
    if let Err(e) = worker::launch_wt(&crate::core::wt::setup_args(&t, &texts)) {
        dialogs::error(&trf("Error on {name}", &[("name", &t)]), &e);
    }
}

pub unsafe fn perform(a: Action) {
    let view = st().view;
    match a {
        Action::Quit => {
            let _ = DestroyWindow(st().main);
        }
        Action::Hide => hide_main(),
        Action::NewHost => {
            select(Entry::NewHost, true);
        }
        Action::AddHost => add_host(),
        Action::GotoOpen => select(Entry::Open, true),
        Action::GotoDirect => select(Entry::Direct, true),
        Action::GotoShowAll => select(Entry::ShowAll, true),
        Action::OpenAll => {
            if st().model.any_pending() {
                st().open_all_waiting = true;
                views::refresh_view_data(None);
            } else {
                do_open_all();
            }
        }
        Action::Settings => dialogs::show_settings(false),
        Action::SettingsLanguage => dialogs::show_settings(true),
        Action::DeleteHost => delete_host(),
        Action::Toggle | Action::ToggleAll | Action::Invert => views::selection_action(a),
        Action::JumpStart => {
            if let Some(pair) = views::focused_pair() {
                launch_pairs(vec![pair], true);
            }
        }
        Action::Reload => {
            if let Some(t) = selected_host() {
                start_connect(&t);
                views::refresh_host_list();
                views::show_view();
            }
        }
        Action::SetupHost => setup_host(),
        Action::DirectStart => direct_start(),
        Action::OneWindow | Action::PerHost | Action::Execute => {
            let single = a != Action::PerHost;
            match view {
                View::Open => open_selection(single),
                View::Direct => execute_direct(single),
                View::ShowAll => launch_pairs(st().model.all_selection(), single),
                _ => {}
            }
        }
        Action::SortHost | Action::SortSession => {
            st().model.sort_all(if a == Action::SortHost {
                SortKey::Host
            } else {
                SortKey::Session
            });
            views::refresh_view_data(None);
        }
        Action::Filter => views::focus_filter(),
        Action::Help => toggle_help_all(),
        Action::ReloadConfig => reload_hosts(),
    }
}

// ----------------------------------------------------------------------------
// Keyboard (SPEC-003 §5): runs before IsDialogMessage in the message loop.
// ----------------------------------------------------------------------------
fn key_down(vk: VIRTUAL_KEY) -> bool {
    unsafe { GetKeyState(vk.0 as i32) < 0 }
}

/// Actions bound to a single character, depending on the view.
fn char_action(c: char, view: View) -> Option<Action> {
    let global = match c {
        'q' => Some(Action::Quit),
        'n' => Some(Action::NewHost),
        'o' => Some(Action::GotoOpen),
        'x' => Some(Action::OpenAll),
        '+' => Some(Action::GotoDirect),
        '-' => Some(Action::GotoShowAll),
        'l' => Some(Action::SettingsLanguage),
        '/' => Some(Action::Filter),
        _ => None,
    };
    if global.is_some() {
        return global;
    }
    match (view, c) {
        (View::Connected | View::Direct | View::ShowAll, 't') => Some(Action::Toggle),
        (View::Connected | View::Direct | View::ShowAll, 'a') => Some(Action::ToggleAll),
        (View::Connected | View::Direct | View::ShowAll, 'i') => Some(Action::Invert),
        (View::Connected | View::ShowAll, 'j') => Some(Action::JumpStart),
        (View::Direct, 'j') => Some(Action::Execute),
        (View::Connected | View::Failed, 'r') => Some(Action::Reload),
        (View::Failed, 's') => Some(Action::SetupHost),
        (View::Open | View::ShowAll, 'e') => Some(Action::OneWindow),
        (View::Open | View::Direct | View::ShowAll, 'p') => Some(Action::PerHost),
        (View::ShowAll, 'h') => Some(Action::SortHost),
        (View::ShowAll, 's') => Some(Action::SortSession),
        _ => None,
    }
}

unsafe fn pre_translate(msg: &MSG) -> bool {
    let main = st().main;
    if msg.hwnd != main && !IsChild(main, msg.hwnd).as_bool() {
        return false;
    }
    let focus = GetFocus();
    match msg.message {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let vk = VIRTUAL_KEY(msg.wParam.0 as u16);
            let ctrl = key_down(VK_CONTROL);
            let shift = key_down(VK_SHIFT);
            let alt = key_down(VK_MENU);
            if alt {
                return false;
            }
            match vk {
                VK_F1 => perform(Action::Help),
                VK_F5 => perform(Action::ReloadConfig),
                VK_F if ctrl => perform(Action::Filter),
                VK_OEM_COMMA if ctrl => perform(Action::Settings),
                VK_UP | VK_DOWN | VK_LEFT | VK_RIGHT if ctrl && shift => views::nudge_splitter(vk),
                VK_RETURN if ctrl => {
                    if st().view == View::Connected {
                        perform(Action::DirectStart);
                    }
                }
                VK_ESCAPE => {
                    if !st().help_wnd.is_invalid() && IsWindowVisible(st().help_wnd).as_bool() {
                        hide_help();
                    } else if let Some(list) = views::list_of_filter(focus) {
                        let _ = SetFocus(Some(list));
                    } else {
                        perform(Action::Hide);
                    }
                }
                VK_DOWN if views::list_of_filter(focus).is_some() => {
                    let _ = SetFocus(views::list_of_filter(focus));
                }
                VK_RETURN => {
                    if focus == st().c.host_list {
                        views::focus_detail();
                    } else if let Some(list) = views::list_of_filter(focus) {
                        let _ = SetFocus(Some(list));
                    } else if focus == st().c.ed_new {
                        perform(Action::AddHost);
                    } else if views::is_button(focus) {
                        views::click(focus);
                    } else {
                        return false;
                    }
                }
                VK_DELETE if !is_edit(focus) => {
                    if selected_host().is_some() {
                        perform(Action::DeleteHost);
                    }
                }
                _ => return false,
            }
            true
        }
        WM_CHAR => {
            if is_edit(focus) || key_down(VK_CONTROL) {
                return false;
            }
            let Some(c) = char::from_u32(msg.wParam.0 as u32) else {
                return false;
            };
            match char_action(c.to_ascii_lowercase(), st().view) {
                Some(a) if c.is_ascii_lowercase() || !c.is_ascii_alphabetic() => {
                    perform(a);
                    true
                }
                _ => false,
            }
        }
        _ => false,
    }
}

// ----------------------------------------------------------------------------
// Show / hide, tray, hotkey
// ----------------------------------------------------------------------------
pub unsafe fn show_main() {
    let h = st().main;
    if IsIconic(h).as_bool() {
        let _ = ShowWindow(h, SW_RESTORE);
    } else {
        let _ = ShowWindow(h, SW_SHOW);
    }
    let _ = SetForegroundWindow(h);
    views::restore_focus();
}

pub unsafe fn hide_main() {
    hide_help();
    views::remember_focus();
    let _ = ShowWindow(st().main, SW_HIDE);
}

unsafe fn toggle_main() {
    let h = st().main;
    if IsWindowVisible(h).as_bool() && !IsIconic(h).as_bool() && GetForegroundWindow() == h {
        hide_main();
    } else {
        show_main();
    }
}

pub unsafe fn register_hotkey(mods: u32, vk: u32) -> bool {
    RegisterHotKey(Some(st().main), HOTKEY_ID, HOT_KEY_MODIFIERS(mods | MOD_NOREPEAT.0), vk).is_ok()
}

pub unsafe fn update_tray(op: NOTIFY_ICON_MESSAGE) {
    let stt = st();
    stt.nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    stt.nid.hWnd = stt.main;
    stt.nid.uID = 1;
    stt.nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    stt.nid.uCallbackMessage = WM_APP_TRAY;
    if stt.nid.hIcon.is_invalid() {
        stt.nid.hIcon = LoadImageW(
            Some(stt.inst),
            PCWSTR(std::ptr::without_provenance(1)),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            IMAGE_FLAGS(0),
        )
        .map(|h| HICON(h.0))
        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
        .unwrap_or_default();
    }
    let tip = if stt.hotkey_ok {
        format!("{APP_NAME} – {}", store::hotkey_text(stt.set.mods, stt.set.vk))
    } else {
        format!("{APP_NAME} – {}", tr("hotkey not active"))
    };
    stt.nid.szTip = [0; 128];
    for (i, c) in tip.encode_utf16().take(127).enumerate() {
        stt.nid.szTip[i] = c;
    }
    let _ = Shell_NotifyIconW(op, &stt.nid);
}

unsafe fn balloon(title: &str, body: &str) {
    let mut nid = st().nid;
    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = NIIF_INFO;
    nid.szInfoTitle = [0; 64];
    for (i, c) in title.encode_utf16().take(63).enumerate() {
        nid.szInfoTitle[i] = c;
    }
    nid.szInfo = [0; 256];
    for (i, c) in body.encode_utf16().take(255).enumerate() {
        nid.szInfo[i] = c;
    }
    let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
}

unsafe fn about() {
    let stt = st();
    let txt = format!(
        "{APP_NAME} {}\n\n{}\n\n{}: {}\n{}:\n{}",
        env!("CARGO_PKG_VERSION"),
        tr("tmux sessions on remote Linux hosts as Windows Terminal tabs."),
        tr("Hotkey"),
        store::hotkey_text(stt.set.mods, stt.set.vk),
        tr("Settings file"),
        stt.store.path.display()
    );
    dialogs::info(APP_NAME, &txt);
}

unsafe fn tray_menu() {
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let Ok(m) = CreatePopupMenu() else { return };
    let open = format!("{}\t{}", tr("Open"), store::hotkey_text(st().set.mods, st().set.vk));
    let _ = AppendMenuW(m, MF_STRING, IDM_OPEN, &hs(&open));
    let _ = AppendMenuW(m, MF_STRING, IDM_SETTINGS, &hs(&(tr("Settings") + "…")));
    let _ = AppendMenuW(m, MF_STRING, IDM_ABOUT, &hs(&tr("About")));
    let _ = AppendMenuW(m, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(m, MF_STRING, IDM_EXIT, &hs(&tr("Quit")));
    let _ = SetMenuDefaultItem(m, IDM_OPEN as u32, 0);
    let h = st().main;
    let _ = SetForegroundWindow(h);
    let cmd = TrackPopupMenu(
        m,
        TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
        pt.x,
        pt.y,
        None,
        h,
        None,
    )
    .0 as usize;
    post(h, WM_NULL, 0, 0);
    let _ = DestroyMenu(m);
    match cmd {
        IDM_OPEN => show_main(),
        IDM_SETTINGS => {
            show_main();
            dialogs::show_settings(false);
        }
        IDM_ABOUT => about(),
        IDM_EXIT => {
            let _ = DestroyWindow(h);
        }
        _ => {}
    }
}

// ----------------------------------------------------------------------------
// Help overlay
// ----------------------------------------------------------------------------
pub unsafe fn hide_help() {
    let h = st().help_wnd;
    if !h.is_invalid() {
        let _ = ShowWindow(h, SW_HIDE);
    }
    st().help_all = false;
}

unsafe fn toggle_help_all() {
    let h = st().help_wnd;
    if !h.is_invalid() && IsWindowVisible(h).as_bool() && st().help_all {
        hide_help();
        return;
    }
    let text = help::text_all(st().view);
    st().help_all = true;
    dialogs::show_overlay(&text, None);
}

// ----------------------------------------------------------------------------
// Main window procedure
// ----------------------------------------------------------------------------
unsafe fn splitter_at(x: i32, y: i32) -> Option<usize> {
    st().splitters.iter().position(|sp| pt_in(&sp.rect, x, y))
}

unsafe fn footer_at(x: i32, y: i32) -> Option<usize> {
    st().footer_rects.iter().position(|(r, _)| pt_in(r, x, y))
}

unsafe extern "system" fn main_proc(h: HWND, m: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if m == st().msg_taskbar_created && m != 0 {
        update_tray(NIM_ADD);
        return LRESULT(0);
    }
    match m {
        worker::WM_APP_EVENT => {
            handle_events();
            return LRESULT(0);
        }
        WM_HOTKEY => {
            toggle_main();
            return LRESULT(0);
        }
        WM_APP_SHOW => {
            show_main();
            return LRESULT(0);
        }
        WM_APP_SETTINGS => {
            show_main();
            dialogs::show_settings(false);
            return LRESULT(0);
        }
        WM_APP_TRAY => {
            match loword(lp.0 as usize) {
                WM_LBUTTONUP => show_main(),
                WM_RBUTTONUP | WM_CONTEXTMENU => tray_menu(),
                _ => {}
            }
            return LRESULT(0);
        }
        WM_COMMAND => {
            views::on_command(loword(wp.0) as i32, hiword(wp.0), HWND(lp.0 as *mut c_void));
            return LRESULT(0);
        }
        WM_NOTIFY => {
            return views::on_notify(lp);
        }
        WM_DRAWITEM => {
            views::on_draw_item(&*(lp.0 as *const DRAWITEMSTRUCT));
            return LRESULT(1);
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC | WM_CTLCOLORLISTBOX => {
            return views::on_ctl_color(m, HDC(wp.0 as *mut c_void), HWND(lp.0 as *mut c_void));
        }
        WM_ERASEBKGND => return LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(h, &mut ps);
            views::paint(dc);
            let _ = EndPaint(h, &ps);
            return LRESULT(0);
        }
        WM_SIZE => {
            views::layout();
            return LRESULT(0);
        }
        WM_GETMINMAXINFO => {
            let mmi = &mut *(lp.0 as *mut MINMAXINFO);
            mmi.ptMinTrackSize = POINT { x: s(560), y: s(380) };
            return LRESULT(0);
        }
        WM_SETCURSOR => {
            if loword(lp.0 as usize) == HTCLIENT {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                let _ = ScreenToClient(h, &mut pt);
                if let Some(i) = splitter_at(pt.x, pt.y).or(st().drag.map(|d| d.0)) {
                    let id = if st().splitters[i].vertical {
                        IDC_SIZEWE
                    } else {
                        IDC_SIZENS
                    };
                    SetCursor(LoadCursorW(None, id).ok());
                    return LRESULT(1);
                }
                if footer_at(pt.x, pt.y).is_some() {
                    SetCursor(LoadCursorW(None, IDC_HAND).ok());
                    return LRESULT(1);
                }
            }
        }
        WM_LBUTTONDOWN => {
            let (x, y) = (x_lparam(lp), y_lparam(lp));
            hide_help();
            if let Some(i) = splitter_at(x, y) {
                let sp = st().splitters[i];
                let pos = if sp.vertical { x - sp.rect.left } else { y - sp.rect.top };
                st().drag = Some((i, pos));
                SetCapture(h);
            } else if let Some(i) = footer_at(x, y) {
                let a = st().footer_rects[i].1.action;
                perform(a);
            }
            return LRESULT(0);
        }
        WM_MOUSEMOVE => {
            let (x, y) = (x_lparam(lp), y_lparam(lp));
            if let Some((i, off)) = st().drag {
                views::drag_splitter(i, x, y, off);
                return LRESULT(0);
            }
            let hit = footer_at(x, y);
            if hit != st().hover {
                st().hover = hit;
                let _ = InvalidateRect(Some(h), Some(&st().rc_footer), false);
                if !st().help_all {
                    hide_help();
                }
                if hit.is_some() {
                    SetTimer(Some(h), TIMER_HOVER, 400, None);
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: h,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                }
            }
            return LRESULT(0);
        }
        WM_MOUSELEAVE => {
            st().hover = None;
            let _ = InvalidateRect(Some(h), Some(&st().rc_footer), false);
            if !st().help_all {
                hide_help();
            }
            return LRESULT(0);
        }
        WM_TIMER if wp.0 == TIMER_HOVER => {
            let _ = KillTimer(Some(h), TIMER_HOVER);
            if let Some(i) = st().hover {
                if let Some((r, item)) = st().footer_rects.get(i).copied() {
                    st().help_all = false;
                    dialogs::show_overlay(&help::text_one(&item), Some(r));
                }
            }
            return LRESULT(0);
        }
        WM_LBUTTONUP => {
            if st().drag.take().is_some() {
                let _ = ReleaseCapture();
                views::save_splits();
                let _ = st().store.save();
            }
            return LRESULT(0);
        }
        WM_DPICHANGED => {
            st().dpi = hiword(wp.0);
            let r = &*(lp.0 as *const RECT);
            let _ = SetWindowPos(
                h,
                None,
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            create_fonts();
            views::apply_fonts();
            views::layout();
            return LRESULT(0);
        }
        WM_SETTINGCHANGE | WM_DWMCOLORIZATIONCOLORCHANGED => {
            apply_theme();
            views::apply_colors();
        }
        WM_ACTIVATE => {
            if loword(wp.0) == WA_INACTIVE {
                views::remember_focus();
            }
        }
        WM_SETFOCUS => {
            views::restore_focus();
            return LRESULT(0);
        }
        WM_CLOSE => {
            hide_main();
            return LRESULT(0);
        }
        WM_ENDSESSION => {
            views::save_geometry();
            let _ = st().store.save();
        }
        WM_DESTROY => {
            views::save_geometry();
            save_store();
            update_tray(NIM_DELETE);
            PostQuitMessage(0);
            return LRESULT(0);
        }
        _ => {}
    }
    DefWindowProcW(h, m, wp, lp)
}

// ----------------------------------------------------------------------------
// Language (live switch, SPEC-003 §7/§8)
// ----------------------------------------------------------------------------
pub unsafe fn preferred_languages() -> Vec<String> {
    let mut num = 0u32;
    let mut len = 0u32;
    let flags = windows::Win32::Globalization::MUI_LANGUAGE_NAME;
    if windows::Win32::Globalization::GetUserPreferredUILanguages(flags, &mut num, None, &mut len).is_err() || len == 0
    {
        return Vec::new();
    }
    let mut buf = vec![0u16; len as usize];
    if windows::Win32::Globalization::GetUserPreferredUILanguages(
        flags,
        &mut num,
        Some(windows::core::PWSTR(buf.as_mut_ptr())),
        &mut len,
    )
    .is_err()
    {
        return Vec::new();
    }
    buf.split(|&c| c == 0)
        .filter(|p| !p.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

pub unsafe fn apply_language() {
    let code = match &st().set.language {
        Some(c) => c.clone(),
        None => i18n::pick_language(&preferred_languages()).to_string(),
    };
    i18n::set_language(&code);
    let main = st().main;
    if main.is_invalid() {
        return;
    }
    views::remember_focus();
    let ex = GetWindowLongPtrW(main, GWL_EXSTYLE) as u32;
    let rtl = WS_EX_LAYOUTRTL.0;
    let want = if i18n::is_rtl() { ex | rtl } else { ex & !rtl };
    views::destroy_controls();
    if want != ex {
        SetWindowLongPtrW(main, GWL_EXSTYLE, want as isize);
    }
    if !st().help_wnd.is_invalid() {
        let _ = DestroyWindow(st().help_wnd);
        st().help_wnd = HWND::default();
    }
    views::create_controls();
    views::refresh_host_list();
    views::show_view();
    update_tray(NIM_MODIFY);
    let _ = InvalidateRect(Some(main), None, true);
}

// ----------------------------------------------------------------------------
// Entry point
// ----------------------------------------------------------------------------
pub fn run() {
    unsafe { run_inner() }
}

unsafe fn run_inner() {
    let inst: HINSTANCE = GetModuleHandleW(None).map(|m| m.into()).unwrap_or_default();
    let mutex = CreateMutexW(None, true, w!("Local\\OpenTmuxTabs_SingleInstance_3C91"));
    if GetLastError() == ERROR_ALREADY_EXISTS {
        if let Ok(other) = FindWindowW(MAIN_CLASS, PCWSTR::null()) {
            let mut pid = 0u32;
            GetWindowThreadProcessId(other, Some(&mut pid));
            let _ = AllowSetForegroundWindow(pid);
            let msg = if arg_present("--settings") {
                WM_APP_SETTINGS
            } else {
                WM_APP_SHOW
            };
            post(other, msg, 0, 0);
        }
        return;
    }
    let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    let icc = INITCOMMONCONTROLSEX {
        dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_STANDARD_CLASSES | ICC_WIN95_CLASSES | ICC_LISTVIEW_CLASSES,
    };
    let _ = InitCommonControlsEx(&icc);

    let store = Store::load();
    let set = store.settings();
    let state = Box::new(State {
        inst,
        main: HWND::default(),
        c: views::Controls::default(),
        model: Model::default(),
        store,
        set,
        th: Theme::default(),
        br_bg: HBRUSH::default(),
        br_panel: HBRUSH::default(),
        br_field: HBRUSH::default(),
        dpi: 96,
        font: HFONT::default(),
        font_bold: HFONT::default(),
        line_h: 16,
        char_w: 7,
        entries: Vec::new(),
        sel: Entry::Open,
        view: View::Open,
        left_w: 260,
        rc_header: RECT::default(),
        rc_footer: RECT::default(),
        splitters: Vec::new(),
        drag: None,
        footer_rects: Vec::new(),
        hover: None,
        help_wnd: HWND::default(),
        help_all: false,
        syncing: false,
        open_all_waiting: false,
        hotkey_ok: false,
        nid: NOTIFYICONDATAW::default(),
        msg_taskbar_created: 0,
        settings_wnd: HWND::default(),
        last_sel_index: -1,
    });
    STATE = Box::into_raw(state);
    apply_language();
    apply_theme();
    if let Some(cur) = store::get_autostart() {
        if cur != store::run_value() {
            store::set_autostart(true);
        }
    }

    let big_icon = LoadImageW(
        Some(inst),
        PCWSTR(std::ptr::without_provenance(1)),
        IMAGE_ICON,
        0,
        0,
        LR_DEFAULTSIZE,
    )
    .map(|h| HICON(h.0))
    .unwrap_or_default();
    let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(main_proc),
        hInstance: inst,
        hCursor: cursor,
        hIcon: big_icon,
        lpszClassName: MAIN_CLASS,
        ..Default::default()
    };
    RegisterClassExW(&wc);
    dialogs::register_classes(inst, big_icon, cursor);
    st().msg_taskbar_created = RegisterWindowMessageW(w!("TaskbarCreated"));

    let ex = if i18n::is_rtl() {
        WS_EX_LAYOUTRTL | WS_EX_CONTROLPARENT
    } else {
        WS_EX_CONTROLPARENT
    };
    let Ok(main) = CreateWindowExW(
        ex,
        MAIN_CLASS,
        &hs(APP_NAME),
        WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        800,
        600,
        None,
        None,
        Some(inst),
        None,
    ) else {
        return;
    };
    st().main = main;
    worker::set_main(main);
    let _ = ChangeWindowMessageFilterEx(main, st().msg_taskbar_created, MSGFLT_ALLOW, None);
    st().dpi = GetDpiForWindow(main);
    apply_theme();
    create_fonts();
    let w = st().store.ini.get_i32("Window", "HostListWidth").unwrap_or(260);
    st().left_w = w.clamp(120, 2000);
    views::create_controls();

    reload_hosts();
    st().sel = if st().model.hosts.is_empty() {
        Entry::NewHost
    } else {
        Entry::Open
    };
    views::refresh_host_list();
    views::show_view();

    let maximized = views::restore_geometry();
    let autostart = arg_present("--autostart");
    if !autostart {
        let _ = ShowWindow(main, if maximized { SW_SHOWMAXIMIZED } else { SW_SHOW });
        let _ = SetForegroundWindow(main);
        views::focus_detail();
    }

    let set = st().set.clone();
    st().hotkey_ok = register_hotkey(set.mods, set.vk);
    update_tray(NIM_ADD);
    if arg_present("--settings") {
        show_main();
        dialogs::show_settings(false);
    } else if st().hotkey_ok && !autostart {
        let txt = trf(
            "Press {hotkey} to show or hide the window.",
            &[("hotkey", &store::hotkey_text(set.mods, set.vk))],
        );
        balloon(&trf("{app} is running", &[("app", APP_NAME)]), &txt);
    }
    if !st().hotkey_ok {
        let msg = trf(
            "The hotkey {hotkey} is already in use.\n\nPlease choose a different one in the settings.",
            &[("hotkey", &store::hotkey_text(set.mods, set.vk))],
        );
        dialogs::error(APP_NAME, &msg);
        show_main();
        dialogs::show_settings(false);
    }

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
        let dlg = st().settings_wnd;
        if !dlg.is_invalid() && (msg.hwnd == dlg || IsChild(dlg, msg.hwnd).as_bool()) {
            if dialogs::settings_pre_translate(&msg) || IsDialogMessageW(dlg, &msg).as_bool() {
                continue;
            }
        } else if pre_translate(&msg) || IsDialogMessageW(st().main, &msg).as_bool() {
            continue;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    if let Ok(m) = mutex {
        let _ = CloseHandle(m);
    }
}
