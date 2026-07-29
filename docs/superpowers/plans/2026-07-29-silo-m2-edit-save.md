# Silo M2 (slice 1): Edit a Note & Save It Safely — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make notes editable and persist edits to `.md` safely: an editable body, a real frontmatter-round-tripping atomic writer with `chrono` timestamps, new-note creation, and debounced autosave.

**Architecture:** The persistence spine lands in `silo-vault` (pure-Rust, fully TDD'd): `write_note` (atomic temp-write + rename, YAML frontmatter round-trip, stamps `updated`, preserves `id`/`created`) and `create_note`. The editor is an adapted GPUI text component — GPUI has **no built-in text input**, so we adapt its `crates/gpui/examples/input.rs` (EntityInputHandler + custom Element paint) into a multi-line note editor and bind it to the selected note, saved on a debounce.

**Tech Stack:** Rust, `serde_yaml`, `chrono` (new), `unicode-segmentation` (new, for the editor), GPUI (pinned rev `82aef44308540b576e4e51fb379efa71614e5c91`).

## Global Constraints

- `silo-core` / `silo-markdown` stay GPUI-free and IO-free. `chrono` may be added to `silo-core` if a timestamp type is introduced; the editor deps (`unicode-segmentation`) live only in `silo-ui`.
- **Never lose the user's writing, never fail silently.** Writes are atomic (temp file in the same dir + rename). On write failure, surface it and keep the in-memory copy.
- Frontmatter round-trip must be **lossless**: `read_note` → `write_note` → `read_note` yields an identical `Frontmatter` (`id` and `created` preserved; `updated` is the only field the save changes).
- Notes stay sync-ready: stable ULID, `updated` stamped on every write (RFC3339 UTC via `chrono`).
- GPUI code is representative and must be reconciled against the pinned rev's `crates/gpui/examples/input.rs`. `silo-ui` is verified by running.

## Out of scope (M2 slice 2, a later plan)
Folder picker + persisted vault path in `.silo/settings.json`; `notify` external-edit watcher + conflict (`*.conflict.md`) handling; reopen-last-note. This slice keeps the CLI vault arg from M1 and single-process assumptions.

---

### Task 1: `chrono` timestamps + frontmatter serialization in `silo-core`

Replace M1's placeholder timestamp and give `Frontmatter` a canonical YAML serialization so vault read/write share one format.

**Files:**
- Modify: `crates/silo-core/Cargo.toml` (add `chrono`, `serde` with derive, `serde_yaml`)
- Modify: `crates/silo-core/src/note.rs` (serde on `Frontmatter`; `now_rfc3339` helper; `to_yaml`/`from_yaml`)

**Interfaces:**
- Consumes: existing `NoteId` (needs `Serialize`/`Deserialize` via string form).
- Produces:
  - `silo_core::now_rfc3339() -> String` (RFC3339 UTC via chrono)
  - `Frontmatter::to_yaml(&self) -> Result<String, serde_yaml::Error>`
  - `Frontmatter::from_yaml(&str) -> Result<Frontmatter, serde_yaml::Error>`
  - `NoteId: Serialize + Deserialize` (serializes as its ULID string)

- [ ] **Step 1: Add deps**

`crates/silo-core/Cargo.toml`:
```toml
[dependencies]
ulid = { workspace = true }
serde = { workspace = true }
serde_yaml = { workspace = true }
chrono = { version = "0.4", features = ["clock"] }
```

- [ ] **Step 2: Write failing tests**

Append to `crates/silo-core/src/note.rs` tests module:
```rust
#[test]
fn now_rfc3339_parses_as_datetime() {
    let s = crate::now_rfc3339();
    assert!(chrono::DateTime::parse_from_rfc3339(&s).is_ok(), "not rfc3339: {s}");
}

#[test]
fn frontmatter_yaml_roundtrips() {
    let id = NoteId::new();
    let fm = Frontmatter {
        id,
        created: "2026-01-01T00:00:00+00:00".into(),
        updated: "2026-01-02T00:00:00+00:00".into(),
        tags: vec!["a".into(), "b".into()],
        pinned: true,
    };
    let yaml = fm.to_yaml().unwrap();
    let back = Frontmatter::from_yaml(&yaml).unwrap();
    assert_eq!(fm, back);
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p silo-core frontmatter_yaml_roundtrips`
Expected: FAIL — `to_yaml`/`from_yaml`/serde not defined.

- [ ] **Step 4: Implement serde + helpers**

In `crates/silo-core/src/note.rs`, derive serde on `NoteId` via string, and on `Frontmatter`:
```rust
use serde::{Deserialize, Deserializer, Serialize, Serializer};

impl Serialize for NoteId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for NoteId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}
```
Add `#[derive(Serialize, Deserialize)]` to `Frontmatter` (keep existing `Clone, Debug, PartialEq`), then:
```rust
impl Frontmatter {
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> { serde_yaml::to_string(self) }
    pub fn from_yaml(s: &str) -> Result<Frontmatter, serde_yaml::Error> { serde_yaml::from_str(s) }
}
```
In `crates/silo-core/src/lib.rs`:
```rust
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p silo-core`
Expected: PASS (existing + 2 new).

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(core): chrono timestamps + Frontmatter YAML round-trip"
```

---

### Task 2: atomic `write_note` in `silo-vault`

**Files:**
- Modify: `crates/silo-vault/src/lib.rs` (add `write_note`; switch `now_rfc3339` to `silo_core::now_rfc3339`; add `Serialize` path)

**Interfaces:**
- Consumes: `silo_core::{Note, Frontmatter, now_rfc3339}`; `Frontmatter::to_yaml`.
- Produces: `write_note(note: &Note) -> Result<(), VaultError>` — writes `---\n<yaml>---\n<body>` atomically (temp file in the same directory, then `fs::rename`), stamping `note.frontmatter.updated`. Caller is responsible for setting `updated` OR `write_note` stamps it; **this plan: `write_note` stamps `updated = now` and writes**, returning the value it wrote is not needed (the in-memory note is updated by the caller from disk on next read).

- [ ] **Step 1: Write failing tests**

Add to `crates/silo-vault/src/lib.rs` tests:
```rust
#[test]
fn write_then_read_roundtrips_frontmatter_and_body() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("n.md");
    fs::write(&p, "---\nid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\ncreated: 2026-01-01T00:00:00+00:00\nupdated: 2026-01-01T00:00:00+00:00\ntags: [x]\npinned: false\n---\n# Title\nbody").unwrap();
    let mut note = read_note(&p).unwrap();
    note.body = "# Title\nedited body".into();
    write_note(&note).unwrap();
    let reread = read_note(&p).unwrap();
    assert_eq!(reread.id, note.id);                        // id preserved
    assert_eq!(reread.frontmatter.created, note.frontmatter.created); // created preserved
    assert!(reread.body.contains("edited body"));          // body persisted
}

#[test]
fn write_is_atomic_no_partial_file_on_same_dir() {
    // temp file must be created in the same directory as the target (same filesystem for rename)
    let dir = tempdir().unwrap();
    let p = dir.path().join("a.md");
    fs::write(&p, "# A\n").unwrap();
    let note = read_note(&p).unwrap();
    write_note(&note).unwrap();
    // exactly one .md file remains; no leftover temp
    let count = fs::read_dir(dir.path()).unwrap().filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md")).count();
    assert_eq!(count, 1);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p silo-vault write_then_read_roundtrips`
Expected: FAIL — `write_note` not defined.

- [ ] **Step 3: Implement `write_note`**

In `crates/silo-vault/src/lib.rs`, replace the local `now_rfc3339` with `use silo_core::now_rfc3339;` (delete the placeholder fn) and add:
```rust
use std::io::Write;

pub fn write_note(note: &Note) -> Result<(), VaultError> {
    let mut fm = note.frontmatter.clone();
    fm.updated = now_rfc3339();
    let yaml = fm
        .to_yaml()
        .map_err(|e| VaultError::Serialize { path: note.path.clone(), msg: e.to_string() })?;
    let contents = format!("---\n{yaml}---\n{}", note.body);

    let dir = note.path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)
        .map_err(|source| VaultError::Io { path: dir.to_path_buf(), source })?;
    tmp.write_all(contents.as_bytes())
        .map_err(|source| VaultError::Io { path: note.path.clone(), source })?;
    tmp.persist(&note.path)
        .map_err(|e| VaultError::Io { path: note.path.clone(), source: e.error })?;
    Ok(())
}
```
Add the error variant to `VaultError`:
```rust
    #[error("serialize error for {path}: {msg}")]
    Serialize { path: PathBuf, msg: String },
```
Add to `crates/silo-vault/Cargo.toml` `[dependencies]`: `tempfile = { workspace = true }` (promote from dev — it's now used at runtime).

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p silo-vault`
Expected: PASS (existing 4 + 2 new).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(vault): atomic write_note with frontmatter round-trip"
```

---

### Task 3: `create_note` in `silo-vault`

**Files:**
- Modify: `crates/silo-vault/src/lib.rs`

**Interfaces:**
- Consumes: `silo_core::{Note, NoteId, Frontmatter, now_rfc3339}`, `write_note`.
- Produces: `create_note(dir: &Path, title: &str) -> Result<Note, VaultError>` — new ULID, `created = updated = now`, body `"# <title>\n"`, filename a slug of the title (fallback to the ULID), written via `write_note`, returns the `Note`.

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn create_note_writes_and_is_readable() {
    let dir = tempdir().unwrap();
    let note = create_note(dir.path(), "My First Note").unwrap();
    assert_eq!(note.title, "My First Note");
    let reread = read_note(&note.path).unwrap();
    assert_eq!(reread.id, note.id);
    assert!(reread.body.contains("My First Note"));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p silo-vault create_note_writes`
Expected: FAIL — `create_note` not defined.

- [ ] **Step 3: Implement**

```rust
fn slug(title: &str) -> String {
    let s: String = title.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "untitled".into() } else { s }
}

pub fn create_note(dir: &Path, title: &str) -> Result<Note, VaultError> {
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
        frontmatter: Frontmatter { id, created: now.clone(), updated: now, tags: vec![], pinned: false },
        body: format!("# {title}\n"),
    };
    write_note(&note)?;
    Ok(note)
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p silo-vault` → PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(vault): create_note"
```

---

### Task 4: editable note editor in `silo-ui` (adapt GPUI input example)

The milestone's main lift. GPUI has no text-input widget; adapt `crates/gpui/examples/input.rs` from the pinned rev into a note-body editor. **Before writing code, fetch that example at the pinned SHA and mirror its structure** (`EntityInputHandler`, `actions!`, `KeyBinding`, a custom `Element` with `prepaint`/`paint`, `focus_handle`, grapheme navigation via `unicode-segmentation`). Verified by running, not unit tests.

**Files:**
- Create: `crates/silo-ui/src/editor.rs` (the `NoteEditor` view + `TextElement`)
- Modify: `crates/silo-ui/src/lib.rs` (mount the editor in the reader pane for the selected note; add key bindings/focus)
- Modify: `crates/silo-ui/Cargo.toml` (add `unicode-segmentation`)

**Interfaces:**
- Consumes: `AppState.selected_note()`; the selected `Note`.
- Produces:
  - `editor::NoteEditor` — an entity holding the editable buffer (`content: String`, `selected_range`, `focus_handle`), implementing `Render`, `Focusable`, and `EntityInputHandler`.
  - `NoteEditor::text(&self) -> &str` and `NoteEditor::set_text(&mut self, &str)` — bridge to note body.
  - A callback/hook the app observes on edits (used by Task 5 for autosave) — e.g. `cx.notify()` on each mutation; the app subscribes.

- [ ] **Step 1: Fetch and study the example**

Fetch `https://raw.githubusercontent.com/zed-industries/zed/82aef44308540b576e4e51fb379efa71614e5c91/crates/gpui/examples/input.rs` and note the exact trait/method signatures at this rev (`EntityInputHandler::replace_text_in_range`, `text_for_range`, `selected_text_range`, `Element::{prepaint,paint}`, `KeyBinding::new`, `actions!`).

- [ ] **Step 2: Add dep + create `editor.rs`**

`crates/silo-ui/Cargo.toml` `[dependencies]`: `unicode-segmentation = "1"`.
Create `crates/silo-ui/src/editor.rs` adapting the example: a `NoteEditor` entity with `content: String`, `selected_range: Range<usize>`, `focus_handle: FocusHandle`; a `TextElement` implementing `Element` for cursor/selection paint; `actions!` for Backspace/Delete/Left/Right/Up/Down/etc.; `EntityInputHandler` impl for IME + `replace_text_in_range`. Adapt from single-line to the note body (allow `\n`). Keep grapheme navigation via `unicode_segmentation`.

- [ ] **Step 3: Register actions/keybindings and mount in the reader pane**

In `crates/silo-ui/src/lib.rs`: bind keys (`cx.bind_keys([KeyBinding::new("backspace", Backspace, None), ...])` per the example), give the editor a `focus_handle`, and render `NoteEditor` in the reader pane when a note is selected (replacing the read-only `reader` text for the body; keep the title above it). Seed the editor content from `selected_note().body` when the selection changes.

- [ ] **Step 4: Build and run to verify editing works**

Run: `cargo run -p silo -- ./scratch-vault`
Expected: selecting a note shows its body in an editable field; typing, arrow keys, backspace, and selection work; the cursor renders. (No persistence yet — Task 5.)

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(ui): editable note editor adapted from GPUI input example"
```

---

### Task 5: debounced autosave

Wire edits to `write_note` after a short idle, so edits persist without an explicit save.

**Files:**
- Modify: `crates/silo-ui/src/lib.rs` / `editor.rs` (debounce timer; call into vault)
- Modify: `crates/silo-ui/src/app_state.rs` (hold the editing note's path/frontmatter; apply edited body then save)

**Interfaces:**
- Consumes: `silo_vault::write_note`; `NoteEditor::text`.
- Produces: on edit, schedule a save ~500ms after the last keystroke (`cx.spawn` + timer per the pinned rev's async API); build a `Note` from the selected note's `id`/`created`/`path` + the editor's current text and call `write_note`. On error, log via `tracing` and keep the buffer (never lose edits).

- [ ] **Step 1: Implement debounce + save**

In the editor's edit handler, cancel any pending save task and schedule a new one (store a `Task<()>` handle; dropping it cancels). After the delay, construct the updated `Note` (preserve `id`/`created`/`path`; body = editor text; title re-derived via `silo_markdown::derive_title`) and call `silo_vault::write_note`. Reconcile the timer API against the pinned rev (`cx.background_executor().timer(...)` / `cx.spawn`).

- [ ] **Step 2: Verify end-to-end**

Run: `cargo run -p silo -- ./scratch-vault`, edit a note, wait ~1s, quit, and confirm the `.md` on disk contains the edit:
```bash
cat scratch-vault/inbox.md
```
Expected: edited body persisted; frontmatter intact with a bumped `updated`.

- [ ] **Step 3: Full gate + commit**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
```bash
git add -A && git commit -m "feat(ui): debounced autosave to disk"
```

---

## Self-Review

**Spec coverage (M2 sub-spec, slice 1):** editable buffer → Task 4; debounced autosave → Task 5; atomic writes → Task 2; frontmatter round-trip → Tasks 1–2; real chrono timestamps → Task 1; new-note creation → Task 3. **Deferred to slice 2 (flagged):** folder picker + persisted vault path; external-edit reconciliation + conflict; reopen-last-note.

**Placeholder scan:** pure-Rust tasks (1–3) contain concrete, runnable code + tests. Tasks 4–5 are GPUI and are intentionally specified as "adapt the pinned rev's `input.rs`" with exact traits/integration points named — the ~400-line editor body is adapted example code that must be reconciled against the SHA (documented risk), not invented here.

**Type consistency:** `Frontmatter::{to_yaml,from_yaml}`, `now_rfc3339`, `write_note`, `create_note` signatures are consistent across tasks and match M0-M1 types (`Note`, `Frontmatter`, `NoteId`). `write_note` stamps `updated`; `read_note`'s fallback now uses `silo_core::now_rfc3339` (Task 2 removes the M1 placeholder).

**Known risk:** Task 4 (the editor) is the largest and least certain piece in the whole project — a from-scratch/adapted text editor. If adapting the example proves too large for one task during execution, split it (single-line field first → multi-line) and re-plan per executing-plans guidance.
