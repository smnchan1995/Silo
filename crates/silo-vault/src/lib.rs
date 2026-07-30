mod config;
pub use config::{config_path, load_config, save_config, AppConfig};

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

/// Turn a title into a filesystem-safe slug (alphanumerics, `-` for the rest).
fn slug(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "untitled".into()
    } else {
        s
    }
}

/// Create a new folder under `parent` (name slugged, de-duplicated).
pub fn create_folder(parent: &Path, name: &str) -> Result<PathBuf, VaultError> {
    let base = slug(name);
    let mut path = parent.join(&base);
    let mut i = 2;
    while path.exists() {
        path = parent.join(format!("{base}-{i}"));
        i += 1;
    }
    std::fs::create_dir_all(&path).map_err(|source| VaultError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

/// Soft-delete: move a note or folder into `<vault>/.silo/trash/` (recoverable),
/// rather than permanently removing it. De-duplicates names in the trash.
pub fn trash(vault_root: &Path, target: &Path) -> Result<(), VaultError> {
    let trash_dir = vault_root.join(".silo").join("trash");
    std::fs::create_dir_all(&trash_dir).map_err(|source| VaultError::Io {
        path: trash_dir.clone(),
        source,
    })?;
    let name = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("item");
    let mut dest = trash_dir.join(name);
    let mut i = 1;
    while dest.exists() {
        dest = trash_dir.join(format!("{i}-{name}"));
        i += 1;
    }
    std::fs::rename(target, &dest).map_err(|source| VaultError::Io {
        path: target.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// The directory that holds a note's children: `Foo.md` → `Foo/` (the sibling
/// folder, which may not exist yet).
pub fn children_dir(note_path: &Path) -> PathBuf {
    note_path.with_extension("")
}

/// Create a new note in `dir` with the given title, write it, and return it.
/// Creates `dir` if it doesn't exist (so a note can be added under a leaf note,
/// whose children folder is created on demand).
pub fn create_note(dir: &Path, title: &str) -> Result<Note, VaultError> {
    std::fs::create_dir_all(dir).map_err(|source| VaultError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    let id = NoteId::new();
    let now = now_rfc3339();
    let mut path = dir.join(format!("{}.md", slug(title)));
    if path.exists() {
        path = dir.join(format!("{}-{}.md", slug(title), id));
    }
    let note = Note {
        id,
        path,
        title: title.to_string(),
        frontmatter: Frontmatter {
            id,
            created: now.clone(),
            updated: now,
            tags: vec![],
            pinned: false,
        },
        body: format!("# {title}\n"),
    };
    write_note(&note)?;
    Ok(note)
}

/// Sibling path for preserving a conflicting version, e.g.
/// `inbox.md` + `2026-...` → `inbox.conflict-2026-....md`.
pub fn conflict_path(original: &Path, stamp: &str) -> PathBuf {
    let stem = original
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("note");
    original.with_file_name(format!("{stem}.conflict-{stamp}.md"))
}

/// Atomically write raw bytes to `path` (used for conflict files, which already
/// carry their own frontmatter from disk).
pub fn write_raw(path: &Path, contents: &str) -> Result<(), VaultError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|source| VaultError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    tmp.write_all(contents.as_bytes())
        .map_err(|source| VaultError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    tmp.persist(path).map_err(|e| VaultError::Io {
        path: path.to_path_buf(),
        source: e.error,
    })?;
    Ok(())
}

/// Watch a vault recursively for changes. Returns a channel that yields batches
/// of changed `.md` paths. The watcher runs on its own thread that lives for the
/// life of the app (until the receiver is dropped).
pub fn watch(root: &Path) -> std::sync::mpsc::Receiver<Vec<PathBuf>> {
    use notify::{RecursiveMode, Watcher};
    use std::sync::mpsc::channel;

    let (tx, rx) = channel::<Vec<PathBuf>>();
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let (raw_tx, raw_rx) = channel();
        let mut watcher = match notify::recommended_watcher(move |res| {
            let _ = raw_tx.send(res);
        }) {
            Ok(w) => w,
            Err(_) => return,
        };
        if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
            return;
        }
        for event in raw_rx.into_iter().flatten() {
            let md: Vec<PathBuf> = event
                .paths
                .into_iter()
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
                .collect();
            if !md.is_empty() && tx.send(md).is_err() {
                break; // receiver gone; stop watching
            }
        }
        drop(watcher);
    });
    rx
}

/// Walk the vault root into the unified tree. The root itself is not a note
/// (`note: None`); its children are the top-level notes.
pub fn walk_vault(root: &Path) -> Result<Notebook, VaultError> {
    let name = root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Vault")
        .to_string();
    Ok(Notebook {
        name,
        path: root.to_path_buf(),
        note: None,
        is_virtual: false,
        children: walk_children(root)?,
    })
}

/// Turn a folder slug into a display title for a virtual folder-note:
/// `note-taking` → `Note taking`.
fn folder_title(name: &str) -> String {
    let spaced = name.replace(['-', '_'], " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => spaced,
    }
}

/// Walk a directory into child notes under the sibling-folder convention: `X.md`
/// is a note whose children live in `X/`; a directory with no matching `X.md` is
/// a virtual folder-note. Dotfiles and `.silo` are skipped.
fn walk_children(dir: &Path) -> Result<Vec<Notebook>, VaultError> {
    let entries = fs::read_dir(dir).map_err(|source| VaultError::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    let mut md_files: Vec<PathBuf> = Vec::new();
    let mut subdirs: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue; // skip .silo and all dotfiles
        }
        if path.is_dir() {
            subdirs.push(path);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            md_files.push(path);
        }
    }

    let mut nodes = Vec::new();
    // Real notes; each consumes its sibling `<stem>/` folder as children.
    for md in &md_files {
        let note = read_note(md)?;
        let cdir = children_dir(md);
        let children = if cdir.is_dir() {
            subdirs.retain(|d| d != &cdir);
            walk_children(&cdir)?
        } else {
            Vec::new()
        };
        nodes.push(Notebook {
            name: note.title.clone(),
            path: cdir,
            note: Some(note),
            is_virtual: false,
            children,
        });
    }
    // Remaining directories back no note file → virtual folder-notes.
    for sub in subdirs {
        let fname = sub
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("folder")
            .to_string();
        let would_be = sub.with_extension("md");
        let vid = NoteId::from_path(&would_be);
        let now = now_rfc3339();
        let note = Note {
            id: vid,
            path: would_be,
            title: folder_title(&fname),
            frontmatter: Frontmatter {
                id: vid,
                created: now.clone(),
                updated: now,
                tags: vec![],
                pinned: false,
            },
            body: String::new(),
        };
        let children = walk_children(&sub)?;
        nodes.push(Notebook {
            name: note.title.clone(),
            path: sub,
            note: Some(note),
            is_virtual: true,
            children,
        });
    }
    nodes.sort_by_key(|n| n.name.to_lowercase());
    Ok(nodes)
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
    fn conflict_path_inserts_marker() {
        let p = conflict_path(Path::new("/x/inbox.md"), "2026-07-29T23-47-34Z");
        assert_eq!(
            p,
            PathBuf::from("/x/inbox.conflict-2026-07-29T23-47-34Z.md")
        );
    }

    #[test]
    fn write_raw_writes_contents() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("c.md");
        write_raw(&p, "hello").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "hello");
    }

    #[test]
    fn create_folder_makes_dir() {
        let dir = tempdir().unwrap();
        let p = create_folder(dir.path(), "My Folder").unwrap();
        assert!(p.is_dir());
        assert_eq!(p.file_name().unwrap().to_str().unwrap(), "my-folder");
    }

    #[test]
    fn trash_moves_target_out_of_vault() {
        let dir = tempdir().unwrap();
        let note = dir.path().join("a.md");
        fs::write(&note, "# A\n").unwrap();
        trash(dir.path(), &note).unwrap();
        assert!(!note.exists());
        assert!(dir.path().join(".silo/trash/a.md").exists());
    }

    #[test]
    fn create_note_writes_and_is_readable() {
        let dir = tempdir().unwrap();
        let note = create_note(dir.path(), "My First Note").unwrap();
        assert_eq!(note.title, "My First Note");
        let reread = read_note(&note.path).unwrap();
        assert_eq!(reread.id, note.id);
        assert!(reread.body.contains("My First Note"));
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
                                        // `research/` has no backing `research.md` → a virtual folder-note.
        let research = nb.children.iter().find(|c| c.name == "Research").unwrap();
        assert!(research.is_virtual);
        assert_eq!(research.children.len(), 1); // z.md
        assert!(nb.children.iter().all(|c| c.name != ".silo"));
    }

    #[test]
    fn note_under_note_via_sibling_folder() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("kyoto.md"), "# Kyoto\n").unwrap();
        fs::create_dir(dir.path().join("kyoto")).unwrap();
        fs::write(dir.path().join("kyoto/day-1.md"), "# Day 1\n").unwrap();
        let nb = walk_vault(dir.path()).unwrap();
        // One top-level note (kyoto), which has one child (day-1) — not two siblings.
        assert_eq!(nb.children.len(), 1);
        let kyoto = &nb.children[0];
        assert_eq!(kyoto.name, "Kyoto");
        assert!(!kyoto.is_virtual);
        assert_eq!(kyoto.children.len(), 1);
        assert_eq!(kyoto.children[0].name, "Day 1");
    }
}
