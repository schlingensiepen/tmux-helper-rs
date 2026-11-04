//! Reading host names from the OpenSSH client configuration
//! (`%USERPROFILE%\.ssh\config`), SPEC-003 §3.
//!
//! Every name of a `Host` line becomes a host, except patterns (`*`, `?`,
//! `!`). `Include` is followed (relative paths are relative to `~/.ssh`,
//! wildcards in the file name are allowed), at most 8 levels deep.

use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 8;

/// Splits a config line into keyword and arguments (`Key value`,
/// `Key=value`, quoted arguments).
fn split_line(line: &str) -> Option<(String, Vec<String>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let end = line.find(|c: char| c.is_whitespace() || c == '=').unwrap_or(line.len());
    let key = line[..end].to_ascii_lowercase();
    let mut rest = line[end..].trim_start();
    if let Some(r) = rest.strip_prefix('=') {
        rest = r.trim_start();
    }
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut has = false;
    for c in rest.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                has = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has {
                    args.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            '#' if !quoted && !has => break,
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        args.push(cur);
    }
    Some((key, args))
}

fn is_pattern(name: &str) -> bool {
    name.contains(['*', '?', '!'])
}

/// Hosts from config text. `include` returns the contents of the files an
/// `Include` argument refers to.
pub fn hosts_from_text(text: &str, include: &mut dyn FnMut(&str) -> Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    collect(text, include, &mut out, 0);
    out
}

fn collect(text: &str, include: &mut dyn FnMut(&str) -> Vec<String>, out: &mut Vec<String>, depth: usize) {
    for line in text.lines() {
        let Some((key, args)) = split_line(line) else { continue };
        match key.as_str() {
            "host" => {
                for a in args {
                    if !is_pattern(&a) && !out.contains(&a) {
                        out.push(a);
                    }
                }
            }
            "include" if depth < MAX_DEPTH => {
                for a in args {
                    for content in include(&a) {
                        collect(&content, include, out, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Simple wildcard match (`*`, `?`) for file names.
pub fn wildcard(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let (mut pi, mut ni, mut star, mut mark) = (0, 0, None, 0);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            pi += 1;
            mark = ni;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Resolves an `Include` argument to file paths (sorted).
pub fn include_paths(ssh_dir: &Path, home: &Path, arg: &str) -> Vec<PathBuf> {
    let expanded = if let Some(r) = arg.strip_prefix("~/").or_else(|| arg.strip_prefix("~\\")) {
        home.join(r)
    } else {
        let p = PathBuf::from(arg);
        if p.is_absolute() { p } else { ssh_dir.join(p) }
    };
    let name = expanded.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if !name.contains(['*', '?']) {
        return vec![expanded];
    }
    let dir = expanded.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| wildcard(&name, &e.file_name().to_string_lossy()))
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// Reads `<home>/.ssh/config` and returns its host names (empty if the file
/// does not exist).
pub fn read_hosts(home: &Path) -> Vec<String> {
    let ssh_dir = home.join(".ssh");
    let Ok(text) = std::fs::read_to_string(ssh_dir.join("config")) else { return Vec::new() };
    let mut include = |arg: &str| -> Vec<String> {
        include_paths(&ssh_dir, home, arg)
            .into_iter()
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .collect()
    };
    hosts_from_text(&text, &mut include)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hosts() {
        let cfg = "# comment\nHost clawbook\n  HostName localhost\n  Port 2222\nHost a b *.corp !x\nhost=\"quoted name\"\nHost *\n  User x\nMatch host foo\n  User y\nHost a\n";
        let hosts = hosts_from_text(cfg, &mut |_| Vec::new());
        assert_eq!(hosts, vec!["clawbook", "a", "b", "quoted name"]);
    }

    #[test]
    fn includes() {
        let cfg = "Include conf.d/*\nHost main\n";
        let mut seen = Vec::new();
        let hosts = hosts_from_text(cfg, &mut |a| {
            seen.push(a.to_string());
            vec!["Host inc1 inc2\n".to_string()]
        });
        assert_eq!(seen, vec!["conf.d/*"]);
        assert_eq!(hosts, vec!["inc1", "inc2", "main"]);
        // Recursion stops.
        let hosts = hosts_from_text("Include self\nHost z\n", &mut |_| vec!["Include self\nHost y\n".into()]);
        assert_eq!(hosts, vec!["y", "z"]);
    }

    #[test]
    fn wildcards() {
        assert!(wildcard("*.conf", "a.conf"));
        assert!(wildcard("*", "x"));
        assert!(wildcard("a?c", "abc"));
        assert!(!wildcard("*.conf", "a.txt"));
    }

    #[test]
    fn files() {
        let dir = std::env::temp_dir().join(format!("oth-sshcfg-{}", std::process::id()));
        let ssh = dir.join(".ssh");
        std::fs::create_dir_all(ssh.join("conf.d")).unwrap();
        std::fs::write(ssh.join("config"), "Include conf.d/*.conf\nHost top\n").unwrap();
        std::fs::write(ssh.join("conf.d").join("b.conf"), "Host b\n").unwrap();
        std::fs::write(ssh.join("conf.d").join("a.conf"), "Host a\n").unwrap();
        std::fs::write(ssh.join("conf.d").join("ignored.txt"), "Host no\n").unwrap();
        assert_eq!(read_hosts(&dir), vec!["a", "b", "top"]);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(read_hosts(&dir).is_empty());
    }
}
