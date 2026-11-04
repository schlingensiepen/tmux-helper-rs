//! Translations. The catalogs (`locales/*.po`) are embedded in the EXE.
//!
//! `tr("English text")` returns the text in the active language; texts
//! without a translation stay English. Placeholders use `{name}` and are
//! filled with [`trf`].

use std::collections::HashMap;
use std::sync::RwLock;

/// Supported languages: code, native name, right-to-left.
pub const LANGUAGES: &[(&str, &str, bool)] = &[
    ("en", "English", false),
    ("de", "Deutsch", false),
    ("es", "Español", false),
    ("fr", "Français", false),
    ("eo", "Esperanto", false),
    ("tlh", "tlhIngan Hol", false),
    ("ur", "اردو", true),
];

fn catalog_source(code: &str) -> Option<&'static str> {
    Some(match code {
        "de" => include_str!("../../locales/de.po"),
        "es" => include_str!("../../locales/es.po"),
        "fr" => include_str!("../../locales/fr.po"),
        "eo" => include_str!("../../locales/eo.po"),
        "tlh" => include_str!("../../locales/tlh.po"),
        "ur" => include_str!("../../locales/ur.po"),
        _ => return None,
    })
}

struct Active {
    code: &'static str,
    map: HashMap<String, String>,
}

static ACTIVE: RwLock<Option<Active>> = RwLock::new(None);

/// Normalizes a language code to a supported one, or `None`.
pub fn supported(code: &str) -> Option<&'static str> {
    let primary = code.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    LANGUAGES.iter().find(|(c, _, _)| *c == primary).map(|(c, _, _)| *c)
}

/// Picks the first supported language from the OS preference list
/// (e.g. `["de-DE", "en-US"]`); English if none is supported.
pub fn pick_language(preferred: &[String]) -> &'static str {
    preferred.iter().find_map(|p| supported(p)).unwrap_or("en")
}

/// Activates a language (unknown codes fall back to English).
pub fn set_language(code: &str) {
    let code = supported(code).unwrap_or("en");
    let map = catalog_source(code).map(crate::core::po::parse).unwrap_or_default();
    *ACTIVE.write().unwrap() = Some(Active { code, map });
}

/// The active language code.
pub fn language() -> &'static str {
    ACTIVE.read().unwrap().as_ref().map(|a| a.code).unwrap_or("en")
}

pub fn native_name(code: &str) -> &'static str {
    LANGUAGES
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, n, _)| *n)
        .unwrap_or("English")
}

pub fn is_rtl() -> bool {
    let code = language();
    LANGUAGES.iter().any(|(c, _, rtl)| *c == code && *rtl)
}

/// Translates a text.
pub fn tr(msgid: &str) -> String {
    ACTIVE
        .read()
        .unwrap()
        .as_ref()
        .and_then(|a| a.map.get(msgid).cloned())
        .unwrap_or_else(|| msgid.to_string())
}

/// Translates and fills `{key}` placeholders.
pub fn trf(msgid: &str, args: &[(&str, &str)]) -> String {
    let mut s = tr(msgid);
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test only: the active language is global state.
    #[test]
    fn languages() {
        assert_eq!(pick_language(&["de-DE".into(), "en-US".into()]), "de");
        assert_eq!(pick_language(&["it-IT".into(), "es-ES".into()]), "es");
        assert_eq!(pick_language(&["it-IT".into()]), "en");
        assert_eq!(supported("UR-pk"), Some("ur"));
        set_language("de");
        assert_eq!(language(), "de");
        assert_eq!(tr("Add host"), "Host hinzufügen");
        assert_eq!(trf("Delete host '{name}'?", &[("name", "x")]), "Host 'x' löschen?");
        assert_eq!(tr("no such text"), "no such text");
        assert!(!is_rtl());
        set_language("ur");
        assert!(is_rtl());
        set_language("xx");
        assert_eq!(language(), "en");
        assert_eq!(tr("Add host"), "Add host");
        // Every catalog parses and translates the core texts.
        for (code, _, _) in LANGUAGES.iter().filter(|(c, _, _)| *c != "en") {
            set_language(code);
            assert_ne!(tr("Add host"), "Add host", "{code}");
        }
    }
}

#[cfg(test)]
mod catalog_tests {
    use crate::core::po;

    /// Every catalog translates every msgid of the template, with the same
    /// placeholders.
    #[test]
    fn complete_catalogs() {
        let pot = include_str!("../../locales/messages.pot");
        let ids: Vec<String> = pot
            .lines()
            .filter_map(|l| l.strip_prefix("msgid \""))
            .map(|l| {
                po::parse(&format!("msgid \"{l}\nmsgstr \"x\"\n"))
                    .into_keys()
                    .next()
                    .unwrap_or_default()
            })
            .filter(|s| !s.is_empty())
            .collect();
        assert!(ids.len() > 100);
        let ph = |s: &str| {
            let mut v: Vec<String> = s
                .split('{')
                .skip(1)
                .filter_map(|p| p.split_once('}').map(|x| x.0.to_string()))
                .collect();
            v.sort();
            v
        };
        for (code, src) in [
            ("de", include_str!("../../locales/de.po")),
            ("es", include_str!("../../locales/es.po")),
            ("fr", include_str!("../../locales/fr.po")),
            ("eo", include_str!("../../locales/eo.po")),
            ("tlh", include_str!("../../locales/tlh.po")),
            ("ur", include_str!("../../locales/ur.po")),
        ] {
            let m = po::parse(src);
            for id in &ids {
                let t = m.get(id).unwrap_or_else(|| panic!("{code}: missing {id:?}"));
                assert_eq!(ph(id), ph(t), "{code}: placeholders of {id:?}");
            }
        }
    }

    /// Every text the code translates (`tr("…")`, `trf("…")`, help items,
    /// column titles) is in the template.
    #[test]
    fn code_texts_in_template() {
        let pot = po::parse(&include_str!("../../locales/messages.pot").replace("msgstr \"\"", "msgstr \"x\""));
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = vec![];
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("i18n.rs") {
                    files.push(std::fs::read_to_string(p).unwrap());
                }
            }
        }
        let mut missing = vec![];
        for src in &files {
            for marker in ["tr(\"", "trf(\"", "it(\""] {
                for (i, _) in src.match_indices(marker) {
                    if src[..i]
                        .chars()
                        .last()
                        .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
                    {
                        continue;
                    }
                    let rest = &src[i + marker.len()..];
                    // help items: it("key", "name", "help", ...) -> check name and help
                    let mut strings = vec![];
                    let mut cur = rest;
                    let n = if marker == "it(\"" { 3 } else { 1 };
                    for k in 0..n {
                        let mut out = String::new();
                        let mut chars = cur.char_indices();
                        let mut end = 0;
                        while let Some((j, c)) = chars.next() {
                            match c {
                                '\\' => {
                                    if let Some((_, e)) = chars.next() {
                                        out.push(match e {
                                            'n' => '\n',
                                            o => o,
                                        });
                                    }
                                }
                                '"' => {
                                    end = j;
                                    break;
                                }
                                c => out.push(c),
                            }
                        }
                        if k > 0 || marker != "it(\"" {
                            strings.push(out);
                        }
                        cur = &cur[end + 1..];
                        if let Some(q) = cur.find('"') {
                            cur = &cur[q + 1..];
                        }
                    }
                    for s in strings {
                        if !s.is_empty() && !pot.contains_key(&s) {
                            missing.push(s);
                        }
                    }
                }
            }
        }
        assert!(missing.is_empty(), "not in messages.pot: {missing:?}");
    }
}
