//! Minimal gettext `.po` parser (msgid/msgstr, multi-line strings, escapes).
//! Entries with an empty translation are skipped, so the English text is
//! shown instead. Plural forms are not used by this application.

use std::collections::HashMap;

fn unquote(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    Some(out)
}

#[derive(PartialEq)]
enum Field {
    None,
    Id,
    Str,
}

/// Parses a `.po` file into a msgid → msgstr map.
pub fn parse(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut id = String::new();
    let mut st = String::new();
    let mut field = Field::None;
    let flush = |id: &mut String, st: &mut String, map: &mut HashMap<String, String>| {
        if !id.is_empty() && !st.is_empty() {
            map.insert(std::mem::take(id), std::mem::take(st));
        }
        id.clear();
        st.clear();
    };
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            if field == Field::Str {
                flush(&mut id, &mut st, &mut map);
                field = Field::None;
            }
            continue;
        }
        if let Some(rest) = l.strip_prefix("msgid ") {
            if field == Field::Str {
                flush(&mut id, &mut st, &mut map);
            }
            id = unquote(rest).unwrap_or_default();
            field = Field::Id;
        } else if let Some(rest) = l.strip_prefix("msgstr ") {
            st = unquote(rest).unwrap_or_default();
            field = Field::Str;
        } else if l.starts_with('"') {
            let s = unquote(l).unwrap_or_default();
            match field {
                Field::Id => id += &s,
                Field::Str => st += &s,
                Field::None => {}
            }
        }
    }
    if field == Field::Str {
        flush(&mut id, &mut st, &mut map);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_po() {
        let po = r#"
msgid ""
msgstr ""
"Language: de\n"

#. comment
msgid "Add host"
msgstr "Host hinzufügen"

msgid "Line "
"two"
msgstr "Zeile\n"
"zwei \"x\""

msgid "Untranslated"
msgstr ""
"#;
        let m = parse(po);
        assert_eq!(m.get("Add host").map(String::as_str), Some("Host hinzufügen"));
        assert_eq!(m.get("Line two").map(String::as_str), Some("Zeile\nzwei \"x\""));
        assert!(!m.contains_key("Untranslated"));
        assert!(!m.contains_key(""));
    }
}
