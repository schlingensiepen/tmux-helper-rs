//! Truncation of host names for the host list (SPEC-001 §3.1).
//!
//! All entries get the same width. The longest `user@host` prefix keeps at
//! least [`MIN_DOMAIN_CHARS`] characters of its domain visible; shorter
//! entries show more of their domain; short entries are shown in full.

pub const MIN_DOMAIN_CHARS: usize = 5;

/// Splits `user@host.domain` into (`user@host`, `.domain`).
pub fn split_host(name: &str) -> (&str, &str) {
    let dot = match name.find('@') {
        Some(at) => name[at + 1..].find('.').map(|d| d + at + 1),
        None => name.find('.'),
    };
    match dot {
        Some(d) => (&name[..d], &name[d..]),
        None => (name, ""),
    }
}

/// Returns the truncated names (same order as the input) and the common
/// width in characters. `max_width` is a soft limit (in characters).
pub fn compute(names: &[String], max_width: Option<usize>) -> (Vec<String>, usize) {
    if names.is_empty() {
        return (Vec::new(), 0);
    }
    let len = |s: &str| s.chars().count();
    let mut max_prefix = 0;
    let mut max_full = 0;
    let mut has_domain = false;
    for n in names {
        let (p, d) = split_host(n);
        max_prefix = max_prefix.max(len(p));
        max_full = max_full.max(len(n));
        has_domain |= !d.is_empty();
    }
    let min_required = if has_domain {
        max_prefix + 1 + MIN_DOMAIN_CHARS
    } else {
        max_prefix
    };
    let width = match max_width {
        None => max_full,
        Some(m) if max_full <= m => max_full,
        Some(m) => min_required.max(m),
    };
    let out = names
        .iter()
        .map(|n| {
            if len(n) <= width {
                n.clone()
            } else {
                n.chars().take(width).collect()
            }
        })
        .collect();
    (out, width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split() {
        assert_eq!(split_host("root@dev.example.com"), ("root@dev", ".example.com"));
        assert_eq!(split_host("root@dev"), ("root@dev", ""));
        assert_eq!(split_host("a.b.c"), ("a", ".b.c"));
        assert_eq!(split_host("j.s@h.x"), ("j.s@h", ".x"));
    }

    #[test]
    fn truncation() {
        let names = vec![
            "root@dev-server.example.com".to_string(),
            "jsp@build.corp.example.com".to_string(),
            "admin@ci.lo".to_string(),
        ];
        let (all, w) = compute(&names, None);
        assert_eq!(w, 27);
        assert_eq!(all[2], "admin@ci.lo");
        let (t, w) = compute(&names, Some(10));
        // longest prefix "root@dev-server" (15) + "." + 5
        assert_eq!(w, 21);
        assert_eq!(t[0], "root@dev-server.examp");
        assert_eq!(t[2], "admin@ci.lo");
        let (t, w) = compute(&names, Some(24));
        assert_eq!(w, 24);
        assert_eq!(t[1].chars().count(), 24);
    }
}
