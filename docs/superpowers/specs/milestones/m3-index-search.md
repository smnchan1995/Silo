# M3 — Index & Search (Milestone Sub-Spec)

**Parent spec:** `../2026-07-29-silo-notes-design.md`
**Depends on:** M0–M1 (vault read, domain types), M2 (writes — so the index can update on save).
**Status:** Sub-spec ready — write the bite-sized plan (`writing-plans`) before building.

## Goal
Make the vault instantly searchable and command-driven: a rebuildable SQLite/FTS5 index feeding live full-text search and a keyboard command palette.

## In scope
- **`silo-index` crate (fill the M0 stub).** SQLite via `rusqlite` (bundled feature), schema from master spec §4:
  - `notes(id, path, title, updated, pinned)`
  - `notes_fts USING fts5(title, body)`
  - `links(from_id, to_title, to_id)` and `tags(note_id, tag)` — **populated now** (from `silo_markdown::extract_links`/`extract_tags`) so M4 can consume them without a schema change.
- **Build from vault.** `build_index(vault: &Notebook) -> Index` walks all notes and populates every table. Index lives at `<vault>/.silo/index.sqlite`.
- **Incremental update.** On note save/create/delete (from M2), upsert/remove that note's rows — no full rebuild per keystroke.
- **Disposable/rebuildable.** Any open/query error → log, delete `index.sqlite`, rebuild from the vault. Index is never load-bearing.
- **Full-text search.** `search(query: &str, limit) -> Vec<SearchHit>` over FTS5 (title + body), ranked; empty query returns recent notes by `updated`.
- **Search UI.** A search field ("Search notes and text…") that shows live results as you type; selecting a result opens that note.
- **Command palette (⌘K).** Modal overlay, keyboard nav (↑/↓/Enter/Esc), fuzzy over: (a) commands (new note, toggle theme, open vault…), and (b) note titles. Enter runs/opens the selection. Built to Modernist tokens (square corners, accent on the active row).

## Out of scope (later)
- `[[link]]` navigation + backlinks panel (M4) — though M4 reads the `links` table this milestone fills.
- Tag browse/filter view (Phase 2) — the `tags` table is populated, the UI is later.
- Ranking sophistication beyond FTS5 default (bm25) — good enough for v0.1.

## Data-model deltas
- Introduces `silo-index` dependencies: `rusqlite = { version = "0.3x", features = ["bundled"] }`.
- `SearchHit { id: NoteId, title: String, snippet: String }` in `silo-index`.
- No `.md`/frontmatter changes — index is derived only.

## Key interfaces (targets — finalize in the plan)
- `silo_index::Index::open_or_build(vault_dir: &Path, vault: &Notebook) -> Result<Index, IndexError>`
- `silo_index::Index::search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, IndexError>`
- `silo_index::Index::upsert_note(&self, note: &Note)` / `remove_note(&self, id: NoteId)`
- `silo_ui`: search field state + results list; `CommandPalette` overlay component + action registry.

## Deliverable & verification
Open Silo, press ⌘K → palette opens; type to fuzzy-jump to a note or run "New note"; use the search field to find notes by body text; delete `.silo/index.sqlite` while closed → it rebuilds transparently on next launch.
- Unit tests (no GPUI): build index over a temp vault and assert search hits; FTS matches body text not just titles; `upsert_note` reflects edits; `remove_note` drops hits; corrupt/missing db triggers rebuild; `links`/`tags` tables populated from parsed content.
- Run: ⌘K palette keyboard flow; live search returns results as you type.

## Risks
- Keeping index writes off the render thread (master spec: disk work on background executors). Plan must run index ops on a background executor and marshal results back.
- rusqlite `bundled` build time / linking on macOS — verify early in the plan.
