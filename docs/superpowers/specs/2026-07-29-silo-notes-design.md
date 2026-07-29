# Silo — Design Spec

**Date:** 2026-07-29
**Status:** Approved (brainstorming), ready for implementation planning
**Design reference:** `references/Silo Prototype.html` — the interactive "Modernist" prototype this spec is derived from.

> A fast, keyboard-driven, local-first notes app for macOS, built in Rust on Zed's GPUI framework. The wedge is **design/taste**; performance and opinionated structure back it up.

---

## 1. Product Context

**Who it's for.** Primarily a **real product for other people**, with strong personal-daily-driver motivation and learning value along the way. Decisions favor a product-serious result, but the hard/interesting parts (native GPU UI, custom editor, indexing) are welcome, not avoided.

**The wedge, in priority order.** These decide what we protect when we cut scope:
1. **Design / taste** *(highest)* — the best-looking, calmest, most focused notes app on the Mac. The "Modernist" aesthetic is the product, not decoration.
2. **Feel / performance** — native GPUI, Zed-smooth. Obsidian is Electron and feels it; Silo is not.
3. **Opinionated structure** — batteries-included note shapes (journal, tasks-linked-to-notes, travel, training). Later phases.

**Long-term direction (NOT built in v0.1, but shapes the data model now).**
- **Multi-device: Mac + iPhone later.** GPUI does not target iOS, so a mobile app is a **separate codebase** (likely SwiftUI) that shares *data*, not code. The pure-Rust `silo-core` / `silo-markdown` crates are the only realistic code reuse.
- **Sync eventually** — likely iCloud/CloudKit or a file-sync folder, possibly a dedicated service. We build none of it now, but the data model must not preclude it.

---

## 2. Scope

### v0.1 — Design-forward Lean-core MVP (the thing we show people)
The note-taking spine, executed to a high visual polish. Every screen must look intentional in **both light and dark** before v0.1 is "done." Polish is a feature, not a finishing step.

- Vault: pick/open a folder; walk it into a notebook (folder) tree.
- Sidebar: notebook tree navigation, collapsible.
- Note list: notes in selected notebook, sorted by `updated`; pinned float to top.
- Markdown editor: **single editable buffer** with lightweight inline styling (see §5).
- Wiki-links: `[[` autocomplete over note titles; click to navigate; create-on-follow for missing targets.
- Backlinks panel: "Linked mentions" for the current note.
- Full-text search: FTS5-backed, results as you type.
- Command palette (⌘K): modal, keyboard nav (↑/↓/Enter/Esc), fuzzy over commands + notes.
- Light/dark theme: full palette remap; follows system or manual toggle.
- Persistence: debounced atomic autosave to `.md`; reopen last note on launch.
- External-edit reconciliation: `notify` watcher reloads notes changed on disk.

### Considered future phases (documented, not built)
- **Phase 2 — Capture & doing:** tasks/checklists with due dates and note-of-origin links; Inbox quick-capture; tags browse/filter view; note version history/restore; drag-to-reorder. *(This is the "opinionated structure" wedge showing up.)*
- **Phase 3 — Structured note types (full prototype parity):** journal + habit/mood logging; calendar Today/Week/Month; training log; travel itinerary board; configurable right "day rail."
- **Phase 4 — Multi-device:** sync + iPhone companion app.

**Explicit non-goals for v0.1:** no sync, no mobile, no plugins, no collaboration, no block-widget editor (see §5), no structured note types.

---

## 3. Architecture

All domain logic lives in plain Rust crates that **don't know GPUI exists**; the UI is a thin rendering + action layer. This keeps the core testable (GPUI is hard to unit-test) and lets a future mobile app reuse the non-UI crates.

```
silo/  (Cargo workspace)
├── crates/
│   ├── silo-core/      # domain types: Note, Notebook, Link, Tag, NoteId. No IO, no UI.
│   ├── silo-vault/     # filesystem: read/write .md, frontmatter, notify watcher
│   ├── silo-index/     # SQLite + FTS5: build, query, incremental update
│   ├── silo-markdown/  # parse md → block tree; extract [[links]] & #tags (block-ready)
│   └── silo-ui/        # GPUI: shell, panes, overlays, theme, actions
└── app/silo/           # thin binary: wires crates, opens window
```

```
┌── UI (GPUI) ──────────────────────────────────┐
│  AppShell → Sidebar │ NoteList │ Editor │ Backlinks
│  Overlays: CommandPalette (⌘K), Search          │
└──────────────┬─────────────────────────────────┘
        actions │ ▲ observed state changes
┌──────────────▼─────────────────────────────────┐
│  Core state (Workspace, AppState)               │
└──────┬───────────────────────────────┬──────────┘
   ┌───▼──── Vault (fs) ────┐   ┌───────▼──── Index (SQLite/FTS5) ──┐
   │ read/write .md + watch │──▶│ search · backlinks · tags          │  (rebuildable)
   └────────────────────────┘   └────────────────────────────────────┘
```

**Load-bearing boundaries:**
- `silo-core` + `silo-markdown` are pure Rust with no macOS/GPUI ties — the reuse surface for a future mobile app, and the reason the Approach-1→3 editor evolution is not a rewrite.
- **The index is disposable.** The vault (`.md` files) is always source of truth. Delete `.silo/index.sqlite` and it rebuilds. The index never has to sync.
- Disk/index work stays on background executors; the render thread only touches in-memory state (this is where "Zed-smooth" comes from).

---

## 4. Data Model (built sync-ready, kept simple)

### On disk
```
~/Silo/                       # the vault (user-chosen folder)
├── inbox/quick-capture.md
├── research/
│   ├── zettelkasten-vs-folders.md
│   └── note-linking-conventions.md
└── .silo/
    ├── index.sqlite          # rebuildable — safe to delete
    └── settings.json         # theme, layout, last-open note, vault path
```

### A note file (YAML frontmatter — chosen for Obsidian/ecosystem compatibility)
```markdown
---
id: 01J9X4M2Q7ZK8       # ULID — stable, device-independent, time-sortable
created: 2026-07-28T10:12:00Z
updated: 2026-07-28T11:40:00Z
tags: [method, research]
pinned: false
---
# Zettelkasten vs. folders

Folders force one hierarchy. Links let a note live in many contexts —
see [[Note-linking conventions]].
```

### The three sync-ready rules (cheap now, essential later)
1. **Stable ULID on every note**, stored in frontmatter — identity survives renames/moves and never collides across devices. Wiki-links resolve by title at edit time but the index *stores the resolved id*.
2. **`updated` timestamp on every write** — gives last-writer-wins conflict resolution for free when sync arrives.
3. **No "one process owns the files" assumption** — the `notify` watcher means an external change (iCloud drop, git pull, another device later) reconciles into the UI instead of being clobbered.

### Core types (`silo-core`, illustrative)
```rust
struct Note { id: NoteId, path: PathBuf, title: String, frontmatter: Frontmatter, body: String }
struct Frontmatter { id: NoteId, created: DateTime, updated: DateTime, tags: Vec<Tag>, pinned: bool }
struct Link { from: NoteId, to_title: String, resolved: Option<NoteId> }
struct Notebook { path: PathBuf, name: String, children: Vec<Notebook> }  // = folder tree
```

### Index schema (SQLite — derived, disposable)
```sql
CREATE TABLE notes(id TEXT PRIMARY KEY, path TEXT, title TEXT, updated INTEGER, pinned INTEGER);
CREATE VIRTUAL TABLE notes_fts USING fts5(title, body);   -- full-text
CREATE TABLE links(from_id TEXT, to_title TEXT, to_id TEXT);   -- to_id NULL = unresolved
CREATE TABLE tags(note_id TEXT, tag TEXT);
```

Notebooks are just folders — no table. Phase 2+ note "kinds" (task/journal/etc.) add frontmatter fields the structure absorbs without schema change.

---

## 5. Editor Strategy (Approach 1 now → Approach 3 later)

**v0.1 (Approach 1):** a **single editable text buffer** over the note's markdown, with lightweight inline styling:
- headings rendered larger/bolder inline,
- `[[wiki-links]]` and `#tags` highlighted and clickable,
- checkboxes toggleable.

Crucially, `silo-markdown` **already parses the buffer into a block tree** (heading, paragraph, list, code, task) — the editor simply doesn't render each block as a separate widget yet.

**Eventual destination (Approach 3):** a block-based editor where each block is its own entity (matching the prototype's `mdDraft`/`activeBlock` model). Because the parser already produces blocks, this evolution changes **only the UI layer** — not the data model, not the parser. Additive, not a rewrite.

**Rejected for now:** adapting Zed's editor component (rope/multibuffer). Powerful but heavy coupling to Zed internals, and making someone else's editor embody the Modernist wedge is harder than it's worth for notes.

---

## 6. Design System — "Modernist" (first-class subsystem)

Because taste is the wedge, the design system is a subsystem, not styling-as-you-go. A single `Theme` struct in `silo-ui` with **light + dark** variants. **No component ever hardcodes a hex value.** Tokens extracted from the prototype:

### Signature traits
- **Square corners everywhere — 0px radius.** The defining move; nothing rounds.
- Warm off-white "paper" background, warm near-black ink, one loud red-orange accent used *sparingly* (selection, active nav, emphasis — never a large fill).
- Hairline dividers, very soft low-opacity shadows. Flat and structural, not glossy.
- Heavy **Archivo 800** headings, tight tracking; small-caps letter-spaced labels.

### Tokens (light)
```
--color-bg        #f3f2f2   (paper)
--color-surface   #eae9e9
--color-text      #201e1d   (warm near-black ink)
--color-divider   #201e1d @ 40%

Neutral ramp (warm gray):
100 #f8f4f4 · 200 #eae7e7 · 300 #d7d3d3 · 400 #bab6b6 · 500 #9b9797
600 #7d7979 · 700 #605d5d · 800 #444141 · 900 #2d2b2b

Accent (red-orange, signature):
--color-accent    #ec3013
ramp 100 #fff2ef · 500 #ff563c · 600 #dd2b0f · 700 #ae1800 · 900 #4d170e
--color-accent-2  #e15b47   (softer coral)
```
Dark mode remaps the same semantic aliases (`--s-bg/-surface/-text/-acc/…`).

### Typography
- Family: **Archivo** (bundled with the app, not assumed installed), fallback `system-ui, sans-serif`. Mono: `ui-monospace, Menlo, monospace`.
- Heading weight **800**, tracking `-0.015em`. Labels letter-spaced `0.06–0.1em`.
- Size scale (px): 10 · 11 · 12 · 13 · 14 · 15 · 16 · 17 · 18 · 20 · 25 · 26 · 32 · 42.

### Spacing / shadow / motion
- Spacing: 4 · 8 · 12 · 16 · 24 · 32.
- Shadows: sm `0 1px 2px`, md `0 3px 10px`, lg `0 12px 32px` — tinted `#2d2b2b` @ 14–22%.
- Motion: `.18s ease` on color/border/fill; entrance = fade + small rise.

### Window chrome
Custom titlebar with macOS traffic-light dots (`#ff5f57 #febc2e #28c840`), matching the prototype's framed look. GPUI supports custom title bars.

---

## 7. Error Handling

Guiding rule: **never lose the user's writing, never fail silently.** For a notes app, this is a feature.

- **Saves:** debounced autosave; write-to-temp-then-atomic-rename so a crash mid-write can't corrupt a note. On write failure, surface it visibly and keep the in-memory copy — never drop edits.
- **External-change conflict:** if a note changed on disk while unsaved edits exist, don't clobber — detect via `updated` timestamp, keep both, flag the conflict. Rare in v0.1, essential once sync lands.
- **Index corruption:** any index error → log, delete `.silo/index.sqlite`, rebuild from the vault. The index is never load-bearing.
- **Malformed notes:** bad frontmatter or unparseable file loads as plain text with a warning — never crashes or hides the note.
- **Types:** `thiserror` in library crates, `anyhow` at the app boundary; `tracing` logs (stderr in dev, rolling file in release under the app-support dir).

---

## 8. Testing

Leans directly on the crate boundaries in §3.

- **`silo-core`, `silo-markdown`, `silo-index`, `silo-vault` — unit-tested without GPUI.** That boundary is *why* they're separate crates.
  - `insta` snapshot tests for markdown→block tree and `[[link]]`/`#tag` extraction.
  - Round-trip tests for frontmatter: parse → write → parse is identical.
  - Temp-dir tests for vault read/write and index build/query.
- **`silo-ui` (GPUI):** minimal logic; tested by hand for v0.1 — the smoothness/design wedge is judged by eye. We do not over-invest in UI test infra early.
- **Per-milestone `verify`:** launch the app and drive the actual flow, not just green tests.

---

## 9. Tech Stack

| Layer | Choice | Notes |
|---|---|---|
| Language | Rust (edition 2021) | Workspace of crates per §3. |
| UI framework | **GPUI** | Git dependency pinned to a specific Zed revision (no crates.io release). |
| Rendering | Metal via GPUI | macOS-first. |
| Async | GPUI executor + `smol` | Keep IO/indexing off the render thread. |
| Content store | Markdown `.md` files | Source of truth. |
| Index | `rusqlite` (bundled SQLite) + **FTS5** | Rebuildable; `<vault>/.silo/index.sqlite`. |
| Markdown | `pulldown-cmark` + custom `[[wiki-link]]` pass | Parse to block tree; extract links/tags. |
| File watching | `notify` | External-edit reconciliation. |
| Frontmatter | `serde_yaml` | YAML block at top of each note. |
| Settings | `serde` + `serde_json` | `.silo/settings.json`. |
| IDs | `ulid` | Stable, time-sortable, device-independent. |
| Errors | `anyhow` (app), `thiserror` (libs) | |
| Logging | `tracing` + `tracing-subscriber` | |
| Paths | `directories` | Standard macOS dirs. |

### GPUI reality check
GPUI is young, thinly documented, and developed against Zed's own needs (pinned to a commit, not semver-stable). Expect to read Zed's source as documentation and to pin/bump a specific revision deliberately. This is the cost of Zed-caliber smoothness; budget learning time in the first milestone.

---

## 10. Project Utilities & Tooling

| Concern | Setup |
|---|---|
| Build / run | `cargo build`; `cargo run -p silo`. |
| Format | `rustfmt` (`rustfmt.toml` checked in). |
| Lint | `clippy` with `-D warnings` in CI. |
| Test | `cargo test` per crate; `insta` snapshots. |
| Pre-commit | `cargo fmt --check && cargo clippy && cargo test` (git hook). |
| CI | GitHub Actions, macOS runner: fmt, clippy, test, build. |
| GPUI pinning | Pin Zed git rev in `Cargo.toml`; document the rev; bump deliberately. |
| Fonts | Bundle Archivo in app resources; register at startup. |
| Packaging | `cargo-bundle` early → codesign + notarize + `.dmg` later (needed for a product shipped to others). |
| Fast iteration | `cargo-watch -x 'run -p silo'`. |
| Toolchain | `rust-toolchain.toml` pinned. |
| Repo hygiene | `.gitignore` (`/target`, `.DS_Store`, `*.sqlite`, test vaults), `CLAUDE.md` (build/run/test + architecture), `LICENSE`, `CHANGELOG.md`. |

---

## 11. Milestones

- **M0 — Skeleton.** Cargo workspace + crates; GPUI window opens rendering "Silo" with the Modernist theme + traffic-light titlebar. Proves the GPUI dependency and theme pipeline.
- **M1 — Read a vault.** Walk a folder → notebook tree + note list; open a note read-only with markdown rendering.
- **M2 — Edit & save.** Editable buffer, debounced atomic autosave, frontmatter round-trip, external-edit reload.
- **M3 — Index & search.** SQLite/FTS5 index, live search, command palette (⌘K).
- **M4 — Links.** `[[wiki-link]]` autocomplete + navigation + backlinks panel. → **v0.1 MVP complete.**
- **Polish gate.** Every screen intentional in light + dark before calling v0.1 done.
- **M5+.** Phase 2 (tasks, tags, history), then Phase 3 (structured types), then Phase 4 (sync + mobile).

---

## 12. Open Questions / Risks

1. **GPUI editor primitives (highest risk).** How much text-editing scaffolding does GPUI give before we'd need to borrow from Zed's editor? Spike during M2.
2. **Font licensing.** Confirm Archivo's license permits bundling/redistribution in a shipped product (it's Google Fonts / OFL — verify before release).
3. **History backing (Phase 2).** Snapshot files vs. embedded git — decide before the M2 save format hardens, since it affects on-disk layout.
4. **Sync mechanism (Phase 4).** iCloud/CloudKit vs. plain synced folder vs. dedicated service — deferred, but the §4 rules keep all options open.
5. **Mobile reuse boundary.** Validate that `silo-core`/`silo-markdown` stay genuinely GPUI-free so they *could* compile for an iOS target later.
