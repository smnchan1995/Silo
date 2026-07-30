# Silo M3: Index & Search + ⌘K Palette — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make the vault instantly searchable and keyboard-navigable: a rebuildable SQLite/FTS5 index, and a ⌘K command palette that fuzzy-finds notes (title + full text) and runs commands.

**Architecture:** A new `silo-index` crate wraps `rusqlite` (bundled SQLite, which ships FTS5). It builds from the in-memory `Notebook` tree, updates incrementally on save, and is disposable (delete `.silo/index.sqlite` → rebuild). `AppState` holds the `Index`, builds it at startup and upserts on autosave. The ⌘K palette is a GPUI overlay (`deferred` + `occlude_mouse`) with a keystroke-driven query, listing index results + commands.

**Tech Stack:** Rust, `rusqlite` (bundled) with FTS5, GPUI (pinned rev `82aef44308540b576e4e51fb379efa71614e5c91`), existing `silo-core`/`silo-markdown`/`silo-vault`.

## Global Constraints

- The index is **derived and disposable**: the `.md` vault is the source of truth; any index error → delete the file and rebuild. The `links`/`tags` tables are populated now (from `silo_markdown`) so M4 can consume them without a schema change.
- `silo-core`/`silo-markdown` stay GPUI-free and IO-free. SQLite lives only in `silo-index`.
- Palette query input is keystroke-driven (`on_key_down`) — no full text-editor machinery needed for a single line.
- GPUI code is representative; reconcile against the pinned rev. `silo-index` logic is unit-tested; palette/UI verified by running.

## Out of scope
Tag-browse view and a persistent sidebar search field (the palette is the search surface for M3); ranking beyond FTS5 bm25; async/background index build (startup build is synchronous — fine for local vaults, noted as a follow-up).

---

### Task 1: `silo-index` — SQLite/FTS5 index (TDD)

**Files:**
- Modify: `crates/silo-index/Cargo.toml` (deps)
- Rewrite: `crates/silo-index/src/lib.rs`

**Interfaces:**
- Produces:
  - `struct SearchHit { id: NoteId, title: String, snippet: String }`
  - `#[derive(thiserror::Error)] enum IndexError { Sqlite(#[from] rusqlite::Error) }`
  - `struct Index { /* conn */ }`
  - `Index::open_or_build(vault_dir: &Path, vault: &Notebook) -> Result<Index, IndexError>` — opens `<vault_dir>/.silo/index.sqlite` (creating dirs), ensures schema, and fully (re)populates from `vault`.
  - `Index::search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, IndexError>` — FTS5 MATCH (prefix on the last token); empty query returns recent notes by `updated` desc.
  - `Index::upsert_note(&self, note: &Note) -> Result<(), IndexError>`
  - `Index::remove_note(&self, id: NoteId) -> Result<(), IndexError>`

- [ ] **Step 1: Deps**

`crates/silo-index/Cargo.toml`:
```toml
[dependencies]
silo-core = { path = "../silo-core" }
silo-markdown = { path = "../silo-markdown" }
rusqlite = { version = "0.32", features = ["bundled"] }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
silo-vault = { path = "../silo-vault" }
```

- [ ] **Step 2: Write failing tests**

`crates/silo-index/src/lib.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn vault_with(dir: &std::path::Path, files: &[(&str, &str)]) -> silo_core::Notebook {
        for (name, body) in files {
            fs::write(dir.join(name), body).unwrap();
        }
        silo_vault::walk_vault(dir).unwrap()
    }

    #[test]
    fn builds_and_searches_body_text() {
        let dir = tempdir().unwrap();
        let vault = vault_with(dir.path(), &[
            ("a.md", "# Alpha\nZettelkasten beats folders"),
            ("b.md", "# Beta\nspaced repetition is useful"),
        ]);
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
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p silo-index builds_and_searches`
Expected: FAIL — `Index` not defined. (If this first FTS5 build errors on `fts5`, the bundled SQLite lacks it — add `features = ["bundled", "bundled-full"]` or a `load_extension`; but bundled ships FTS5, so it should compile.)

- [ ] **Step 4: Implement**

`crates/silo-index/src/lib.rs` (prepend):
```rust
use rusqlite::{params, Connection};
use silo_core::{Note, NoteId};
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
    pub fn open_or_build(vault_dir: &Path, vault: &silo_core::Notebook) -> Result<Index, IndexError> {
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

    fn rebuild(&self, vault: &silo_core::Notebook) -> Result<(), IndexError> {
        self.conn.execute_batch(
            "DELETE FROM notes_fts; DELETE FROM links; DELETE FROM tags;",
        )?;
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
            params![id, note.path.to_string_lossy(), note.title, note.body, note.frontmatter.updated],
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
        self.conn.execute("DELETE FROM notes_fts WHERE id = ?", params![id])?;
        self.conn.execute("DELETE FROM links WHERE from_id = ?", params![id])?;
        self.conn.execute("DELETE FROM tags WHERE note_id = ?", params![id])?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, IndexError> {
        let q = query.trim();
        if q.is_empty() {
            let mut stmt = self.conn.prepare(
                "SELECT id, title FROM notes_fts ORDER BY updated DESC LIMIT ?",
            )?;
            let rows = stmt.query_map(params![limit as i64], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            return rows
                .map(|res| {
                    let (id, title) = res?;
                    Ok(SearchHit { id: parse_id(&id), title, snippet: String::new() })
                })
                .collect();
        }
        // prefix-match the query: sanitize, append '*'
        let safe: String = q.chars().map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' }).collect();
        let match_q = format!("{}*", safe.trim());
        let mut stmt = self.conn.prepare(
            "SELECT id, title, snippet(notes_fts, 3, '[', ']', '…', 12)
             FROM notes_fts WHERE notes_fts MATCH ? ORDER BY rank LIMIT ?",
        )?;
        let rows = stmt.query_map(params![match_q, limit as i64], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?;
        rows.map(|res| {
            let (id, title, snippet) = res?;
            Ok(SearchHit { id: parse_id(&id), title, snippet })
        })
        .collect()
    }
}

fn parse_id(s: &str) -> NoteId {
    s.parse().unwrap_or_else(|_| NoteId::new())
}

fn collect<'a>(nb: &'a silo_core::Notebook, out: &mut Vec<&'a Note>) {
    out.extend(nb.notes.iter());
    for c in &nb.children {
        collect(c, out);
    }
}
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p silo-index`
Expected: PASS (3 tests). If `fts5` errors at runtime ("no such module: fts5"), switch the dep to `rusqlite = { version = "0.32", features = ["bundled"] }` is already correct — bundled enables FTS5; if not, add the `"bundled-sqlcipher"`/full variant. (Should not happen.)

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(index): SQLite/FTS5 index — build, search, upsert, remove"
```

---

### Task 2: hold the index in `AppState`; build on startup, upsert on save

**Files:**
- Modify: `crates/silo-ui/Cargo.toml` (dep on `silo-index`)
- Modify: `crates/silo-ui/src/app_state.rs` (field + upsert in `save_now`)
- Modify: `crates/silo-ui/src/lib.rs` (build in `open_main_window`)

**Interfaces:**
- Consumes: `silo_index::{Index, SearchHit}`.
- Produces: `AppState.index: Option<Index>`; `AppState::search(&self, q, limit) -> Vec<SearchHit>` (empty on no index).

- [ ] **Step 1: Dep + field**

`crates/silo-ui/Cargo.toml`: `silo-index = { path = "../silo-index" }`.
`AppState`: add `pub index: Option<silo_index::Index>,`. Update the two unit-test literals with `index: None,`.

- [ ] **Step 2: Build on startup**

In `open_main_window`, after walking the vault:
```rust
let index = silo_index::Index::open_or_build(&vault_path, &vault).ok();
```
Pass `index` into the `AppState { .. }` literal.

- [ ] **Step 3: Upsert on save + a search helper**

In `save_now`, after a successful `write_note`, also update the index:
```rust
if let Some(idx) = &self.index {
    let _ = idx.upsert_note(&updated);
}
```
Add:
```rust
pub fn search(&self, q: &str, limit: usize) -> Vec<silo_index::SearchHit> {
    self.index.as_ref().and_then(|i| i.search(q, limit).ok()).unwrap_or_default()
}
```

- [ ] **Step 4: Build + test**

Run: `cargo build -p silo && cargo test`
Expected: builds; existing tests still pass (AppState literals updated).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(ui): hold rebuildable index; build on open, upsert on save"
```

---

### Task 3: ⌘K command palette (overlay + live search)

A modal overlay: type to filter, ↑/↓ to move, Enter to open a note / run a command, Esc to close. Results = index search hits (notes) + a small command list ("New note", "Toggle theme").

**Files:**
- Create: `crates/silo-ui/src/palette.rs`
- Modify: `crates/silo-ui/src/app_state.rs` (palette state)
- Modify: `crates/silo-ui/src/lib.rs` (⌘K binding, render overlay, dispatch)

**Interfaces:**
- Produces:
  - `palette::PaletteState { open: bool, query: String, selected: usize }`
  - `palette::render(t, st, cx) -> Option<AnyElement>` — the overlay when open (via `deferred`).
  - Actions `TogglePalette`, `PaletteUp`, `PaletteDown`, `PaletteConfirm`, `PaletteClose`, bound at the root with a `Palette` key context when open.

- [ ] **Step 1: Palette state**

Add to `AppState`: `pub palette: palette::PaletteState` (default closed). Add `palette::PaletteState { pub open: bool, pub query: String, pub selected: usize }` with `Default`.

- [ ] **Step 2: Build the overlay**

`crates/silo-ui/src/palette.rs`: a function that, when `st.palette.open`, returns a centered box over a dimmed, mouse-occluding backdrop (`div().absolute().inset_0()...occlude_mouse()` + a centered card). The card shows: a query line (the current `query` text + a caret block), then up to N rows from `st.search(&query, 20)` plus command rows; the row at `selected` is highlighted (accent bg/text). Use `deferred(...)` so it paints above the panes. Reconcile `absolute`/`inset_0`/`deferred`/`occlude_mouse` against the pinned rev.

- [ ] **Step 3: Keyboard**

Bind (in `bind_editor_keys` or a new `bind_palette_keys`): `cmd-k` → `TogglePalette` (global, no context). When open, bind in `Palette` context: `up`→PaletteUp, `down`→PaletteDown, `enter`→PaletteConfirm, `escape`→PaletteClose. On the root element, set `.key_context("Palette")` when the palette is open and register `.on_action` handlers + `.on_key_down` to append typed characters to `query` (and backspace to pop). Handlers live on `AppState`:
```rust
fn toggle_palette(&mut self, _: &TogglePalette, _w, cx) { self.palette.open = !self.palette.open; self.palette.query.clear(); self.palette.selected = 0; cx.notify(); }
fn palette_down(&mut self, ..) { self.palette.selected += 1; cx.notify(); } // clamp against result count in render/confirm
fn palette_up(&mut self, ..) { self.palette.selected = self.palette.selected.saturating_sub(1); cx.notify(); }
fn palette_confirm(&mut self, _, window, cx) { /* resolve selected → open note via select_note, or run command; close */ }
fn palette_close(&mut self, ..) { self.palette.open = false; cx.notify(); }
```
Character input while open: `.on_key_down(cx.listener(|st, ev, _w, cx| { if st.palette.open { if let Some(c) = ev.keystroke.key_char.as_ref() { st.palette.query.push_str(c); st.palette.selected = 0; cx.notify(); } } }))` — reconcile the `KeyDownEvent`/`Keystroke.key_char` shape against the rev.

- [ ] **Step 4: Confirm actions**

`palette_confirm`: recompute the same ordered list (search hits then commands), pick index `selected`. If a note → `select_note(self, id, window, cx)`. If "New note" → `silo_vault::create_note(vault_dir, "Untitled")`, re-walk vault, upsert to index, select it. Close the palette.

- [ ] **Step 5: Render the overlay in the root**

In `impl Render for AppState`, after the main column, add `.children(palette::render(&t, self, cx))` (an `Option<AnyElement>`), and set `.key_context("Palette")` + palette `on_action`s when `self.palette.open`.

- [ ] **Step 6: Verify by running**

Run: `cargo run -p silo`. Press ⌘K → palette opens; type a word in a note's body → matching notes appear; ↑/↓ moves the highlight; Enter opens the note; Esc closes. "New note" creates and opens a note.

- [ ] **Step 7: Full gate + commit**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
```bash
git add -A && git commit -m "feat(ui): ⌘K command palette with live full-text search"
```

---

## Self-Review

**Spec coverage (M3 sub-spec):** silo-index FTS5 crate → Task 1; build/rebuild + disposable → Task 1 (`open_or_build` rebuilds); incremental update → Task 1 `upsert_note` + Task 2 (save hook); links/tags populated → Task 1 `insert`; full-text search → Task 1 `search`; search UI + ⌘K palette → Task 3 (palette is the search surface). **Deferred (flagged):** dedicated sidebar search field, tag-browse view, background/async index build.

**Placeholder scan:** Task 1 is concrete pure-Rust/SQLite with tests. Tasks 2–3 are integration/GPUI with exact APIs named (`open_or_build`, `upsert_note`, `search`, `deferred`, `occlude_mouse`, `on_key_down`, `Keystroke.key_char`) to reconcile against the rev — not invented.

**Type consistency:** `Index`, `SearchHit`, `IndexError`, `open_or_build`, `search`, `upsert_note`, `remove_note` are consistent across Tasks 1–3 and build on `Note`/`NoteId`/`Notebook` and `silo_markdown::extract_links`/`extract_tags`. `create_note` (unused since M2) is finally consumed by the palette's New-note command.

**Known risks:** (1) FTS5 availability in bundled rusqlite — verified by Task 1 tests; if absent, adjust the feature. (2) The palette's keystroke-driven query is intentionally minimal (no cursor/selection/IME); acceptable for a search box. (3) Startup index build is synchronous — fine for local vaults; move to a background task if large vaults lag.
