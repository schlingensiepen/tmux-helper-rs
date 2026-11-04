//! Settings file `OpenTmuxTabs.ini` (next to the EXE, fallback
//! `%APPDATA%\OpenTmuxTabs`), autostart entry and hotkey names.

use super::util::*;
use crate::core::ini::Ini;
use crate::core::model::Model;
use std::ffi::c_void;
use std::path::PathBuf;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub const APP_NAME: &str = "Open-TmuxTabs";
const INI_NAME: &str = "OpenTmuxTabs.ini";
const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: &str = "OpenTmuxTabs";
pub const MODS_MASK: u32 = MOD_ALT.0 | MOD_CONTROL.0 | MOD_SHIFT.0 | MOD_WIN.0;

#[derive(Clone, PartialEq)]
pub struct Settings {
    pub mods: u32,
    pub vk: u32,
    pub bg: COLORREF,
    /// `None` = follow the system language.
    pub language: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            mods: MOD_CONTROL.0 | MOD_ALT.0,
            vk: 'T' as u32,
            bg: rgb(28, 30, 36),
            language: None,
        }
    }
}

pub fn valid_key(vk: u32) -> bool {
    (0x30..=0x39).contains(&vk)
        || (0x41..=0x5A).contains(&vk)
        || (VK_F1.0 as u32..=VK_F24.0 as u32).contains(&vk)
        || [
            VK_SPACE.0,
            VK_OEM_PERIOD.0,
            VK_OEM_COMMA.0,
            VK_OEM_MINUS.0,
            VK_OEM_PLUS.0,
        ]
        .contains(&(vk as u16))
}

fn exe_path() -> PathBuf {
    std::env::current_exe().unwrap_or_default()
}
fn exe_dir() -> PathBuf {
    exe_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}
fn appdata_dir() -> PathBuf {
    match std::env::var("APPDATA") {
        Ok(a) if !a.is_empty() => PathBuf::from(a).join("OpenTmuxTabs"),
        _ => exe_dir(),
    }
}

pub struct Store {
    pub ini: Ini,
    pub path: PathBuf,
}

impl Store {
    pub fn load() -> Store {
        let local = exe_dir().join(INI_NAME);
        let roam = appdata_dir().join(INI_NAME);
        let path = if !local.exists() && roam.exists() { roam } else { local };
        let ini = std::fs::read_to_string(&path)
            .map(|t| Ini::parse(&t))
            .unwrap_or_default();
        Store { ini, path }
    }

    /// Writes the INI; falls back to `%APPDATA%` if the EXE folder is
    /// read-only. Returns `false` if neither works.
    pub fn save(&mut self) -> bool {
        let text = self.ini.to_text();
        if std::fs::write(&self.path, &text).is_ok() {
            return true;
        }
        let dir = appdata_dir();
        let _ = std::fs::create_dir_all(&dir);
        let roam = dir.join(INI_NAME);
        if std::fs::write(&roam, &text).is_ok() {
            self.path = roam;
            return true;
        }
        false
    }

    pub fn settings(&self) -> Settings {
        let d = Settings::default();
        let i = &self.ini;
        let mut s = Settings {
            mods: i
                .get_i32("Settings", "HotkeyMods")
                .map(|v| v as u32 & MODS_MASK)
                .unwrap_or(d.mods),
            vk: i.get_i32("Settings", "HotkeyKey").map(|v| v as u32).unwrap_or(d.vk),
            bg: d.bg,
            language: i
                .get("Settings", "Language")
                .filter(|l| *l != "auto" && !l.is_empty())
                .map(String::from),
        };
        if let Some(c) = i.get("Settings", "BackgroundColor") {
            if c.len() == 7 && c.starts_with('#') {
                if let Ok(v) = u32::from_str_radix(&c[1..], 16) {
                    s.bg = rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
                }
            }
        }
        if s.mods == 0 || !valid_key(s.vk) {
            s.mods = d.mods;
            s.vk = d.vk;
        }
        s
    }

    pub fn set_settings(&mut self, s: &Settings) {
        let i = &mut self.ini;
        i.set("Settings", "HotkeyMods", s.mods.to_string());
        i.set("Settings", "HotkeyKey", s.vk.to_string());
        i.set(
            "Settings",
            "BackgroundColor",
            format!("#{:02X}{:02X}{:02X}", cr_r(s.bg), cr_g(s.bg), cr_b(s.bg)),
        );
        i.set(
            "Settings",
            "Language",
            s.language.clone().unwrap_or_else(|| "auto".into()),
        );
    }

    pub fn manual_hosts(&self) -> Vec<String> {
        self.ini
            .entries("Hosts")
            .into_iter()
            .map(|(_, v)| v)
            .filter(|v| !v.is_empty())
            .collect()
    }
    pub fn hidden_hosts(&self) -> Vec<String> {
        self.ini
            .entries("Hidden")
            .into_iter()
            .map(|(_, v)| v)
            .filter(|v| !v.is_empty())
            .collect()
    }

    /// Applies stored usage counters to the model.
    pub fn apply_usage(&self, m: &mut Model) {
        for (k, v) in self.ini.entries("Usage") {
            if let Some(h) = m.host_mut(&k) {
                let mut it = v.split(',');
                h.use_count = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0);
                h.last_used = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0);
            }
        }
    }

    /// Writes hosts, hidden hosts and usage from the model.
    pub fn put_hosts(&mut self, m: &Model) {
        let hosts = m
            .manual_targets()
            .into_iter()
            .enumerate()
            .map(|(i, t)| ((i + 1).to_string(), t))
            .collect();
        self.ini.set_section("Hosts", hosts);
        let hidden = m
            .hidden
            .iter()
            .enumerate()
            .map(|(i, t)| ((i + 1).to_string(), t.clone()))
            .collect();
        self.ini.set_section("Hidden", hidden);
        let usage = m
            .hosts
            .iter()
            .filter(|h| h.use_count > 0)
            .map(|h| (h.target.clone(), format!("{},{}", h.use_count, h.last_used)))
            .collect();
        self.ini.set_section("Usage", usage);
    }

    /// Splitter positions in per mille for a key (`host:<target>`, `open`,
    /// `direct`, `host:*` = last value used for any host).
    pub fn splits(&self, key: &str) -> Option<Vec<i32>> {
        let v = self.ini.get("Splits", key)?;
        let out: Vec<i32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        (!out.is_empty()).then_some(out)
    }
    pub fn set_splits(&mut self, key: &str, v: &[i32]) {
        let s = v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
        self.ini.set("Splits", key, s);
    }
    pub fn remove_splits(&mut self, key: &str) {
        self.ini.remove("Splits", key);
    }
}

// ---- Autostart --------------------------------------------------------------

pub fn run_value() -> String {
    format!("\"{}\" --autostart", exe_path().display())
}

pub unsafe fn get_autostart() -> Option<String> {
    let mut buf = vec![0u16; 2048];
    let mut size = (buf.len() * 2) as u32;
    let r = RegGetValueW(
        HKEY_CURRENT_USER,
        RUN_KEY,
        &hs(RUN_VALUE),
        RRF_RT_REG_SZ,
        None,
        Some(buf.as_mut_ptr() as *mut c_void),
        Some(&mut size),
    );
    if r != ERROR_SUCCESS {
        return None;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
    Some(String::from_utf16_lossy(&buf[..n]))
}

pub unsafe fn set_autostart(on: bool) {
    let mut key = HKEY::default();
    if RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, KEY_SET_VALUE, &mut key) != ERROR_SUCCESS {
        return;
    }
    if on {
        let v = wide(&run_value());
        let bytes = std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2);
        let _ = RegSetValueExW(key, &hs(RUN_VALUE), None, REG_SZ, Some(bytes));
    } else {
        let _ = RegDeleteValueW(key, &hs(RUN_VALUE));
    }
    let _ = RegCloseKey(key);
}

// ---- Hotkey names -------------------------------------------------------------

pub unsafe fn key_name(vk: u32) -> String {
    if (0x30..=0x39).contains(&vk) || (0x41..=0x5A).contains(&vk) {
        return char::from_u32(vk).map(|c| c.to_string()).unwrap_or_default();
    }
    if (VK_F1.0 as u32..=VK_F24.0 as u32).contains(&vk) {
        return format!("F{}", vk - VK_F1.0 as u32 + 1);
    }
    match vk as u16 {
        x if x == VK_SPACE.0 => return crate::core::i18n::tr("Space"),
        x if x == VK_OEM_PERIOD.0 => return ".".into(),
        x if x == VK_OEM_COMMA.0 => return ",".into(),
        x if x == VK_OEM_MINUS.0 => return "-".into(),
        x if x == VK_OEM_PLUS.0 => return "+".into(),
        _ => {}
    }
    format!("#{vk}")
}

pub unsafe fn hotkey_text(mods: u32, vk: u32) -> String {
    let tr = crate::core::i18n::tr;
    let mut s = String::new();
    if mods & MOD_WIN.0 != 0 {
        s += "Win+";
    }
    if mods & MOD_CONTROL.0 != 0 {
        s += &(tr("Ctrl") + "+");
    }
    if mods & MOD_ALT.0 != 0 {
        s += "Alt+";
    }
    if mods & MOD_SHIFT.0 != 0 {
        s += &(tr("Shift") + "+");
    }
    s + &key_name(vk)
}
