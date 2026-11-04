//! A small INI reader/writer (UTF-8, order preserving, case-insensitive
//! section and key names). Used for `OpenTmuxTabs.ini`.

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Ini {
    sections: Vec<(String, Vec<(String, String)>)>,
}

impl Ini {
    pub fn parse(text: &str) -> Ini {
        let mut ini = Ini::default();
        let mut cur: Option<usize> = None;
        for raw in text.trim_start_matches('\u{feff}').lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                let name = line[1..line.len() - 1].trim().to_string();
                cur = Some(ini.section_index_or_add(&name));
                continue;
            }
            if let (Some(i), Some(eq)) = (cur, line.find('=')) {
                let k = line[..eq].trim().to_string();
                let v = line[eq + 1..].trim().to_string();
                let sec = &mut ini.sections[i].1;
                match sec.iter_mut().find(|(kk, _)| kk.eq_ignore_ascii_case(&k)) {
                    Some(e) => e.1 = v,
                    None => sec.push((k, v)),
                }
            }
        }
        ini
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (name, entries) in &self.sections {
            out += &format!("[{name}]\r\n");
            for (k, v) in entries {
                out += &format!("{k}={v}\r\n");
            }
            out += "\r\n";
        }
        out
    }

    fn section_index(&self, name: &str) -> Option<usize> {
        self.sections.iter().position(|(n, _)| n.eq_ignore_ascii_case(name))
    }

    fn section_index_or_add(&mut self, name: &str) -> usize {
        match self.section_index(name) {
            Some(i) => i,
            None => {
                self.sections.push((name.to_string(), Vec::new()));
                self.sections.len() - 1
            }
        }
    }

    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        let i = self.section_index(section)?;
        self.sections[i]
            .1
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    pub fn get_i32(&self, section: &str, key: &str) -> Option<i32> {
        self.get(section, key)?.parse().ok()
    }

    pub fn set(&mut self, section: &str, key: &str, value: impl Into<String>) {
        let i = self.section_index_or_add(section);
        let v = value.into();
        let sec = &mut self.sections[i].1;
        match sec.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(key)) {
            Some(e) => e.1 = v,
            None => sec.push((key.to_string(), v)),
        }
    }

    pub fn remove(&mut self, section: &str, key: &str) {
        if let Some(i) = self.section_index(section) {
            self.sections[i].1.retain(|(k, _)| !k.eq_ignore_ascii_case(key));
        }
    }

    /// All (key, value) pairs of a section, in file order.
    pub fn entries(&self, section: &str) -> Vec<(String, String)> {
        self.section_index(section)
            .map(|i| self.sections[i].1.clone())
            .unwrap_or_default()
    }

    /// Replaces a whole section (used for lists such as `Hosts`).
    pub fn set_section(&mut self, section: &str, entries: Vec<(String, String)>) {
        let i = self.section_index_or_add(section);
        self.sections[i].1 = entries;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut ini =
            Ini::parse("\u{feff}; comment\n[Settings]\nHotkeyKey = 84\nLanguage=de\n\n[Splits]\nhost:root@x=400,700\n");
        assert_eq!(ini.get("settings", "hotkeykey"), Some("84"));
        assert_eq!(ini.get_i32("Settings", "HotkeyKey"), Some(84));
        assert_eq!(ini.get("Splits", "host:root@x"), Some("400,700"));
        ini.set("Settings", "Language", "ur");
        ini.set("Window", "X", "10");
        ini.remove("Splits", "host:root@x");
        let again = Ini::parse(&ini.to_text());
        assert_eq!(again.get("Settings", "Language"), Some("ur"));
        assert_eq!(again.get("Window", "X"), Some("10"));
        assert_eq!(again.get("Splits", "host:root@x"), None);
        assert_eq!(again, ini);
    }

    #[test]
    fn sections() {
        let mut ini = Ini::default();
        ini.set_section("Hosts", vec![("1".into(), "a".into()), ("2".into(), "اردو".into())]);
        let again = Ini::parse(&ini.to_text());
        assert_eq!(again.entries("Hosts")[1].1, "اردو");
    }
}
