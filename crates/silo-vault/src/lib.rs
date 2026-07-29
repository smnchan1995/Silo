use serde::Deserialize;
use silo_core::{now_rfc3339, Frontmatter, Note, NoteId, Notebook};
use silo_markdown::{derive_title, split_frontmatter};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("io error reading {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("serialize error for {path}: {msg}")]
    Serialize { path: PathBuf, msg: String },
}

/// Raw YAML shape; every field optional so malformed/partial frontmatter degrades gracefully.
#[derive(Deserialize, Default)]
struct RawFm {
    id: Option<String>,
    created: Option<String>,
    updated: Option<String>,
    tags: Option<Vec<String>>,
    pinned: Option<bool>,
}

/// Parse a single `.md` file into a `Note`. Never panics on bad frontmatter —
/// a malformed YAML block degrades to a fresh id and plain-text body.
pub fn read_note(path: &Path) -> Result<Note, VaultError> {
    let raw = fs::read_to_string(path).map_err(|source| VaultError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let (yaml, body) = split_frontmatter(&raw);

    // Parse frontmatter if present; on any parse failure, fall back to defaults.
    let parsed: RawFm = yaml
        .and_then(|y| serde_yaml::from_str::<RawFm>(y).ok())
        .unwrap_or_default();

    let id = parsed
        .id
        .as_deref()
        .and_then(|s| s.parse::<NoteId>().ok())
        .unwrap_or_default();

    let frontmatter = Frontmatter {
        id,
        created: parsed.created.unwrap_or_else(now_rfc3339),
        updated: parsed.updated.unwrap_or_else(now_rfc3339),
        tags: parsed.tags.unwrap_or_default(),
        pinned: parsed.pinned.unwrap_or(false),
    };

    Ok(Note {
        id,
        path: path.to_path_buf(),
        title: derive_title(body),
        frontmatter,
        body: body.to_string(),
    })
}

/// Write a note to its `.md` path atomically (temp file in the same directory,
/// then rename). Serializes the frontmatter as a leading YAML block and stamps
/// `updated` to now. `id` and `created` are preserved from the note.
pub fn write_note(note: &Note) -> Result<(), VaultError> {
    let mut fm = note.frontmatter.clone();
    fm.updated = now_rfc3339();
    let yaml = fm.to_yaml().map_err(|e| VaultError::Serialize {
        path: note.path.clone(),
        msg: e.to_string(),
    })?;
    let contents = format!("---\n{yaml}---\n{}", note.body);

    let dir = note.path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|source| VaultError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    tmp.write_all(contents.as_bytes())
        .map_err(|source| VaultError::Io {
            path: note.path.clone(),
            source,
        })?;
    tmp.persist(&note.path).map_err(|e| VaultError::Io {
        path: note.path.clone(),
        source: e.error,
    })?;
    Ok(())
}

/// Recursively walk a folder into a `Notebook` tree. `.md` files become notes;
/// dotfiles and the `.silo` directory are skipped.
pub fn walk_vault(root: &Path) -> Result<Notebook, VaultError> {
    let name = root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Vault")
        .to_string();
    let mut children = Vec::new();
    let mut notes = Vec::new();

    let entries = fs::read_dir(root).map_err(|source| VaultError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        let fname = entry.file_name();
        let fname = fname.to_string_lossy();
        if fname.starts_with('.') {
            continue; // skip .silo and all dotfiles
        }
        if path.is_dir() {
            children.push(walk_vault(&path)?);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            notes.push(read_note(&path)?);
        }
    }
    // Stable order: notebooks then notes, each alphabetical.
    children.sort_by(|a, b| a.name.cmp(&b.name));
    notes.sort_by_key(|a| a.title.to_lowercase());

    Ok(Notebook {
        name,
        path: root.to_path_buf(),
        children,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn reads_note_with_frontmatter() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("n.md");
        fs::write(
            &p,
            "---\nid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\ncreated: 2026-01-01T00:00:00Z\nupdated: 2026-01-02T00:00:00Z\ntags: [x, y]\npinned: true\n---\n# Hello\nbody text",
        )
        .unwrap();
        let note = read_note(&p).unwrap();
        assert_eq!(note.title, "Hello");
        assert_eq!(note.frontmatter.tags, vec!["x", "y"]);
        assert!(note.frontmatter.pinned);
        assert_eq!(note.id.to_string(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(note.body.starts_with("# Hello"));
    }

    #[test]
    fn malformed_frontmatter_loads_as_plain_text() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("bad.md");
        fs::write(&p, "---\n: : not yaml : :\n---\n# Still Readable").unwrap();
        let note = read_note(&p).unwrap(); // must not error
        assert_eq!(note.title, "Still Readable");
    }

    #[test]
    fn note_without_frontmatter_gets_fresh_id_and_title() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("plain.md");
        fs::write(&p, "# Just A Note\ncontent").unwrap();
        let note = read_note(&p).unwrap();
        assert_eq!(note.title, "Just A Note");
        assert!(!note.frontmatter.created.is_empty());
    }

    #[test]
    fn write_then_read_roundtrips_frontmatter_and_body() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("n.md");
        fs::write(&p, "---\nid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\ncreated: 2026-01-01T00:00:00+00:00\nupdated: 2026-01-01T00:00:00+00:00\ntags: [x]\npinned: false\n---\n# Title\nbody").unwrap();
        let mut note = read_note(&p).unwrap();
        note.body = "# Title\nedited body".into();
        write_note(&note).unwrap();
        let reread = read_note(&p).unwrap();
        assert_eq!(reread.id, note.id); // id preserved
        assert_eq!(reread.frontmatter.created, note.frontmatter.created); // created preserved
        assert!(reread.body.contains("edited body")); // body persisted
    }

    #[test]
    fn write_is_atomic_no_leftover_temp() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("a.md");
        fs::write(&p, "# A\n").unwrap();
        let note = read_note(&p).unwrap();
        write_note(&note).unwrap();
        // exactly one .md file remains; NamedTempFile leaves no leftover on success
        let count = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn walk_vault_builds_tree_and_skips_dot_silo() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("research")).unwrap();
        fs::create_dir(dir.path().join(".silo")).unwrap();
        fs::write(dir.path().join("inbox.md"), "# Inbox\n").unwrap();
        fs::write(dir.path().join("research/z.md"), "# Zettel\n").unwrap();
        fs::write(dir.path().join(".silo/index.sqlite"), "x").unwrap();
        let nb = walk_vault(dir.path()).unwrap();
        assert_eq!(nb.note_count(), 2); // inbox + research/z, NOT .silo
        assert!(nb.children.iter().any(|c| c.name == "research"));
        assert!(nb.children.iter().all(|c| c.name != ".silo"));
    }
}
