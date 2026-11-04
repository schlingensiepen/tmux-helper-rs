//! Validation of tmux session names typed by the user.

/// A session name may only contain `[a-zA-Z0-9_-]` and must not be empty.
pub fn is_valid_session_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Result of checking a multi-line input field (one name per line).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParsedNames {
    /// Valid names in input order, without duplicates.
    pub valid: Vec<String>,
    /// Lines that are not valid names (trimmed), in input order.
    pub invalid: Vec<String>,
}

/// Splits the text into lines, trims them and sorts them into valid and
/// invalid names. Empty lines are ignored.
pub fn parse_names(text: &str) -> ParsedNames {
    let mut out = ParsedNames::default();
    for line in text.lines() {
        let name = line.trim();
        if name.is_empty() {
            continue;
        }
        if is_valid_session_name(name) {
            if !out.valid.iter().any(|n| n == name) {
                out.valid.push(name.to_string());
            }
        } else {
            out.invalid.push(name.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_names() {
        assert!(is_valid_session_name("build_01-x"));
        assert!(!is_valid_session_name(""));
        assert!(!is_valid_session_name("a b"));
        assert!(!is_valid_session_name("a.b"));
        assert!(!is_valid_session_name("ä"));
        assert!(!is_valid_session_name("a'b"));
    }

    #[test]
    fn parse_lines() {
        let p = parse_names("build\n\n  deploy \r\nbad name\nbuild\nx;y");
        assert_eq!(p.valid, vec!["build", "deploy"]);
        assert_eq!(p.invalid, vec!["bad name", "x;y"]);
    }
}
