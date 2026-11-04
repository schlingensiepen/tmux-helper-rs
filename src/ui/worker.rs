//! Background work: SSH calls and Windows Terminal launches run in their own
//! threads. Results are queued and announced to the UI thread with a window
//! message (`WM_APP_EVENT`), so the UI never blocks.

use crate::core::i18n::{tr, trf};
use crate::core::{protocol, wt};
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_APP_EVENT: u32 = WM_APP + 10;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const SSH_TIMEOUT: Duration = Duration::from_secs(120);
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);

pub enum Event {
    Connected { target: String, generation: u64, result: Result<Vec<String>, String> },
    Resolved { target: String, display: String },
    JobDone(JobResult),
}

#[derive(Default)]
pub struct JobResult {
    /// (host target, message)
    pub errors: Vec<(String, String)>,
    /// Hosts whose session list should be reloaded.
    pub refresh: Vec<String>,
    /// Hosts that had tabs opened (usage counter).
    pub opened: Vec<String>,
}

static QUEUE: Mutex<Vec<Event>> = Mutex::new(Vec::new());
static MAIN: AtomicIsize = AtomicIsize::new(0);

pub fn set_main(h: HWND) {
    MAIN.store(h.0 as isize, Ordering::SeqCst);
}

fn push(e: Event) {
    QUEUE.lock().unwrap().push(e);
    let h = HWND(MAIN.load(Ordering::SeqCst) as *mut _);
    unsafe {
        let _ = PostMessageW(Some(h), WM_APP_EVENT, WPARAM(0), LPARAM(0));
    }
}

pub fn drain() -> Vec<Event> {
    std::mem::take(&mut *QUEUE.lock().unwrap())
}

enum RunError {
    Timeout,
    NotFound,
    Other(String),
}

/// Runs a program without a console window, with a time limit.
fn run(program: &str, args: &[&str], timeout: Duration) -> Result<(i32, String, String), RunError> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { RunError::NotFound } else { RunError::Other(e.to_string()) })?;
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let t_out = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = so.read_to_end(&mut b);
        b
    });
    let t_err = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = se.read_to_end(&mut b);
        b
    });
    let start = Instant::now();
    let code = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st.code().unwrap_or(-1),
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Timeout);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(e) => return Err(RunError::Other(e.to_string())),
        }
    };
    let out = String::from_utf8_lossy(&t_out.join().unwrap_or_default()).trim().to_string();
    let err = String::from_utf8_lossy(&t_err.join().unwrap_or_default()).trim().to_string();
    Ok((code, out, err))
}

/// One SSH call with the batched script; returns the parsed output.
fn ssh(target: &str, script: &str) -> Result<protocol::Output, String> {
    match run("ssh", &["-o", "ConnectTimeout=10", target, script], SSH_TIMEOUT) {
        Err(RunError::Timeout) => Err(tr("SSH connection timed out")),
        Err(RunError::NotFound) => Err(tr("ssh executable not found")),
        Err(RunError::Other(e)) => Err(trf("SSH error: {error}", &[("error", &e)])),
        Ok((code, out, err)) => {
            if code != 0 && out.is_empty() {
                let mut m = tr("SSH connection failed");
                if !err.is_empty() {
                    m += "\n";
                    m += &err;
                }
                Err(m)
            } else {
                Ok(protocol::parse_output(&out))
            }
        }
    }
}

pub fn connect(target: String, generation: u64) {
    std::thread::spawn(move || {
        let result = ssh(&target, &protocol::connect_script()).map(|o| o.sessions);
        push(Event::Connected { target, generation, result });
    });
}

/// Resolves an SSH config alias to `user@alias` (`ssh -G`).
pub fn resolve(target: String) {
    if target.contains('@') {
        return;
    }
    std::thread::spawn(move || {
        if let Ok((0, out, _)) = run("ssh", &["-G", &target], RESOLVE_TIMEOUT) {
            let user = std::env::var("USERNAME").unwrap_or_default();
            if let Some(d) = protocol::resolve_from_ssh_g(&target, &out, &user) {
                push(Event::Resolved { target, display: d });
            }
        }
    });
}

/// Starts `wt.exe` with a prepared argument string.
pub fn launch_wt(args: &str) -> Result<(), String> {
    Command::new("wt.exe")
        .raw_arg(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{}\n{e}", tr("Windows Terminal (wt.exe) could not be started.")))
}

/// A launch: optionally reconnect failed hosts, create sessions, then open
/// tabs. Hosts with an error get no tabs (SPEC-003 §6.2).
pub struct LaunchJob {
    pub reconnect: Vec<String>,
    pub create: Vec<(String, Vec<String>)>,
    /// Sessions to open, grouped by host in display order.
    pub open: Vec<(String, Vec<String>)>,
    pub single_window: bool,
    /// Hosts to reload afterwards even without errors.
    pub refresh: Vec<String>,
}

pub fn launch(job: LaunchJob) {
    std::thread::spawn(move || {
        let mut res = JobResult { refresh: job.refresh.clone(), ..Default::default() };
        let mut failed: Vec<String> = Vec::new();
        for t in &job.reconnect {
            if let Err(e) = ssh(t, &protocol::connect_script()) {
                res.errors.push((t.clone(), e));
                failed.push(t.clone());
            }
        }
        for (t, names) in &job.create {
            if failed.contains(t) || names.is_empty() {
                continue;
            }
            if let Err(e) = ssh(t, &protocol::create_script(names)) {
                res.errors.push((t.clone(), e));
                failed.push(t.clone());
            }
            if !res.refresh.contains(t) {
                res.refresh.push(t.clone());
            }
        }
        let open: Vec<(String, Vec<String>)> =
            job.open.into_iter().filter(|(t, s)| !failed.contains(t) && !s.is_empty()).collect();
        let args = if job.single_window {
            let pairs: Vec<(String, String)> =
                open.iter().flat_map(|(t, ss)| ss.iter().map(move |s| (t.clone(), s.clone()))).collect();
            wt::single_window_args(&pairs)
        } else {
            wt::per_host_args(&open)
        };
        for a in args {
            if let Err(e) = launch_wt(&a) {
                res.errors.push((String::new(), e));
                break;
            }
        }
        res.opened = open.into_iter().map(|(t, _)| t).collect();
        push(Event::JobDone(res));
    });
}
