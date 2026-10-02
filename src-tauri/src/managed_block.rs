//! Insert/replace/remove a marked block inside a text config file (`~/.gitconfig`,
//! `~/.ssh/config`). Everything outside the markers is left untouched.

fn start_marker(id: &str) -> String {
    format!("# >>> gitswitch:{} >>>", id)
}

fn end_marker(id: &str) -> String {
    format!("# <<< gitswitch:{} <<<", id)
}

/// Removes the block (and one blank line directly before it). No-op if absent.
pub fn remove_block(text: &str, id: &str) -> String {
    let (start, end) = (start_marker(id), end_marker(id));
    let lines: Vec<&str> = text.lines().collect();

    let Some(s) = lines.iter().position(|l| l.trim() == start) else {
        return text.to_string();
    };
    let Some(e) = lines[s..].iter().position(|l| l.trim() == end).map(|i| i + s) else {
        // Unterminated marker: leave the file alone rather than guess.
        return text.to_string();
    };

    let mut from = s;
    if from > 0 && lines[from - 1].trim().is_empty() {
        from -= 1;
    }

    let mut kept: Vec<&str> = Vec::with_capacity(lines.len());
    kept.extend_from_slice(&lines[..from]);
    kept.extend_from_slice(&lines[e + 1..]);
    join_lines(&kept)
}

/// Replaces the block with `body`, appending it at the end of the file. Appending
/// (rather than editing in place) means it wins over earlier `user.*` settings.
pub fn upsert_block(text: &str, id: &str, body: &str) -> String {
    let mut out = remove_block(text, id);
    if !out.is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str(&start_marker(id));
    out.push('\n');
    out.push_str(body.trim_end_matches('\n'));
    out.push('\n');
    out.push_str(&end_marker(id));
    out.push('\n');
    out
}

pub fn has_block(text: &str, id: &str) -> bool {
    let start = start_marker(id);
    text.lines().any(|l| l.trim() == start)
}

fn join_lines(lines: &[&str]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let mut s = lines.join("\n");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_into_empty_file() {
        let out = upsert_block("", "x", "[a]\n  b = 1");
        assert_eq!(out, "# >>> gitswitch:x >>>\n[a]\n  b = 1\n# <<< gitswitch:x <<<\n");
    }

    #[test]
    fn upsert_appends_after_existing_content() {
        let out = upsert_block("[user]\n  name = A\n", "x", "[a]");
        assert!(out.starts_with("[user]\n  name = A\n\n# >>> gitswitch:x >>>"));
        assert!(out.ends_with("# <<< gitswitch:x <<<\n"));
    }

    #[test]
    fn upsert_is_idempotent_and_replaces() {
        let base = "[user]\n  name = A\n";
        let once = upsert_block(base, "x", "[a]");
        assert_eq!(upsert_block(&once, "x", "[a]"), once);
        let changed = upsert_block(&once, "x", "[z]");
        assert!(changed.contains("[z]") && !changed.contains("[a]"));
        assert_eq!(changed.matches(">>> gitswitch:x").count(), 1);
    }

    #[test]
    fn block_moves_to_end_when_content_follows_it() {
        let text = "# >>> gitswitch:x >>>\n[a]\n# <<< gitswitch:x <<<\n[user]\n  name = A\n";
        let out = upsert_block(text, "x", "[a]");
        assert!(out.trim_end().ends_with("# <<< gitswitch:x <<<"));
        assert!(out.starts_with("[user]"));
    }

    #[test]
    fn remove_restores_original() {
        let base = "[user]\n  name = A\n";
        let with = upsert_block(base, "x", "[a]");
        assert_eq!(remove_block(&with, "x"), base);
        assert_eq!(remove_block(&upsert_block("", "x", "[a]"), "x"), "");
    }

    #[test]
    fn blocks_are_independent() {
        let t = upsert_block(&upsert_block("", "one", "[1]"), "two", "[2]");
        let t = remove_block(&t, "one");
        assert!(has_block(&t, "two") && !has_block(&t, "one"));
    }

    #[test]
    fn unterminated_marker_is_left_alone() {
        let text = "# >>> gitswitch:x >>>\n[a]\n";
        assert_eq!(remove_block(text, "x"), text);
    }

    #[test]
    fn handles_crlf_input() {
        let text = "[user]\r\n  name = A\r\n";
        let out = upsert_block(text, "x", "[a]");
        assert!(has_block(&out, "x"));
        assert_eq!(remove_block(&out, "x").lines().count(), 2);
    }
}
