//! SQLite/FTS5 index over the vault: full-text search, backlinks, tags.
//! Derived and disposable — the `.md` vault is the source of truth; on any error
//! the `.silo/index.sqlite` file can be deleted and rebuilt.

use rusqlite::{params, Connection};
use silo_core::{Note, NoteId, Notebook};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub id: NoteId,
    pub title: String,
    pub snippet: String,
}

pub struct Index {
    conn: Connection,
}

impl Index {
    /// Open `<vault_dir>/.silo/index.sqlite` (creating dirs), ensure the schema,
    /// and fully (re)populate from `vault`.
    pub fn open_or_build(vault_dir: &Path, vault: &Notebook) -> Result<Index, IndexError> {
        let silo_dir = vault_dir.join(".silo");
        let _ = std::fs::create_dir_all(&silo_dir);
        let conn = Connection::open(silo_dir.join("index.sqlite"))?;
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
                 id UNINDEXED, path UNINDEXED, title, body, updated UNINDEXED
             );
             CREATE TABLE IF NOT EXISTS links(from_id TEXT, to_title TEXT, to_id TEXT);
             CREATE TABLE IF NOT EXISTS tags(note_id TEXT, tag TEXT);",
        )?;
        let idx = Index { conn };
        idx.rebuild(vault)?;
        Ok(idx)
    }

    fn rebuild(&self, vault: &Notebook) -> Result<(), IndexError> {
        self.conn
            .execute_batch("DELETE FROM notes_fts; DELETE FROM links; DELETE FROM tags;")?;
        let mut notes = Vec::new();
        collect(vault, &mut notes);
        for n in notes {
            self.insert(n)?;
        }
        Ok(())
    }

    fn insert(&self, note: &Note) -> Result<(), IndexError> {
        let id = note.id.to_string();
        self.conn.execute(
            "INSERT INTO notes_fts(id, path, title, body, updated) VALUES (?, ?, ?, ?, ?)",
            params![
                id,
                note.path.to_string_lossy(),
                note.title,
                note.body,
                note.frontmatter.updated
            ],
        )?;
        for link in silo_markdown::extract_links(&note.body) {
            self.conn.execute(
                "INSERT INTO links(from_id, to_title, to_id) VALUES (?, ?, NULL)",
                params![id, link],
            )?;
        }
        for tag in silo_markdown::extract_tags(&note.body) {
            self.conn.execute(
                "INSERT INTO tags(note_id, tag) VALUES (?, ?)",
                params![id, tag],
            )?;
        }
        Ok(())
    }

    pub fn upsert_note(&self, note: &Note) -> Result<(), IndexError> {
        self.remove_note(note.id)?;
        self.insert(note)
    }

    pub fn remove_note(&self, id: NoteId) -> Result<(), IndexError> {
        let id = id.to_string();
        self.conn
            .execute("DELETE FROM notes_fts WHERE id = ?", params![id])?;
        self.conn
            .execute("DELETE FROM links WHERE from_id = ?", params![id])?;
        self.conn
            .execute("DELETE FROM tags WHERE note_id = ?", params![id])?;
        Ok(())
    }

    /// Full-text search (prefix-matched). An empty query returns recent notes.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, IndexError> {
        let q = query.trim();
        if q.is_empty() {
            let mut stmt = self
                .conn
                .prepare("SELECT id, title FROM notes_fts ORDER BY updated DESC LIMIT ?")?;
            let rows = stmt.query_map(params![limit as i64], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            return rows
                .map(|res| {
                    let (id, title) = res?;
                    Ok(SearchHit {
                        id: parse_id(&id),
                        title,
                        snippet: String::new(),
                    })
                })
                .collect();
        }
        // Sanitize to alphanumerics/spaces and prefix-match.
        let safe: String = q
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' {
                    c
                } else {
                    ' '
                }
            })
            .collect();
        let match_q = format!("{}*", safe.trim());
        let mut stmt = self.conn.prepare(
            "SELECT id, title, snippet(notes_fts, 3, '[', ']', '…', 12)
             FROM notes_fts WHERE notes_fts MATCH ? ORDER BY rank LIMIT ?",
        )?;
        let rows = stmt.query_map(params![match_q, limit as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        rows.map(|res| {
            let (id, title, snippet) = res?;
            Ok(SearchHit {
                id: parse_id(&id),
                title,
                snippet,
            })
        })
        .collect()
    }
}

fn parse_id(s: &str) -> NoteId {
    s.parse().unwrap_or_else(|_| NoteId::new())
}

fn collect<'a>(nb: &'a Notebook, out: &mut Vec<&'a Note>) {
    out.extend(nb.notes.iter());
    for c in &nb.children {
        collect(c, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn vault_with(dir: &std::path::Path, files: &[(&str, &str)]) -> Notebook {
        for (name, body) in files {
            fs::write(dir.join(name), body).unwrap();
        }
        silo_vault::walk_vault(dir).unwrap()
    }

    #[test]
    fn builds_and_searches_body_text() {
        let dir = tempdir().unwrap();
        let vault = vault_with(
            dir.path(),
            &[
                ("a.md", "# Alpha\nZettelkasten beats folders"),
                ("b.md", "# Beta\nspaced repetition is useful"),
            ],
        );
        let idx = Index::open_or_build(dir.path(), &vault).unwrap();
        let hits = idx.search("zettelkasten", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Alpha");
    }

    #[test]
    fn empty_query_returns_recent() {
        let dir = tempdir().unwrap();
        let vault = vault_with(dir.path(), &[("a.md", "# Alpha\nx"), ("b.md", "# Beta\ny")]);
        let idx = Index::open_or_build(dir.path(), &vault).unwrap();
        assert_eq!(idx.search("", 10).unwrap().len(), 2);
    }

    #[test]
    fn upsert_reflects_new_content_and_remove_drops_it() {
        let dir = tempdir().unwrap();
        let vault = vault_with(dir.path(), &[("a.md", "# Alpha\noriginal")]);
        let idx = Index::open_or_build(dir.path(), &vault).unwrap();
        let mut note = vault.notes[0].clone();
        note.body = "# Alpha\nkryptonite".into();
        idx.upsert_note(&note).unwrap();
        assert_eq!(idx.search("kryptonite", 10).unwrap().len(), 1);
        idx.remove_note(note.id).unwrap();
        assert_eq!(idx.search("kryptonite", 10).unwrap().len(), 0);
    }
}
