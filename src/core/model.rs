//! Application state, independent of the user interface.
//!
//! All selections and inputs of all views live here (not in the controls),
//! so they survive switching views (SPEC-001 §3.2, SPEC-003 §4).

use crate::core::knownhosts::Machine;
use crate::core::validate::parse_names;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pending,
    Connected,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// From `~/.ssh/config`; deleting only hides it.
    Config,
    /// From `known_hosts` (connected as default user); deleting hides it.
    Known,
    /// Added in the app (`n`); stored in the INI.
    Manual,
}

#[derive(Debug, Clone)]
pub struct Host {
    pub target: String,
    pub display: String,
    pub source: Source,
    pub status: Status,
    pub sessions: Vec<String>,
    pub selected: BTreeSet<String>,
    pub new_text: String,
    pub error: String,
    pub filter: String,
    pub last_used: u64,
    pub use_count: u32,
    /// Incremented for every connection attempt; stale results are dropped.
    pub generation: u64,
    /// Name stored in the hidden list when the host is hidden (config
    /// alias or known host name); empty for manual hosts.
    pub hide_key: String,
}

impl Host {
    pub fn new(target: &str, source: Source) -> Host {
        Host {
            target: target.to_string(),
            display: target.to_string(),
            source,
            status: Status::Pending,
            sessions: Vec::new(),
            selected: BTreeSet::new(),
            new_text: String::new(),
            error: String::new(),
            filter: String::new(),
            last_used: 0,
            use_count: 0,
            generation: 0,
            hide_key: String::new(),
        }
    }

    /// Sessions that pass the session filter.
    pub fn visible_sessions(&self) -> Vec<String> {
        self.sessions
            .iter()
            .filter(|s| matches(&self.filter, s))
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Host,
    Session,
}

/// One row of the "Show all" table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub target: String,
    pub display: String,
    pub session: String,
}

/// Summary entry of the Open view: host display name, selected existing
/// sessions, valid new names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenEntry {
    pub target: String,
    pub display: String,
    pub existing: Vec<String>,
    pub new: Vec<String>,
}

/// One (session, host) combination of the Direct session view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectItem {
    pub name: String,
    pub target: String,
    pub display: String,
    pub exists: bool,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub hosts: Vec<Host>,
    pub hidden: Vec<String>,
    /// Host sources (SPEC-003 §3): config aliases, known machines, manual
    /// hosts, and the user for known hosts (empty = ssh default).
    pub config: Vec<String>,
    pub known: Vec<Machine>,
    pub manual: Vec<String>,
    pub default_user: String,
    pub suggest_filter: String,
    pub host_filter: String,
    pub direct_hosts: BTreeSet<String>,
    pub direct_text: String,
    pub direct_filter: String,
    pub all_selected: BTreeSet<(String, String)>,
    pub all_sort: SortKey,
    pub all_asc: bool,
    pub all_filter: String,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            hosts: Vec::new(),
            hidden: Vec::new(),
            config: Vec::new(),
            known: Vec::new(),
            manual: Vec::new(),
            default_user: String::new(),
            suggest_filter: String::new(),
            host_filter: String::new(),
            direct_hosts: BTreeSet::new(),
            direct_text: String::new(),
            direct_filter: String::new(),
            all_selected: BTreeSet::new(),
            all_sort: SortKey::Session,
            all_asc: true,
            all_filter: String::new(),
        }
    }
}

/// Case-insensitive substring match; an empty filter matches everything.
pub fn matches(filter: &str, text: &str) -> bool {
    let f = filter.trim();
    f.is_empty() || text.to_lowercase().contains(&f.to_lowercase())
}

/// "Toggle all" on the visible items: if all are selected, deselect them,
/// otherwise select all of them. Hidden items are not touched.
pub fn toggle_all<K: Ord + Clone>(set: &mut BTreeSet<K>, visible: &[K]) {
    if !visible.is_empty() && visible.iter().all(|k| set.contains(k)) {
        for k in visible {
            set.remove(k);
        }
    } else {
        set.extend(visible.iter().cloned());
    }
}

/// Inverts the selection of the visible items.
pub fn invert<K: Ord + Clone>(set: &mut BTreeSet<K>, visible: &[K]) {
    for k in visible {
        if !set.remove(k) {
            set.insert(k.clone());
        }
    }
}

pub fn toggle<K: Ord + Clone>(set: &mut BTreeSet<K>, k: &K) {
    if !set.remove(k) {
        set.insert(k.clone());
    }
}

/// Host part of `user@host` (or the whole target).
pub fn host_part(target: &str) -> &str {
    target.rsplit_once('@').map(|x| x.1).unwrap_or(target)
}

/// Git hosting services hidden on first start (DEC-077).
pub const DEFAULT_HIDDEN: &[&str] = &[
    "github.com",
    "gitlab.com",
    "bitbucket.org",
    "ssh.dev.azure.com",
    "vs-ssh.visualstudio.com",
    "codeberg.org",
];

/// One entry of the suggestions under "New host".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Primary name.
    pub name: String,
    /// Further names and IP addresses of the same machine.
    pub aliases: Vec<String>,
    /// Text put into the input field.
    pub fill: String,
    pub hidden: bool,
}

impl Model {
    /// Sets all host sources and rebuilds the list.
    pub fn set_sources(&mut self, config: &[String], known: Vec<Machine>, manual: &[String], hidden: &[String]) {
        self.config = config.to_vec();
        self.known = known;
        self.manual = manual.iter().filter(|m| !config.contains(m)).cloned().collect();
        self.hidden = hidden.to_vec();
        self.rebuild();
    }

    pub fn set_default_user(&mut self, user: &str) {
        self.default_user = user.trim().to_string();
        self.rebuild();
    }

    /// SSH target for a known host name.
    pub fn known_target(&self, name: &str) -> String {
        if self.default_user.is_empty() {
            name.to_string()
        } else {
            format!("{}@{name}", self.default_user)
        }
    }

    /// Whether any name of the machine is already used by a config host or
    /// a manual host.
    fn machine_taken(&self, m: &Machine) -> bool {
        let used: Vec<&str> = self
            .config
            .iter()
            .map(String::as_str)
            .chain(self.manual.iter().map(|t| host_part(t)))
            .collect();
        m.names.iter().chain(m.ips.iter()).any(|n| used.contains(&n.as_str()))
    }

    fn machine_hidden(&self, m: &Machine) -> bool {
        m.names.iter().chain(m.ips.iter()).any(|n| self.hidden.contains(n))
    }

    /// Builds the host list: config hosts, known hosts, manual hosts
    /// (SPEC-003 §3), without duplicates; keeps runtime state.
    pub fn rebuild(&mut self) {
        let mut hosts: Vec<Host> = Vec::new();
        let push = |hosts: &mut Vec<Host>, t: &str, source: Source, key: &str| {
            if !hosts.iter().any(|h| h.target == t) {
                let mut h = Host::new(t, source);
                h.hide_key = key.to_string();
                hosts.push(h);
            }
        };
        for t in &self.config {
            if !self.hidden.contains(t) {
                push(&mut hosts, t, Source::Config, t);
            }
        }
        for m in &self.known {
            if m.names.is_empty() || self.machine_taken(m) || self.machine_hidden(m) {
                continue;
            }
            let name = m.names[0].clone();
            push(&mut hosts, &self.known_target(&name), Source::Known, &name);
        }
        for t in &self.manual {
            push(&mut hosts, t, Source::Manual, "");
        }
        for h in hosts.iter_mut() {
            if let Some(old) = self.hosts.iter().find(|o| o.target == h.target) {
                let (source, key) = (h.source, h.hide_key.clone());
                *h = old.clone();
                h.source = source;
                h.hide_key = key;
            }
        }
        self.hosts = hosts;
        let targets: Vec<String> = self.hosts.iter().map(|h| h.target.clone()).collect();
        self.direct_hosts.retain(|t| targets.contains(t));
        self.all_selected.retain(|(t, _)| targets.contains(t));
    }

    pub fn host(&self, target: &str) -> Option<&Host> {
        self.hosts.iter().find(|h| h.target == target)
    }

    pub fn host_mut(&mut self, target: &str) -> Option<&mut Host> {
        self.hosts.iter_mut().find(|h| h.target == target)
    }

    /// Adds a host typed by the user and returns `false` if it is already
    /// in the list. If it matches a hidden config host, or a hidden known
    /// host with the default user, that host is shown again; otherwise it
    /// becomes a manual host.
    pub fn add_manual(&mut self, target: &str) -> bool {
        if self.host(target).is_some() {
            return false;
        }
        if self.config.iter().any(|c| c == target) && self.hidden.iter().any(|h| h == target) {
            self.hidden.retain(|h| h != target);
            self.rebuild();
            return true;
        }
        let machine = self
            .known
            .iter()
            .find(|m| {
                m.names
                    .iter()
                    .chain(m.ips.iter())
                    .any(|n| self.known_target(n) == target)
            })
            .cloned();
        if let Some(m) = machine {
            self.hidden.retain(|h| !m.names.contains(h) && !m.ips.contains(h));
            self.rebuild();
            if self.host(target).is_some() {
                return true;
            }
        }
        self.hidden.retain(|h| h != target);
        self.manual.push(target.to_string());
        self.rebuild();
        true
    }

    /// Removes a host: manual hosts are deleted, config and known hosts are
    /// hidden.
    pub fn remove(&mut self, target: &str) -> Option<Source> {
        let h = self.host(target)?.clone();
        match h.source {
            Source::Manual => self.manual.retain(|t| t != target),
            _ => {
                if !self.hidden.contains(&h.hide_key) {
                    self.hidden.push(h.hide_key.clone());
                }
            }
        }
        self.direct_hosts.remove(target);
        self.all_selected.retain(|(t, _)| t != target);
        self.rebuild();
        Some(h.source)
    }

    pub fn manual_targets(&self) -> Vec<String> {
        self.manual.clone()
    }

    /// Suggestions under "New host": known machines that are not in the
    /// list, and all hidden hosts (DEC-077), filtered.
    pub fn suggestions(&self) -> Vec<Suggestion> {
        let mut out: Vec<Suggestion> = Vec::new();
        let shown: Vec<&str> = self.hosts.iter().map(|h| host_part(&h.target)).collect();
        for m in &self.known {
            let name = m.primary().to_string();
            if name.is_empty() || m.names.iter().chain(m.ips.iter()).any(|n| shown.contains(&n.as_str())) {
                continue;
            }
            if m.names.is_empty() && !self.machine_hidden(m) && self.machine_taken(m) {
                continue;
            }
            let aliases: Vec<String> = m
                .names
                .iter()
                .chain(m.ips.iter())
                .filter(|n| **n != name)
                .cloned()
                .collect();
            out.push(Suggestion {
                fill: self.known_target(&name),
                name,
                aliases,
                hidden: self.machine_hidden(m),
            });
        }
        for h in &self.hidden {
            if out.iter().any(|s| s.name == *h || s.aliases.contains(h)) {
                continue;
            }
            // Preset entries (git hosters) only matter if you ever connected to them.
            if DEFAULT_HIDDEN.contains(&h.as_str()) && !self.config.contains(h) {
                continue;
            }
            let fill = if self.config.contains(h) {
                h.clone()
            } else {
                self.known_target(h)
            };
            out.push(Suggestion {
                name: h.clone(),
                aliases: vec![],
                fill,
                hidden: true,
            });
        }
        out.into_iter()
            .filter(|s| {
                matches(&self.suggest_filter, &s.name) || s.aliases.iter().any(|a| matches(&self.suggest_filter, a))
            })
            .collect()
    }

    /// Indices of hosts that pass the host-list filter.
    pub fn visible_hosts(&self) -> Vec<usize> {
        (0..self.hosts.len())
            .filter(|&i| {
                matches(&self.host_filter, &self.hosts[i].display) || matches(&self.host_filter, &self.hosts[i].target)
            })
            .collect()
    }

    pub fn any_pending(&self) -> bool {
        self.hosts.iter().any(|h| h.status == Status::Pending)
    }

    pub fn record_usage(&mut self, target: &str, now: u64) {
        if let Some(h) = self.host_mut(target) {
            h.use_count += 1;
            h.last_used = now;
        }
    }

    // ---- Open view ------------------------------------------------------

    pub fn open_entries(&self) -> Vec<OpenEntry> {
        self.hosts
            .iter()
            .filter_map(|h| {
                let existing: Vec<String> = h.selected.iter().cloned().collect();
                let new = parse_names(&h.new_text).valid;
                (!existing.is_empty() || !new.is_empty()).then(|| OpenEntry {
                    target: h.target.clone(),
                    display: h.display.clone(),
                    existing,
                    new,
                })
            })
            .collect()
    }

    /// (host, session) pairs of all sessions on connected hosts.
    pub fn all_detected(&self) -> Vec<(String, Vec<String>)> {
        self.hosts
            .iter()
            .filter(|h| h.status == Status::Connected && !h.sessions.is_empty())
            .map(|h| (h.target.clone(), h.sessions.clone()))
            .collect()
    }

    /// Reset after opening from the Open view: clear selections and inputs
    /// of the affected hosts.
    pub fn reset_hosts(&mut self, targets: &[String]) {
        for h in self.hosts.iter_mut().filter(|h| targets.contains(&h.target)) {
            h.selected.clear();
            h.new_text.clear();
        }
    }

    // ---- Direct session -------------------------------------------------

    /// Hosts visible in the Direct session host list.
    pub fn direct_visible(&self) -> Vec<String> {
        self.hosts
            .iter()
            .filter(|h| matches(&self.direct_filter, &h.display) || matches(&self.direct_filter, &h.target))
            .map(|h| h.target.clone())
            .collect()
    }

    /// All (name, host) combinations in host order, then name order.
    pub fn direct_items(&self) -> Vec<DirectItem> {
        let names = parse_names(&self.direct_text).valid;
        let mut out = Vec::new();
        for h in self.hosts.iter().filter(|h| self.direct_hosts.contains(&h.target)) {
            for n in &names {
                out.push(DirectItem {
                    name: n.clone(),
                    target: h.target.clone(),
                    display: h.display.clone(),
                    exists: h.sessions.contains(n),
                });
            }
        }
        out
    }

    // ---- Show all -------------------------------------------------------

    /// All rows of connected hosts, sorted (ties broken by the other column).
    pub fn all_rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = self
            .hosts
            .iter()
            .filter(|h| h.status == Status::Connected)
            .flat_map(|h| {
                h.sessions.iter().map(move |s| Row {
                    target: h.target.clone(),
                    display: h.display.clone(),
                    session: s.clone(),
                })
            })
            .collect();
        rows.sort_by(|a, b| {
            let (x, y) = match self.all_sort {
                SortKey::Host => ((&a.display, &a.session), (&b.display, &b.session)),
                SortKey::Session => ((&a.session, &a.display), (&b.session, &b.display)),
            };
            let o =
                x.0.to_lowercase()
                    .cmp(&y.0.to_lowercase())
                    .then(x.1.to_lowercase().cmp(&y.1.to_lowercase()));
            if self.all_asc {
                o
            } else {
                o.reverse()
            }
        });
        rows
    }

    pub fn all_visible_rows(&self) -> Vec<Row> {
        self.all_rows()
            .into_iter()
            .filter(|r| {
                matches(&self.all_filter, &r.display)
                    || matches(&self.all_filter, &r.session)
                    || matches(&self.all_filter, &r.target)
            })
            .collect()
    }

    /// Selected (host, session) pairs that still exist, in table order.
    pub fn all_selection(&self) -> Vec<(String, String)> {
        self.all_rows()
            .into_iter()
            .map(|r| (r.target, r.session))
            .filter(|k| self.all_selected.contains(k))
            .collect()
    }

    /// Sort by `key`; choosing the same key again flips the direction.
    pub fn sort_all(&mut self, key: SortKey) {
        if self.all_sort == key {
            self.all_asc = !self.all_asc;
        } else {
            self.all_sort = key;
            self.all_asc = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        let mut m = Model::default();
        m.set_sources(
            &["cfg1".into(), "cfg2".into()],
            vec![],
            &["root@m".into(), "cfg1".into()],
            &["cfg2".into()],
        );
        m
    }

    #[test]
    fn sources_and_hiding() {
        let mut m = model();
        let t: Vec<_> = m.hosts.iter().map(|h| (h.target.as_str(), h.source)).collect();
        assert_eq!(t, vec![("cfg1", Source::Config), ("root@m", Source::Manual)]);
        assert_eq!(m.remove("cfg1"), Some(Source::Config));
        assert!(m.hidden.contains(&"cfg1".to_string()));
        assert_eq!(m.remove("root@m"), Some(Source::Manual));
        assert!(m.hosts.is_empty());
        assert!(m.add_manual("cfg2"));
        assert_eq!(m.hosts[0].source, Source::Config);
        assert!(!m.hidden.contains(&"cfg2".to_string()));
        assert!(!m.add_manual("cfg2"));
        assert_eq!(m.manual_targets(), Vec::<String>::new());
    }

    #[test]
    fn reload_keeps_state() {
        let mut m = model();
        m.hosts[0].status = Status::Connected;
        m.hosts[0].sessions = vec!["a".into()];
        m.set_sources(&["cfg1".into()], vec![], &["root@m".into()], &[]);
        assert_eq!(m.hosts[0].status, Status::Connected);
    }

    #[test]
    fn filters_and_toggles() {
        assert!(matches("", "x"));
        assert!(matches("DEV", "root@dev-server"));
        assert!(!matches("prod", "root@dev"));
        let mut set: BTreeSet<String> = BTreeSet::new();
        set.insert("hidden".into());
        let vis = vec!["a".to_string(), "b".to_string()];
        toggle_all(&mut set, &vis);
        assert_eq!(set.len(), 3);
        toggle_all(&mut set, &vis);
        assert_eq!(set.iter().collect::<Vec<_>>(), vec!["hidden"]);
        set.insert("a".into());
        invert(&mut set, &vis);
        assert!(set.contains("b") && !set.contains("a") && set.contains("hidden"));
        let mut m = model();
        m.host_filter = "ROOT".into();
        assert_eq!(m.visible_hosts(), vec![1]);
    }

    #[test]
    fn open_summary() {
        let mut m = model();
        m.hosts[0].selected.insert("x".into());
        m.hosts[1].new_text = "new1\nbad name\nnew1".into();
        let e = m.open_entries();
        assert_eq!(e.len(), 2);
        assert_eq!(e[1].new, vec!["new1"]);
        m.reset_hosts(&["cfg1".into()]);
        assert_eq!(m.open_entries().len(), 1);
    }

    #[test]
    fn direct() {
        let mut m = model();
        m.hosts[0].sessions = vec!["build".into()];
        m.direct_hosts.insert("cfg1".into());
        m.direct_hosts.insert("root@m".into());
        m.direct_text = "build\ndeploy".into();
        let items = m.direct_items();
        assert_eq!(items.len(), 4);
        assert!(items[0].exists && !items[1].exists && !items[2].exists);
        m.direct_filter = "root".into();
        assert_eq!(m.direct_visible(), vec!["root@m"]);
    }

    #[test]
    fn show_all() {
        let mut m = model();
        for (i, s) in [vec!["zeta", "alpha"], vec!["beta"]].into_iter().enumerate() {
            m.hosts[i].status = Status::Connected;
            m.hosts[i].sessions = s.into_iter().map(String::from).collect();
        }
        let names = |m: &Model| m.all_rows().into_iter().map(|r| r.session).collect::<Vec<_>>();
        assert_eq!(names(&m), vec!["alpha", "beta", "zeta"]);
        m.sort_all(SortKey::Session);
        assert_eq!(names(&m), vec!["zeta", "beta", "alpha"]);
        m.sort_all(SortKey::Host);
        assert_eq!(names(&m), vec!["alpha", "zeta", "beta"]);
        m.all_filter = "ET".into();
        assert_eq!(m.all_visible_rows().len(), 2);
        m.all_selected.insert(("root@m".into(), "beta".into()));
        m.all_selected.insert(("gone".into(), "x".into()));
        assert_eq!(m.all_selection(), vec![("root@m".to_string(), "beta".to_string())]);
    }

    fn known() -> Vec<Machine> {
        let text = "pve.x ssh-ed25519 K1\nnala.x ssh-ed25519 K2\npve-nala.x ssh-ed25519 K2\n10.0.0.9 ssh-ed25519 K3\n\
                    github.com ssh-ed25519 K4\ncfgalias ssh-ed25519 K5\nmanual.x ssh-ed25519 K6\n";
        crate::core::knownhosts::machines(&[text.to_string()])
    }

    #[test]
    fn known_hosts_in_list() {
        let mut m = Model {
            default_user: "root".into(),
            ..Default::default()
        };
        let hidden: Vec<String> = DEFAULT_HIDDEN.iter().map(|s| s.to_string()).collect();
        m.set_sources(&["cfgalias".into()], known(), &["jsp@manual.x".into()], &hidden);
        let t: Vec<_> = m.hosts.iter().map(|h| (h.target.as_str(), h.source)).collect();
        assert_eq!(
            t,
            vec![
                ("cfgalias", Source::Config),
                ("root@pve.x", Source::Known),
                ("root@nala.x", Source::Known),
                ("jsp@manual.x", Source::Manual)
            ]
        );
        // Suggestions: the IP-only machine and the hidden github.com.
        let s = m.suggestions();
        assert_eq!(
            s.iter().map(|x| (x.name.as_str(), x.hidden)).collect::<Vec<_>>(),
            vec![("10.0.0.9", false), ("github.com", true)]
        );
        // Hide a known host: it moves to the suggestions, with its alias.
        assert_eq!(m.remove("root@nala.x"), Some(Source::Known));
        assert!(m.hidden.contains(&"nala.x".to_string()));
        let s = m.suggestions();
        let nala = s.iter().find(|x| x.name == "nala.x").unwrap();
        assert!(nala.hidden);
        assert_eq!(nala.aliases, vec!["pve-nala.x"]);
        assert_eq!(nala.fill, "root@nala.x");
        // Taking the suggestion shows it again as a known host.
        assert!(m.add_manual("root@nala.x"));
        assert_eq!(m.host("root@nala.x").unwrap().source, Source::Known);
        assert!(m.manual.len() == 1);
        // Another user for a hidden known host: becomes a manual host, name stays hidden.
        m.remove("root@pve.x");
        assert!(m.add_manual("admin@pve.x"));
        assert_eq!(m.host("admin@pve.x").unwrap().source, Source::Manual);
        assert!(m.host("root@pve.x").is_none());
        // Default user changes the targets.
        m.set_default_user("");
        assert!(m.host("nala.x").is_some());
        // A hidden git host can be taken back.
        assert!(m.add_manual("github.com"));
        assert_eq!(m.host("github.com").unwrap().source, Source::Known);
        // Filter on suggestions.
        m.suggest_filter = "10.0".into();
        assert_eq!(m.suggestions().len(), 1);
    }
}
