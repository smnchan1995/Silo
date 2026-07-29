# M4 — Links & Backlinks (Milestone Sub-Spec) → completes v0.1

**Parent spec:** `../2026-07-29-silo-notes-design.md`
**Depends on:** M0–M1 (parsing already extracts `[[links]]`), M2 (create-on-follow needs writes), M3 (`links` table populated, note-title lookup).
**Status:** Sub-spec ready — write the bite-sized plan (`writing-plans`) before building. This is the last MVP milestone.

## Goal
Make notes a connected graph: `[[wiki-links]]` autocomplete while typing, navigate on click, resolve to stable note IDs, and show a "Linked mentions" backlinks panel — completing the Zettelkasten spine and thus v0.1.

## In scope
- **Link resolution.** Resolve each `[[Title]]` to a `NoteId` by matching note titles (case-insensitive, whitespace-normalized). Store the resolved id in the `links` table (`to_id`); unresolved links keep `to_id = NULL`.
- **Navigation.** Clicking a `[[link]]` in the reader/editor opens the target note. Clicking an unresolved link **creates** it (create-on-follow, via M2 `create_note`) then opens it.
- **Autocomplete.** Typing `[[` opens an inline completion list of note titles (fuzzy), keyboard-navigable; Enter inserts `[[Title]]`.
- **Backlinks panel.** A "Linked mentions" section (right rail or below the note) listing every note whose `links.to_id` equals the current note's id, each showing the source title and the surrounding line as context. Clicking a backlink opens that source note.
- **Live updates.** Editing/saving a note (M2) updates its outgoing links in the index (M3 `upsert_note`), so backlinks stay current.

## Out of scope (post-v0.1)
- Graph visualization view.
- Link aliases / display text (`[[Title|alias]]`) — may be a fast-follow, not required for v0.1.
- Transclusion / embeds.

## Data-model deltas
- None on disk. Uses the `links` table from M3. May add a resolver query:
  `links_to(id) -> Vec<Backlink>` where `Backlink { from_id, from_title, context_line }`.
- Context line may require reading the source note body (via `silo-vault`) or storing a snippet in the index — decide in the plan (prefer storing a snippet to avoid re-reads).

## Key interfaces (targets — finalize in the plan)
- `silo_index::Index::resolve_links(&self)` — fills `to_id` by title match after a build/upsert.
- `silo_index::Index::backlinks(&self, id: NoteId) -> Result<Vec<Backlink>, IndexError>`
- `silo_index::Index::titles_matching(&self, prefix: &str, limit) -> Vec<(NoteId, String)>` (autocomplete source).
- `silo_ui`: link rendering + click handling in the reader; `[[` autocomplete popover; backlinks panel component.

## Deliverable & verification → v0.1 MVP complete
Open Silo, type `[[` and autocomplete an existing note, click the link to jump to it; follow a link to a non-existent note and watch it get created; open a linked note and see its "Linked mentions" listing the notes that point to it, with context; edit a note to add/remove a link and see backlinks update.
- Unit tests (no GPUI, in `silo-index`): title→id resolution (case/space-insensitive), `backlinks` returns correct sources + context, unresolved links stay NULL until target exists, `resolve_links` re-links after a new note is added, autocomplete prefix matching.
- Run: full link flow end-to-end in the app.
- **Polish gate (from master spec §2):** before declaring v0.1 done, every screen must look intentional in **both light and dark**.

## Risks
- Title collisions (two notes with the same title) — plan must define a deterministic tie-break (e.g. most recently updated) and ideally surface ambiguity.
- Autocomplete latency: query the index on a background executor; keep the popover responsive.
