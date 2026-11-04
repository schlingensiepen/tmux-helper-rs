//! Application state, independent of the user interface.
//!
//! All selections and inputs of all views live here (not in the controls),
//! so they survive switching views (SPEC-001 §3.2, SPEC-003 §4).

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

impl Model {
    /// Builds the host list: config hosts first (minus hidden ones), then
    /// manual hosts, without duplicates.
    pub fn load_hosts(&mut self, config: &[String], manual: &[String], hidden: &[String]) {
        self.hidden = hidden.to_vec();
        let mut hosts: Vec<Host> = Vec::new();
        for t in config {
            if !hidden.contains(t) && !hosts.iter().any(|h| &h.target == t) {
                hosts.push(Host::new(t, Source::Config));
            }
        }
        for t in manual {
            if !hosts.iter().any(|h| &h.target == t) {
                hosts.push(Host::new(t, Source::Manual));
            }
        }
        // Keep runtime state of hosts that stay.
        for h in hosts.iter_mut() {
            if let Some(old) = self.hosts.iter().find(|o| o.target == h.target) {
                let source = h.source;
                *h = old.clone();
                h.source = source;
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

    /// Adds a host typed by the user. Returns `false` if it already exists.
    /// A hidden config host is shown again instead.
    pub fn add_manual(&mut self, target: &str) -> bool {
        if self.host(target).is_some() {
            return false;
        }
        let was_hidden = self.hidden.iter().any(|h| h == target);
        self.hidden.retain(|h| h != target);
        let source = if was_hidden { Source::Config } else { Source::Manual };
        self.hosts.push(Host::new(target, source));
        true
    }

    /// Removes a host: manual hosts are deleted, config hosts are hidden.
    pub fn remove(&mut self, target: &str) -> Option<Source> {
        let i = self.hosts.iter().position(|h| h.target == target)?;
        let h = self.hosts.remove(i);
        if h.source == Source::Config && !self.hidden.contains(&h.target) {
            self.hidden.push(h.target.clone());
        }
        self.direct_hosts.remove(target);
        self.all_selected.retain(|(t, _)| t != target);
        Some(h.source)
    }

    pub fn manual_targets(&self) -> Vec<String> {
        self.hosts
            .iter()
            .filter(|h| h.source == Source::Manual)
            .map(|h| h.target.clone())
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
        m.load_hosts(
            &["cfg1".into(), "cfg2".into()],
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
        m.load_hosts(&["cfg1".into()], &["root@m".into()], &[]);
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
}
