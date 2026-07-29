# Silo — Delivery Roadmap

Incremental, milestone-driven delivery toward the **v0.1 design-forward MVP**.

**How this is organized**
- **Master spec:** [`specs/2026-07-29-silo-notes-design.md`](specs/2026-07-29-silo-notes-design.md) — the whole product design.
- **Milestone sub-specs:** [`specs/milestones/`](specs/milestones/) — one focused sub-spec per milestone (scope, data-model deltas, interfaces, deliverable, verification).
- **Implementation plans:** [`plans/`](plans/) — a full bite-sized TDD plan is written (via the `writing-plans` skill) from each sub-spec **just before** that milestone is built. Planning stays just-in-time, not guessed months ahead.

**Flow per milestone:** sub-spec (exists) → write bite-sized plan → execute task-by-task → verify → commit → next milestone.

| Increment | Milestone | Sub-spec | Plan | Status |
|---|---|---|---|---|
| 1 | **M0–M1** — Skeleton + read a vault | *(covered by the plan)* | [`plans/2026-07-29-silo-m0-m1-foundation.md`](plans/2026-07-29-silo-m0-m1-foundation.md) | Plan ready |
| 2 | **M2** — Edit & save | [`specs/milestones/m2-edit-save.md`](specs/milestones/m2-edit-save.md) | *(write when starting)* | Sub-spec ready |
| 3 | **M3** — Index & search | [`specs/milestones/m3-index-search.md`](specs/milestones/m3-index-search.md) | *(write when starting)* | Sub-spec ready |
| 4 | **M4** — Links & backlinks | [`specs/milestones/m4-links-backlinks.md`](specs/milestones/m4-links-backlinks.md) | *(write when starting)* | Sub-spec ready |
| — | **Polish gate** — every screen intentional in light + dark → **v0.1** | master spec §2 | — | — |

**Beyond v0.1** (master spec §2): Phase 2 (tasks, tags, history), Phase 3 (structured note types — journal/calendar/training/travel), Phase 4 (sync + iPhone). Each becomes its own sub-spec + plan when reached.

**Standing constraints across every milestone** (from the master spec):
- `silo-core` / `silo-markdown` stay GPUI-free and IO-free (mobile-reuse boundary).
- The SQLite index is always rebuildable from the `.md` vault; the vault is the source of truth.
- No component hardcodes a hex color — all from the `Theme` struct. Square corners (0px) everywhere.
- Never lose the user's writing; never fail silently. Malformed notes load as plain text.
- Data stays sync-ready: stable ULIDs, `updated` timestamp on every write, no single-owner-of-files assumption.
