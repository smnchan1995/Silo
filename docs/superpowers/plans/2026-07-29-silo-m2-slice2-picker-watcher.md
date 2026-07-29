# Silo M2 (slice 2): Folder Picker + External-Edit Reconciliation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Replace the CLI vault arg with a native folder picker whose choice persists across launches (reopening the last vault + note), and reconcile external edits: reload notes changed on disk, and never clobber unsaved edits when a file changes underneath us.

**Architecture:** App-global config (`~/Library/Application Support/Silo/config.json` via the `directories` crate) stores the last vault path, last-open note id, and theme — the "which vault" pointer must live *outside* any vault (the sub-spec's `<vault>/.silo/settings.json` was circular for that field). A `notify` watcher on the vault feeds a channel drained by a GPUI task; changes reload the tree/editor, and an external change to the open note while it has unsaved edits writes a `*.conflict-<ts>.md` sibling rather than losing either version.

**Tech Stack:** Rust, `serde_json` + `directories` (new, in silo-vault), `notify` (new, in silo-vault), GPUI (pinned rev `82aef44308540b576e4e51fb379efa71614e5c91`).

## Global Constraints

- `silo-core` / `silo-markdown` stay GPUI-free and IO-free. Config + watcher IO live in `silo-vault`.
- Never lose the user's writing. External change + unsaved edits ⇒ preserve both (conflict sibling), never silent overwrite.
- Autosave must not feed back into the watcher: ignore change events for a path we just wrote (record last self-write path + time; skip events within a short window).
- Config load never fails the app: any read/parse error returns `AppConfig::default()`.
- GPUI code is representative; reconcile against the pinned rev. `silo-ui` is verified by running; `silo-vault` logic is unit-tested.

## Out of scope
Multi-vault management UI, soft-wrap, and merge/diff UI for conflicts (we only preserve both files). Theme toggle UI (config carries `theme` but wiring a toggle is later).

---

### Task 1: app-global config in `silo-vault`

**Files:**
- Create: `crates/silo-vault/src/config.rs`
- Modify: `crates/silo-vault/src/lib.rs` (`mod config; pub use config::*;`)
- Modify: `crates/silo-vault/Cargo.toml` (add `serde_json`, `directories`)

**Interfaces:**
- Produces:
  - `AppConfig { vault_path: Option<PathBuf>, last_note: Option<String>, theme: String }` (serde, `Default`)
  - `config_path() -> PathBuf` — `<os-config-dir>/Silo/config.json` via `directories`
  - `load_config(path: &Path) -> AppConfig` — default on any error
  - `save_config(path: &Path, cfg: &AppConfig) -> Result<(), VaultError>` — creates parent dirs, atomic write

- [ ] **Step 1: Add deps**

`crates/silo-vault/Cargo.toml` `[dependencies]`:
```toml
serde_json = "1"
directories = "5"
```

- [ ] **Step 2: Write failing tests**

`crates/silo-vault/src/config.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn config_roundtrips() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("sub/config.json");
        let cfg = AppConfig {
            vault_path: Some("/x/vault".into()),
            last_note: Some("01ARZ3NDEKTSV4RRFFQ69G5FAV".into()),
            theme: "dark".into(),
        };
        save_config(&p, &cfg).unwrap();
        assert_eq!(load_config(&p), cfg);
    }

    #[test]
    fn missing_config_is_default() {
        let dir = tempdir().unwrap();
        assert_eq!(load_config(&dir.path().join("nope.json")), AppConfig::default());
    }

    #[test]
    fn malformed_config_is_default() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("bad.json");
        std::fs::write(&p, "{not json").unwrap();
        assert_eq!(load_config(&p), AppConfig::default());
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p silo-vault config_roundtrips`
Expected: FAIL — `AppConfig`/`load_config`/`save_config` not defined.

- [ ] **Step 4: Implement**

Prepend to `crates/silo-vault/src/config.rs`:
```rust
use crate::VaultError;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub vault_path: Option<PathBuf>,
    #[serde(default)]
    pub last_note: Option<String>,
    #[serde(default)]
    pub theme: String,
}

pub fn config_path() -> PathBuf {
    directories::ProjectDirs::from("com", "silo", "Silo")
        .map(|d| d.config_dir().join("config.json"))
        .unwrap_or_else(|| PathBuf::from("silo-config.json"))
}

pub fn load_config(path: &Path) -> AppConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_config(path: &Path, cfg: &AppConfig) -> Result<(), VaultError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| VaultError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| VaultError::Serialize {
        path: path.to_path_buf(),
        msg: e.to_string(),
    })?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|source| VaultError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    tmp.write_all(json.as_bytes()).map_err(|source| VaultError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    tmp.persist(path).map_err(|e| VaultError::Io {
        path: path.to_path_buf(),
        source: e.error,
    })?;
    Ok(())
}
```
Wire in `crates/silo-vault/src/lib.rs` (top): `mod config;` and `pub use config::{load_config, save_config, config_path, AppConfig};`.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p silo-vault` → PASS (existing + 3 new).

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(vault): app-global config (vault path, last note, theme)"
```

---

### Task 2: folder picker + config-driven startup

Replace the CLI-arg vault with: load config → if a valid `vault_path`, open it; else prompt for a folder, save it. Persist `last_note` on selection and reopen it on launch.

**Files:**
- Modify: `crates/silo-ui/src/lib.rs` (`run` signature + startup flow; save `last_note` on select)
- Modify: `crates/silo-ui/src/app_state.rs` (hold `config` + `config_path`; helper to persist)
- Modify: `app/silo/src/main.rs` (drop the CLI arg; call `silo_ui::run()`)

**Interfaces:**
- Consumes: `silo_vault::{AppConfig, load_config, save_config, config_path, walk_vault}`; `cx.prompt_for_paths`.
- Produces: `silo_ui::run() -> anyhow::Result<()>` (no args). On selection, `AppState` writes `config.last_note` and persists.

- [ ] **Step 1: Startup flow**

In `crates/silo-ui/src/lib.rs`, replace `run(vault_path)`:
```rust
pub fn run() -> anyhow::Result<()> {
    application().run(|cx: &mut App| {
        bind_editor_keys(cx);
        let cfg_path = silo_vault::config_path();
        let cfg = silo_vault::load_config(&cfg_path);
        let existing = cfg
            .vault_path
            .as_ref()
            .filter(|p| p.is_dir())
            .cloned();
        match existing {
            Some(vault_path) => open_main_window(cx, cfg_path, cfg, vault_path),
            None => {
                // prompt for a folder, then open
                let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                });
                cx.spawn(async move |cx| {
                    if let Ok(Ok(Some(paths))) = rx.await {
                        if let Some(vault_path) = paths.into_iter().next() {
                            cx.update(|cx| {
                                let mut cfg = cfg;
                                cfg.vault_path = Some(vault_path.clone());
                                let _ = silo_vault::save_config(&cfg_path, &cfg);
                                open_main_window(cx, cfg_path, cfg, vault_path);
                            })
                            .ok();
                        }
                    } else {
                        cx.update(|cx| cx.quit()).ok();
                    }
                })
                .detach();
            }
        }
    });
    Ok(())
}
```
Add a helper `open_main_window(cx, cfg_path, cfg, vault_path)` that walks the vault, creates the editor, seeds `selected`/editor from `cfg.last_note`, and opens the window (moving the existing `open_window` body here). Reconcile `cx.spawn`/`cx.update` and `prompt_for_paths` against the pinned rev.

- [ ] **Step 2: Persist last_note on selection**

`AppState` gains `pub config: AppConfig` and `pub config_path: PathBuf`. In the note-list click handler, after setting `selected`, set `st.config.last_note = Some(id.to_string())` and `let _ = silo_vault::save_config(&st.config_path, &st.config);`. On startup, if `cfg.last_note` parses to a `NoteId` present in the vault, set `selected` and seed the editor with that note's body.

- [ ] **Step 3: Drop the CLI arg**

`app/silo/src/main.rs`:
```rust
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    silo_ui::run()
}
```

- [ ] **Step 4: Verify by running**

Run: `cargo run -p silo`
Expected: first launch shows a native folder picker; pick `scratch-vault`; the window opens on it. Quit and relaunch → it reopens the same vault and last-selected note without prompting. (Delete `~/Library/Application Support/Silo/config.json` to re-trigger the picker.)

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(ui): folder picker + persisted vault/last-note startup"
```

---

### Task 3: external-edit watcher → reload

Watch the vault; when a note file changes on disk (and it isn't our own just-written save), reload the tree and, if the changed note is open with no unsaved edits, refresh the editor.

**Files:**
- Modify: `crates/silo-vault/src/lib.rs` (a `watch(root) -> Receiver<Vec<PathBuf>>` wrapper)
- Modify: `crates/silo-vault/Cargo.toml` (add `notify`)
- Modify: `crates/silo-ui/src/app_state.rs` / `lib.rs` (drain events on a GPUI task; reload; track last self-write)

**Interfaces:**
- Produces:
  - `silo_vault::watch(root: &Path) -> std::sync::mpsc::Receiver<Vec<PathBuf>>` — spawns a `notify` recommended watcher (recursive) whose handler forwards changed paths.
  - `AppState`: `last_self_write: Option<(PathBuf, Instant)>` set in `save_now`; a `reload_paths(&mut self, paths, cx)` that re-walks the vault and, if the open note changed and the editor is not dirty, calls `editor.set_content`.

- [ ] **Step 1: `watch` in silo-vault**

`crates/silo-vault/Cargo.toml`: `notify = "6"`. Add:
```rust
use std::sync::mpsc::{channel, Receiver};

pub fn watch(root: &Path) -> Receiver<Vec<std::path::PathBuf>> {
    use notify::{RecursiveMode, Watcher};
    let (tx, rx) = channel();
    let root = root.to_path_buf();
    // The watcher owns its thread; leak it into a detached thread that lives with the app.
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
        // keep the watcher alive for the life of this thread
        for res in raw_rx {
            if let Ok(event) = res {
                let md: Vec<_> = event
                    .paths
                    .into_iter()
                    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
                    .collect();
                if !md.is_empty() && tx.send(md).is_err() {
                    break;
                }
            }
        }
        drop(watcher);
    });
    rx
}
```

- [ ] **Step 2: Drain events on a GPUI task + reload**

In `open_main_window`, after building `AppState`, start the watcher and a poll loop:
```rust
let rx = silo_vault::watch(&vault_path);
let handle = /* the AppState entity */;
cx.spawn(async move |cx| {
    loop {
        cx.background_executor().timer(std::time::Duration::from_millis(300)).await;
        let mut paths = Vec::new();
        while let Ok(batch) = rx.try_recv() { paths.extend(batch); }
        if paths.is_empty() { continue; }
        if handle.update(cx, |st, cx| st.reload_paths(paths, cx)).is_err() { break; }
    }
}).detach();
```
`reload_paths` skips paths equal to `last_self_write.0` within ~1s (our own autosave), re-walks the vault into `self.vault`, and if the open note's file changed and the editor is not dirty, re-seeds the editor. Reconcile the `try_recv`/spawn against the pinned rev.

- [ ] **Step 3: Verify by running**

Run `cargo run -p silo`, open the vault, then in a terminal `echo '\nexternal line' >> scratch-vault/inbox.md`. Within ~½s the reader (if Inbox is open and untouched) shows the appended line. Editing in-app still autosaves without a reload loop (self-writes are ignored).

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: notify watcher reloads externally-changed notes"
```

---

### Task 4: conflict handling (external change while dirty)

If the open note changes on disk while the editor has unsaved edits, preserve both: write the incoming disk content to a `*.conflict-<ts>.md` sibling and keep the in-editor version (which autosave then persists to the original path).

**Files:**
- Modify: `crates/silo-ui/src/app_state.rs` (dirty tracking; conflict branch in `reload_paths`)
- Modify: `crates/silo-vault/src/lib.rs` (`conflict_path(original, ts) -> PathBuf` helper; reuse `write_note`/raw write)

**Interfaces:**
- `AppState`: track `saved_text: Option<String>` (content as last written by us) to detect "dirty" = editor text != saved_text.
- `silo_vault::write_raw(path: &Path, contents: &str) -> Result<(), VaultError>` — atomic raw write (for the conflict file, which already contains its own frontmatter from disk).

- [ ] **Step 1: Dirty tracking**

Set `self.saved_text = Some(text.clone())` at the end of `save_now` (after a successful write) and when seeding the editor on selection (`saved_text = Some(body)`). "Dirty" = `editor.text() != saved_text.unwrap_or_default()`.

- [ ] **Step 2: Conflict branch**

In `reload_paths`, for the open note's path: if it changed on disk AND the editor is dirty, read the disk bytes and write them to `conflict_path(&note.path, now)` (e.g. `inbox.conflict-2026....md`), and do NOT re-seed the editor (keep the user's edits). Log the conflict path. If not dirty, re-seed as in Task 3.
`silo_vault::conflict_path`:
```rust
pub fn conflict_path(original: &Path, stamp: &str) -> PathBuf {
    let stem = original.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
    original.with_file_name(format!("{stem}.conflict-{stamp}.md"))
}
```

- [ ] **Step 3: Verify by running**

Open Inbox, type (don't wait for autosave — or disable autosave momentarily), then externally `echo ... >> scratch-vault/inbox.md`. Confirm a `inbox.conflict-*.md` appears containing the external version, your in-editor edits remain, and after autosave the original holds your version. Nothing is lost.

- [ ] **Step 4: Full gate + commit**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
```bash
git add -A && git commit -m "feat: preserve both versions on external-edit conflict"
```

---

## Self-Review

**Spec coverage (M2 sub-spec deferred items):** folder picker + persisted vault path → Task 2; reopen last vault/note → Tasks 1–2; external-edit reconciliation (reload) → Task 3; conflict (keep both) → Task 4; self-write ignore → Task 3. **Deviation:** the "which vault" pointer lives in app-global config (`directories`), not `<vault>/.silo/settings.json` (that location is circular for vault_path) — documented in Architecture.

**Placeholder scan:** Task 1 is concrete pure-Rust with tests. Tasks 2–4 are GPUI/IO and name exact APIs (`prompt_for_paths`, `notify::recommended_watcher`, `cx.spawn`/`timer`/`try_recv`) to reconcile against the pinned rev — not invented inline.

**Type consistency:** `AppConfig`, `load_config`/`save_config`/`config_path`, `watch`, `conflict_path`, `write_raw` signatures are consistent across tasks and build on slice-1 (`write_note`, `Note`, `AppState`). `run()` loses its `PathBuf` arg (Task 2) with the matching `main.rs` update.

**Known risks:** (1) bridging `notify`'s thread to GPUI via a polled `mpsc` is deliberately simple (300ms poll) to avoid an async-channel dependency; acceptable latency for reload. (2) The self-write-ignore window (~1s) is heuristic; if flaky, switch to comparing on-disk content against `saved_text` instead of timing.
