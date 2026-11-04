//! Remote shell scripts and the line protocol (same as the Python product).
//!
//! One SSH call per host runs a batched script that makes sure the tmux base
//! configuration exists, optionally creates sessions and lists all sessions.

const TMUX_CONFIG_BLOCK: &str = "if ! grep -q 'Open-TmuxTabs managed settings' ~/.tmux.conf 2>/dev/null; then\n  cat >> ~/.tmux.conf << 'TMUXCFGEOF'\n\n# --- Open-TmuxTabs managed settings (do not remove this line) ---\nset -g mouse on\nset -g history-limit 10000\n# --- end Open-TmuxTabs ---\nTMUXCFGEOF\n  tmux source-file ~/.tmux.conf 2>/dev/null && echo 'CFG_DEPLOYED_RELOADED' || echo 'CFG_DEPLOYED_PENDING'\nelse\n  echo 'CFG_OK'\nfi";

const LIST_SESSIONS: &str = "tmux list-sessions -F '#{session_name}' 2>/dev/null || echo 'NO_SESSIONS'";

/// Script for connecting: deploy the tmux config, list the sessions.
pub fn connect_script() -> String {
    format!("{TMUX_CONFIG_BLOCK}\n{LIST_SESSIONS}")
}

/// Script for creating sessions and listing all sessions afterwards.
/// The names must have been validated (`validate::is_valid_session_name`).
pub fn create_script(names: &[String]) -> String {
    let mut parts = vec![TMUX_CONFIG_BLOCK.to_string()];
    for name in names {
        parts.push(format!(
            "tmux new-session -d -s '{name}' 2>/dev/null && echo 'CREATED:{name}' || echo 'EXISTS:{name}'"
        ));
    }
    parts.push(LIST_SESSIONS.to_string());
    parts.join("\n")
}

/// Parsed output of a remote script.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Output {
    pub sessions: Vec<String>,
    pub created: Vec<String>,
}

/// Parses the line protocol. Status lines (`CFG_*`, `EXISTS:`, `NO_SESSIONS`)
/// are dropped, `CREATED:` lines are collected, everything else is a session.
pub fn parse_output(output: &str) -> Output {
    let mut out = Output::default();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match line {
            "CFG_DEPLOYED_RELOADED" | "CFG_DEPLOYED_PENDING" | "CFG_OK" | "NO_SESSIONS" => {}
            _ if line.starts_with("EXISTS:") => {}
            _ if line.starts_with("CREATED:") => out.created.push(line[8..].to_string()),
            _ => out.sessions.push(line.to_string()),
        }
    }
    out
}

/// Interprets `ssh -G` output: returns `user@alias` for an alias, or `None`
/// if no host name was reported. Targets that already contain `@` are not
/// passed here.
pub fn resolve_from_ssh_g(alias: &str, ssh_g_output: &str, fallback_user: &str) -> Option<String> {
    let mut hostname = "";
    let mut user = "";
    for line in ssh_g_output.lines() {
        let mut it = line.trim().splitn(2, char::is_whitespace);
        let (Some(k), Some(v)) = (it.next(), it.next()) else {
            continue;
        };
        match k.to_ascii_lowercase().as_str() {
            "hostname" => hostname = v.trim(),
            "user" => user = v.trim(),
            _ => {}
        }
    }
    if hostname.is_empty() {
        return None;
    }
    let user = if user.is_empty() { fallback_user } else { user };
    Some(format!("{user}@{alias}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts() {
        let c = connect_script();
        assert!(c.contains("set -g mouse on"));
        assert!(c.ends_with("echo 'NO_SESSIONS'"));
        let s = create_script(&["a".into(), "b_1".into()]);
        assert!(s.contains("tmux new-session -d -s 'a' 2>/dev/null && echo 'CREATED:a' || echo 'EXISTS:a'"));
        assert!(s.contains("CREATED:b_1"));
        assert!(s.ends_with("echo 'NO_SESSIONS'"));
    }

    #[test]
    fn parse() {
        let o = parse_output("CFG_OK\nCREATED:x\nEXISTS:y\nx\ny\n\nmain\n");
        assert_eq!(o.created, vec!["x"]);
        assert_eq!(o.sessions, vec!["x", "y", "main"]);
        let o = parse_output("CFG_DEPLOYED_PENDING\nNO_SESSIONS\n");
        assert!(o.sessions.is_empty());
    }

    #[test]
    fn ssh_g() {
        let out = "user jsp\nhostname 10.0.0.1\nport 22\n";
        assert_eq!(resolve_from_ssh_g("devbox", out, "me").as_deref(), Some("jsp@devbox"));
        assert_eq!(
            resolve_from_ssh_g("devbox", "hostname x\n", "me").as_deref(),
            Some("me@devbox")
        );
        assert_eq!(resolve_from_ssh_g("devbox", "port 22\n", "me"), None);
    }
}
