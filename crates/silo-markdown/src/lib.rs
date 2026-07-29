/// Split a leading `---\n ... \n---\n` YAML block from the body.
/// Returns `(Some(yaml_without_delimiters), body)` or `(None, whole_input)`.
pub fn split_frontmatter(raw: &str) -> (Option<&str>, &str) {
    let Some(rest) = raw.strip_prefix("---\n") else {
        return (None, raw);
    };
    // Find the closing delimiter line.
    if let Some(end) = rest.find("\n---\n") {
        let yaml = &rest[..end + 1]; // keep trailing newline of last yaml line
        let body = &rest[end + "\n---\n".len()..];
        (Some(yaml), body)
    } else if let Some(without) = rest.strip_suffix("\n---") {
        (Some(&rest[..without.len() + 1]), "")
    } else {
        (None, raw)
    }
}

/// Title = first `# ` heading, else first non-empty line, else "Untitled".
pub fn derive_title(body: &str) -> String {
    for line in body.lines() {
        if let Some(h) = line.strip_prefix("# ") {
            let t = h.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    for line in body.lines() {
        let t = line.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    "Untitled".to_string()
}

/// Inner text of each `[[...]]`, in order, de-duplicated.
pub fn extract_links(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = body.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(close) = body[i + 2..].find("]]") {
                let inner = body[i + 2..i + 2 + close].trim().to_string();
                if !inner.is_empty() && !out.contains(&inner) {
                    out.push(inner);
                }
                i += 2 + close + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Each `#tag` (alphanumeric/`-`/`_`, not an ATX heading), de-duplicated, without the `#`.
pub fn extract_tags(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        // Skip ATX headings so "# Heading" is not a tag.
        if line.trim_start().starts_with("# ") {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '#' {
                let start = i + 1;
                let mut j = start;
                while j < chars.len()
                    && (chars[j].is_alphanumeric() || chars[j] == '-' || chars[j] == '_')
                {
                    j += 1;
                }
                if j > start {
                    let tag: String = chars[start..j].iter().collect();
                    if !out.contains(&tag) {
                        out.push(tag);
                    }
                }
                i = j;
            } else {
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_leading_frontmatter() {
        let raw = "---\nid: 01\n---\n# Hi\nbody";
        let (fm, body) = split_frontmatter(raw);
        assert_eq!(fm, Some("id: 01\n"));
        assert_eq!(body, "# Hi\nbody");
    }

    #[test]
    fn no_frontmatter_returns_none_and_full_body() {
        let raw = "# Hi\nbody";
        let (fm, body) = split_frontmatter(raw);
        assert_eq!(fm, None);
        assert_eq!(body, "# Hi\nbody");
    }

    #[test]
    fn title_prefers_first_h1() {
        assert_eq!(derive_title("intro\n# Real Title\nx"), "Real Title");
    }

    #[test]
    fn title_falls_back_to_first_nonempty_line() {
        assert_eq!(
            derive_title("\n\nplain first line\nmore"),
            "plain first line"
        );
    }

    #[test]
    fn title_defaults_when_empty() {
        assert_eq!(derive_title("   \n\n"), "Untitled");
    }

    #[test]
    fn extracts_wiki_links_in_order_deduped() {
        let body = "see [[Alpha]] and [[Beta]] and [[Alpha]] again";
        assert_eq!(extract_links(body), vec!["Alpha", "Beta"]);
    }

    #[test]
    fn extracts_tags_without_hash_deduped() {
        let body = "tagged #method and #research and #method";
        assert_eq!(extract_tags(body), vec!["method", "research"]);
    }

    #[test]
    fn heading_hash_is_not_a_tag() {
        assert_eq!(extract_tags("# Heading\ntext"), Vec::<String>::new());
    }
}
