//! Host names from OpenSSH `known_hosts` files (SPEC-003 §3, DEC-077).
//!
//! Every line names one or more hosts and a server key. Names that share a
//! server key are the same machine. Hashed entries (`|1|…`), marker lines
//! (`@revoked`, `@cert-authority`), patterns and ports other than 22 are
//! skipped; plain IP addresses are kept apart from names.

use std::net::IpAddr;
use std::path::Path;

/// One machine: its names and IP addresses, in file order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Machine {
    pub names: Vec<String>,
    pub ips: Vec<String>,
    keys: Vec<String>,
}

impl Machine {
    /// The name to show: the first name, otherwise the first IP address.
    pub fn primary(&self) -> &str {
        self.names
            .first()
            .or(self.ips.first())
            .map(String::as_str)
            .unwrap_or("")
    }
}

fn host_of(entry: &str) -> Option<String> {
    if entry.is_empty() || entry.starts_with('|') || entry.contains(['*', '?', '!']) {
        return None;
    }
    if let Some(rest) = entry.strip_prefix('[') {
        let (h, port) = rest.split_once("]:")?;
        return (port == "22").then(|| h.to_string());
    }
    Some(entry.to_string())
}

/// Parses the contents of several known_hosts files into machines.
pub fn machines(texts: &[String]) -> Vec<Machine> {
    let mut out: Vec<Machine> = Vec::new();
    for text in texts {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('@') {
                continue;
            }
            let mut f = line.split_whitespace();
            let (Some(hosts), Some(_kind), Some(key)) = (f.next(), f.next(), f.next()) else {
                continue;
            };
            let entries: Vec<String> = hosts.split(',').filter_map(host_of).collect();
            if entries.is_empty() {
                continue;
            }
            let key = key.to_string();
            // Machines that share this key or one of these names are merged.
            let hits: Vec<usize> = (0..out.len())
                .filter(|&i| {
                    out[i].keys.contains(&key)
                        || entries
                            .iter()
                            .any(|e| out[i].names.contains(e) || out[i].ips.contains(e))
                })
                .collect();
            let idx = match hits.first() {
                Some(&first) => {
                    for &j in hits.iter().skip(1).rev() {
                        let m = out.remove(j);
                        let t = &mut out[first];
                        for n in m.names {
                            if !t.names.contains(&n) {
                                t.names.push(n);
                            }
                        }
                        for ip in m.ips {
                            if !t.ips.contains(&ip) {
                                t.ips.push(ip);
                            }
                        }
                        t.keys.extend(m.keys);
                    }
                    first
                }
                None => {
                    out.push(Machine::default());
                    out.len() - 1
                }
            };
            let m = &mut out[idx];
            if !m.keys.contains(&key) {
                m.keys.push(key);
            }
            for e in entries {
                if e.parse::<IpAddr>().is_ok() {
                    if !m.ips.contains(&e) {
                        m.ips.push(e);
                    }
                } else if !m.names.contains(&e) {
                    m.names.push(e);
                }
            }
        }
    }
    out
}

/// Reads `<home>/.ssh/known_hosts` and `known_hosts2` (missing files are fine).
pub fn read(home: &Path) -> Vec<Machine> {
    let dir = home.join(".ssh");
    let texts: Vec<String> = ["known_hosts", "known_hosts2"]
        .iter()
        .filter_map(|f| std::fs::read_to_string(dir.join(f)).ok())
        .collect();
    machines(&texts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_group() {
        let text = "\
# comment
alpha.example ssh-ed25519 AAAA1
alpha.example ssh-rsa AAAA2
10.0.0.5 ssh-ed25519 AAAA1
nala.example,10.0.0.7 ssh-ed25519 BBBB
pve-nala.example ssh-ed25519 BBBB
|1|hash=|salt= ssh-ed25519 CCCC
@revoked * ssh-ed25519 DDDD
[odd.example]:2222 ssh-ed25519 EEEE
[std.example]:22 ssh-ed25519 FFFF
10.0.0.9 ssh-ed25519 GGGG
";
        let m = machines(&[text.to_string()]);
        assert_eq!(m.len(), 4);
        assert_eq!(m[0].names, vec!["alpha.example"]);
        assert_eq!(m[0].ips, vec!["10.0.0.5"]);
        assert_eq!(m[1].names, vec!["nala.example", "pve-nala.example"]);
        assert_eq!(m[1].ips, vec!["10.0.0.7"]);
        assert_eq!(m[2].names, vec!["std.example"]);
        assert!(m[3].names.is_empty());
        assert_eq!(m[3].primary(), "10.0.0.9");
    }

    #[test]
    fn merge_later() {
        // Two machines that turn out to be one (a later line links them).
        let text = "a ssh-ed25519 K1\nb ssh-ed25519 K2\na,b ssh-rsa K3\n";
        let m = machines(&[text.to_string()]);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].names, vec!["a", "b"]);
    }
}
