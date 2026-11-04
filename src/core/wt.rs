//! Building Windows Terminal (`wt.exe`) command lines.
//!
//! The command lines are passed to `wt.exe` verbatim (`raw_arg`), exactly as
//! the Python product builds them. `wt` treats `;` as a separator between
//! subcommands, so free text (titles) is escaped as `\;`.

/// Windows limits a command line to 32767 characters; stay well below.
pub const MAX_CMD_LEN: usize = 30_000;

fn esc(s: &str) -> String {
    s.replace('"', "'").replace(';', r"\;")
}

/// One `new-tab` subcommand that attaches to `session` on `host`.
pub fn tab_cmd(host: &str, session: &str) -> String {
    let title = esc(&format!("{session}@{host}"));
    format!(r#"new-tab --title "{title}" cmd.exe /c "ssh -t {host} \"tmux attach-session -t {session}\"""#)
}

/// Splits tab subcommands into batches whose joined length stays below
/// [`MAX_CMD_LEN`]. Each batch becomes one `wt.exe` call (one window).
pub fn split_batches(parts: &[String]) -> Vec<Vec<String>> {
    let base = "wt.exe ".len();
    let mut batches = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    let mut len = base;
    for p in parts {
        let plen = p.len() + 3;
        if !cur.is_empty() && len + plen > MAX_CMD_LEN {
            batches.push(std::mem::take(&mut cur));
            len = base;
        }
        cur.push(p.clone());
        len += plen;
    }
    if !cur.is_empty() {
        batches.push(cur);
    }
    batches
}

/// Argument strings for `wt.exe` (one per window) that open all `sessions`
/// (host, session) as tabs in one window (split only if too long).
pub fn single_window_args(sessions: &[(String, String)]) -> Vec<String> {
    let parts: Vec<String> = sessions.iter().map(|(h, s)| tab_cmd(h, s)).collect();
    split_batches(&parts).into_iter().map(|b| b.join(" ; ")).collect()
}

/// Argument strings for `wt.exe`: one window per host, in the given order.
pub fn per_host_args(by_host: &[(String, Vec<String>)]) -> Vec<String> {
    let mut out = Vec::new();
    for (host, sessions) in by_host {
        if sessions.is_empty() {
            continue;
        }
        let parts: Vec<String> = sessions.iter().map(|s| tab_cmd(host, s)).collect();
        out.extend(split_batches(&parts).into_iter().map(|b| b.join(" ; ")));
    }
    out
}

/// Groups (host, session) pairs by host, keeping the first-seen host order.
pub fn group_by_host(sessions: &[(String, String)]) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for (h, s) in sessions {
        match out.iter_mut().find(|(hh, _)| hh == h) {
            Some((_, v)) => v.push(s.clone()),
            None => out.push((h.clone(), vec![s.clone()])),
        }
    }
    out
}

/// Texts shown by the "Set up host" script (already translated).
pub struct SetupTexts {
    pub title: String,
    pub no_key: String,
    pub create_hint: String,
    pub press_enter: String,
    /// Contains the placeholders `{key}` and `{host}`.
    pub deploying: String,
    pub deployed: String,
}

fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// PowerShell script that deploys the first `%USERPROFILE%\.ssh\id_*.pub`
/// to `host` (interactive, so ssh can ask for the password).
pub fn setup_script(host: &str, t: &SetupTexts) -> String {
    // "{key}" becomes the file name at run time; everything else is literal.
    let deploying = t.deploying.replace("{host}", host);
    let deploying = deploying
        .split("{key}")
        .map(ps_quote)
        .collect::<Vec<_>>()
        .join(" + $pk.Name + ");
    let remote = format!(
        "mkdir -p ~/.ssh && cat >> ~/.ssh/authorized_keys && chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys && echo {}",
        t.deployed.replace('\'', "")
    );
    format!(
        "$ErrorActionPreference='Stop'\n\
         $pk = Get-ChildItem (Join-Path $env:USERPROFILE '.ssh\\id_*.pub') -ErrorAction SilentlyContinue | Select-Object -First 1\n\
         if (-not $pk) {{ Write-Host {no_key}; Write-Host {hint}; Read-Host {enter}; exit 1 }}\n\
         Write-Host ({deploying})\n\
         Get-Content $pk.FullName | ssh {host_q} {remote_q}\n\
         Read-Host {enter}\n",
        no_key = ps_quote(&t.no_key),
        hint = ps_quote(&t.create_hint),
        enter = ps_quote(&t.press_enter),
        host_q = ps_quote(host),
        remote_q = ps_quote(&remote),
    )
}

/// `wt.exe` argument string for "Set up host". The script is passed as
/// `-EncodedCommand` (Base64 of UTF-16LE), so neither quotes nor `;` in the
/// script can confuse `wt`.
pub fn setup_args(host: &str, t: &SetupTexts) -> String {
    let script = setup_script(host, t);
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    format!(
        r#"new-tab --title "{}" powershell.exe -NoProfile -EncodedCommand {}"#,
        esc(&t.title),
        base64(&utf16)
    )
}

/// Standard Base64 with padding.
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab() {
        assert_eq!(
            tab_cmd("root@srv", "build"),
            r#"new-tab --title "build@root@srv" cmd.exe /c "ssh -t root@srv \"tmux attach-session -t build\"""#
        );
    }

    #[test]
    fn batches() {
        let parts: Vec<String> = (0..2000)
            .map(|i| tab_cmd("user@a-rather-long-host.example.com", &format!("s{i}")))
            .collect();
        let b = split_batches(&parts);
        assert!(b.len() > 1);
        assert_eq!(b.iter().map(|x| x.len()).sum::<usize>(), 2000);
        for batch in &b {
            assert!("wt.exe ".len() + batch.join(" ; ").len() <= MAX_CMD_LEN);
        }
        assert_eq!(
            single_window_args(&[("h".into(), "a".into()), ("g".into(), "b".into())]).len(),
            1
        );
    }

    #[test]
    fn per_host() {
        let g = group_by_host(&[
            ("h".into(), "a".into()),
            ("g".into(), "b".into()),
            ("h".into(), "c".into()),
        ]);
        assert_eq!(
            g,
            vec![
                ("h".to_string(), vec!["a".to_string(), "c".to_string()]),
                ("g".to_string(), vec!["b".to_string()])
            ]
        );
        let a = per_host_args(&g);
        assert_eq!(a.len(), 2);
        assert!(a[0].contains(" ; "));
    }

    #[test]
    fn b64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn setup() {
        let t = SetupTexts {
            title: "Setup: x;y".into(),
            no_key: "No key".into(),
            create_hint: "Create".into(),
            press_enter: "Press Enter".into(),
            deploying: "Deploying {key} to {host}...".into(),
            deployed: "Key deployed.".into(),
        };
        let s = setup_script("root@h", &t);
        assert!(s.contains("Write-Host ('Deploying ' + $pk.Name + ' to root@h...')"));
        assert!(!s.contains('"'));
        let a = setup_args("root@h", &t);
        assert!(a.starts_with(r#"new-tab --title "Setup: x\;y" powershell.exe -NoProfile -EncodedCommand "#));
        assert!(!a[a.find("EncodedCommand").unwrap()..].contains(';'));
    }
}
