# M2 — Edit & Save (Milestone Sub-Spec)

**Parent spec:** `../2026-07-29-silo-notes-design.md`
**Depends on:** M0–M1 (workspace, theme, `silo-core`, `silo-vault::read_note`/`walk_vault`, three-pane read-only UI).
**Status:** Sub-spec ready — write the bite-sized plan (`writing-plans`) before building.

## Goal
Turn the read-only reader into an editable surface whose changes persist safely to `.md` files, round-trip through frontmatter, and reconcile with external edits — without ever losing writing.

## In scope
- **Editable buffer.** Single contiguous editable text field over the note body (Approach 1 — not block widgets). Lightweight inline affordances only where cheap (heading emphasis, clickable checkbox toggles are optional and may defer to M4/later).
- **Debounced autosave.** After a short idle (e.g. 500ms) since the last keystroke, write the note.
- **Atomic writes.** Write to a temp file in the same directory, then rename over the target, so a crash mid-write cannot corrupt a note.
- **Frontmatter round-trip.** `write_note` serializes `Frontmatter` back to a YAML block; parse → write → parse is identical. On save, stamp `updated`; preserve `id` and `created`.
- **Real timestamps.** Replace M1's placeholder `now_rfc3339` with `chrono` RFC3339 UTC. Add `chrono` to `silo-vault` (and `silo-core` if a timestamp type is introduced).
- **New-note creation.** Create a note in the selected notebook: generate a ULID, write a minimal frontmatter + `# Untitled` body, select it.
- **Folder picker.** Replace M1's CLI-arg vault path with a native folder picker on first launch; persist the chosen vault path in `.silo/settings.json`. Reopen last vault + last note on launch.
- **External-edit reconciliation.** `notify` watcher on the vault: when a note's file changes on disk and the buffer has no unsaved edits, reload it. When it changed on disk *and* there are unsaved edits, detect via `updated` timestamp and surface a non-destructive conflict (keep both — e.g. write a `*.conflict.md` sibling) rather than clobbering.

## Out of scope (later milestones)
- Full-text search / index (M3) — saves need not update any index yet.
- `[[link]]` navigation and backlinks (M4).
- Block-widget editor (Approach 3, post-v0.1).
- Version history / restore (Phase 2).

## Data-model deltas
- `Frontmatter` timestamps may move from `String` to a `chrono::DateTime<Utc>` newtype; if so, update M1's `read_note` and tests accordingly.
- `.silo/settings.json` schema introduced: `{ "vault_path": String, "last_note": Option<String(ULID)>, "theme": "light"|"dark"|"system" }`.

## Key interfaces (targets — finalize in the plan)
- `silo_vault::write_note(note: &Note) -> Result<(), VaultError>` — atomic; serializes frontmatter + body.
- `silo_vault::create_note(dir: &Path, title: &str) -> Result<Note, VaultError>`
- `silo_vault::watch(root: &Path) -> <stream/handle of change events>` (wrap `notify`).
- `silo_core` frontmatter (de)serialization helpers so `read_note`/`write_note` share one format.
- `silo_ui`: editor view bound to the selected note's buffer; autosave debounce; settings load/save; folder-picker entry path.

## Deliverable & verification
Open Silo, pick a vault folder, edit a note, watch it autosave; quit and relaunch → edits and last-open note restored; edit the same file in an external editor → Silo reloads it; edit in both → a conflict sibling is produced, nothing lost.
- Unit tests (no GPUI): `write_note` atomicity (temp+rename), frontmatter round-trip identity, `updated` stamped/`id`+`created` preserved, settings (de)serialize, conflict path chosen when timestamps diverge.
- Run: full editor loop against `scratch-vault`; kill the process mid-edit and confirm no corrupted file.

## Risks
- GPUI text-input/editing primitives are the biggest unknown in the whole project (master spec risk #1). **Spike this first** in the plan: confirm what editable-text support the pinned rev provides before committing the milestone shape.
- Autosave + file watcher can feedback-loop (our own write re-triggers the watcher). Plan must debounce/ignore self-writes.
