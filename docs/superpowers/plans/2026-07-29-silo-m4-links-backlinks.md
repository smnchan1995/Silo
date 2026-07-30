# Silo M4: Links & Backlinks — Implementation Plan → completes v0.1

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make notes a connected graph: resolve `[[wiki-links]]` to notes, navigate them (creating the target if missing), and show a "Linked mentions" backlinks panel — completing the Zettelkasten spine and thus v0.1.

**Architecture:** M3 already populates the `links` table (`from_id, to_title, to_id NULL`). This milestone adds resolution (fill `to_id` by title) and a `backlinks` query in `silo-index`, then a links panel in the content pane: outgoing `[[links]]` as clickable chips (unresolved → create-on-follow) and incoming "Linked mentions". Resolution runs after every build/upsert so backlinks stay live.

**Tech Stack:** Rust, `rusqlite` (existing `silo-index`), GPUI, existing crates.

## Global Constraints

- `silo-core`/`silo-markdown` stay GPUI-free and IO-free.
- Link resolution is by **case-insensitive title match**; ambiguity (duplicate titles) resolves to the first match — deterministic, acceptable for v0.1.
- Resolution and backlinks are derived from the disposable index; the vault stays source of truth.
- GPUI code is representative; reconcile against the pinned rev. `silo-index` logic is unit-tested; the panel is verified by running.

## Out of scope (fast-follow)
The `[[` inline autocomplete popover inside the editor (needs deep text-element integration); graph view; link aliases (`[[Title|alias]]`); backlink context snippets (we list the source note; snippet can come later).

---

### Task 1: link resolution + backlinks in `silo-index` (TDD)

**Files:**
- Modify: `crates/silo-index/src/lib.rs`

**Interfaces:**
- Produces:
  - `struct Backlink { from_id: NoteId, from_title: String }`
  - `Index::resolve_links(&self) -> Result<(), IndexError>` — fills `links.to_id` by case-insensitive title match; called at the end of `open_or_build` and exposed for callers to re-run after upserts.
  - `Index::backlinks(&self, id: NoteId) -> Result<Vec<Backlink>, IndexError>` — notes whose links resolve to `id`.
  - `Index::resolve_title(&self, title: &str) -> Result<Option<NoteId>, IndexError>` — id of a note with that title (case-insensitive), if any.

- [ ] **Step 1: Write failing tests**

Add to `crates/silo-index/src/lib.rs` tests:
```rust
#[test]
fn resolves_links_and_reports_backlinks() {
    let dir = tempdir().unwrap();
    let vault = vault_with(dir.path(), &[
        ("a.md", "# Alpha\nsee [[Beta]] for more"),
        ("b.md", "# Beta\nthe target"),
    ]);
    let idx = Index::open_or_build(dir.path(), &vault).unwrap();
    // Beta's id
    let beta = idx.resolve_title("beta").unwrap().unwrap();
    let back = idx.backlinks(beta).unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].from_title, "Alpha");
}

#[test]
fn unresolved_link_has_no_backlink() {
    let dir = tempdir().unwrap();
    let vault = vault_with(dir.path(), &[("a.md", "# Alpha\nsee [[Ghost]]")]);
    let idx = Index::open_or_build(dir.path(), &vault).unwrap();
    assert!(idx.resolve_title("ghost").unwrap().is_none());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p silo-index resolves_links`
Expected: FAIL — `resolve_title`/`backlinks` not defined.

- [ ] **Step 3: Implement**

Add to `crates/silo-index/src/lib.rs`:
```rust
#[derive(Debug, Clone)]
pub struct Backlink {
    pub from_id: NoteId,
    pub from_title: String,
}

impl Index {
    /// Fill `links.to_id` by matching `to_title` to a note title (case-insensitive).
    pub fn resolve_links(&self) -> Result<(), IndexError> {
        self.conn.execute(
            "UPDATE links SET to_id = (
                 SELECT id FROM notes_fts WHERE lower(title) = lower(links.to_title) LIMIT 1
             )",
            [],
        )?;
        Ok(())
    }

    pub fn resolve_title(&self, title: &str) -> Result<Option<NoteId>, IndexError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM notes_fts WHERE lower(title) = lower(?) LIMIT 1")?;
        let mut rows = stmt.query(params![title])?;
        match rows.next()? {
            Some(r) => Ok(Some(parse_id(&r.get::<_, String>(0)?))),
            None => Ok(None),
        }
    }

    pub fn backlinks(&self, id: NoteId) -> Result<Vec<Backlink>, IndexError> {
        let id = id.to_string();
        let mut stmt = self.conn.prepare(
            "SELECT l.from_id, f.title
               FROM links l JOIN notes_fts f ON f.id = l.from_id
              WHERE l.to_id = ?
              GROUP BY l.from_id
              ORDER BY f.title",
        )?;
        let rows = stmt.query_map(params![id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        rows.map(|res| {
            let (fid, title) = res?;
            Ok(Backlink { from_id: parse_id(&fid), from_title: title })
        })
        .collect()
    }
}
```
In `open_or_build`, after `idx.rebuild(vault)?;` add `idx.resolve_links()?;`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p silo-index`
Expected: PASS (existing 3 + 2 new).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(index): link resolution + backlinks query"
```

---

### Task 2: links panel + navigation in the content pane

Below the editor: outgoing `[[links]]` as clickable chips (unresolved chips create-on-follow) and a "Linked mentions" list. Re-resolve after saves/creates so both stay current.

**Files:**
- Modify: `crates/silo-ui/src/app_state.rs` (helpers + follow/open + re-resolve on save)
- Modify: `crates/silo-ui/src/lib.rs` (`content_pane` gets a links panel; needs `cx`)

**Interfaces:**
- Consumes: `silo_index::{Backlink}`, `silo_markdown::extract_links`, `silo_vault::create_note`.
- Produces on `AppState`:
  - `pub fn outgoing_links(&self) -> Vec<(String, Option<NoteId>)>` — `[[titles]]` of the selected note, each resolved to an id if it exists.
  - `pub fn backlinks_of_selected(&self) -> Vec<silo_index::Backlink>`
  - `pub fn follow_link(&mut self, title: String, window, cx)` — open the target, or create it (titled `title`), reindex, resolve, and open.

- [ ] **Step 1: AppState helpers**

```rust
pub fn outgoing_links(&self) -> Vec<(String, Option<NoteId>)> {
    let Some(note) = self.selected_note() else { return vec![] };
    let idx = self.index.as_ref();
    silo_markdown::extract_links(&note.body)
        .into_iter()
        .map(|title| {
            let id = idx.and_then(|i| i.resolve_title(&title).ok().flatten());
            (title, id)
        })
        .collect()
}

pub fn backlinks_of_selected(&self) -> Vec<silo_index::Backlink> {
    match (self.selected, self.index.as_ref()) {
        (Some(id), Some(idx)) => idx.backlinks(id).unwrap_or_default(),
        _ => vec![],
    }
}

pub fn follow_link(&mut self, title: String, window: &mut Window, cx: &mut Context<Self>) {
    let existing = self.index.as_ref().and_then(|i| i.resolve_title(&title).ok().flatten());
    match existing {
        Some(id) => self.open_note(id, window, cx),
        None => {
            let dir = self.vault.path.clone();
            if let Ok(note) = silo_vault::create_note(&dir, &title) {
                let id = note.id;
                if let Ok(v) = silo_vault::walk_vault(&dir) { self.vault = v; }
                if let Some(idx) = &self.index {
                    let _ = idx.upsert_note(&note);
                    let _ = idx.resolve_links();
                }
                self.open_note(id, window, cx);
            }
        }
    }
}
```

- [ ] **Step 2: Re-resolve after saves**

In `save_now`, after `idx.upsert_note(&updated)`, also `let _ = idx.resolve_links();` (so a newly-typed `[[link]]` immediately resolves and backlinks update).

- [ ] **Step 3: Links panel in `content_pane`**

Change `content_pane(t, st)` → `content_pane(t, st, cx)`. Keep breadcrumb + editor (flex_1), then add a bottom panel (only when a note is selected):
```rust
// outgoing chips
let mut links_row = div().flex().flex_wrap().gap(px(6.0));
for (title, id) in st.outgoing_links() {
    let t2 = title.clone();
    links_row = links_row.child(
        div().px(px(8.0)).py(px(3.0)).border_1().border_color(t.divider)
            .cursor(CursorStyle::PointingHand)
            .text_xs()
            .text_color(if id.is_some() { t.accent } else { t.faint })
            .child(format!("[[{title}]]"))
            .on_mouse_down(MouseButton::Left, cx.listener(move |st, _e, w, cx| st.follow_link(t2.clone(), w, cx))),
    );
}
// backlinks
let mut mentions = div().flex().flex_col().gap(px(2.0));
for b in st.backlinks_of_selected() {
    let id = b.from_id;
    mentions = mentions.child(
        div().text_sm().text_color(t.text).cursor(CursorStyle::PointingHand)
            .child(b.from_title.clone())
            .on_mouse_down(MouseButton::Left, cx.listener(move |st, _e, w, cx| st.open_note(id, w, cx))),
    );
}
let panel = div().flex().flex_col().gap(px(6.0)).pt(px(14.0)).pb(px(18.0))
    .border_t_1().border_color(t.divider)
    .child(label(t, "Links")).child(links_row)
    .child(div().pt(px(8.0)).child(label(t, "Linked mentions")))
    .child(mentions);
```
Append `panel` to the content pane when a note is selected. Update the `render` call site to `content_pane(&t, self, cx)`.

- [ ] **Step 4: Verify by running**

Run `cargo run -p silo`. Open a note with `[[Other]]` in its body:
- The **Links** row shows `[[Other]]` (accent if it exists, faint if not).
- Click a resolved chip → opens that note. Click a faint (missing) chip → **creates** the note and opens it.
- Open the linked-to note → **Linked mentions** lists the source note; click it to jump back.
- Type a new `[[Name]]` in a note, wait for autosave → it appears resolved/faint and backlinks update.

- [ ] **Step 5: Full gate + commit**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
```bash
git add -A && git commit -m "feat(ui): links panel + backlinks navigation (create-on-follow)"
```

---

## Self-Review

**Spec coverage (M4 sub-spec):** link resolution (`to_id` by title) → Task 1 `resolve_links`; navigation + create-on-follow → Task 2 `follow_link`; backlinks "Linked mentions" → Task 1 `backlinks` + Task 2 panel; live updates → Task 2 (resolve after save). **Deferred (flagged):** `[[` inline autocomplete popover, alias links, backlink context snippets, graph view.

**Placeholder scan:** Task 1 is concrete SQL + tests. Task 2 is GPUI with exact APIs (`extract_links`, `resolve_title`, `backlinks`, `create_note`, `on_mouse_down`/`cx.listener`) reused from prior milestones — not invented.

**Type consistency:** `Backlink`, `resolve_links`, `resolve_title`, `backlinks` are consistent across Tasks 1–2 and build on `NoteId`/`Index`/`extract_links`/`create_note`/`open_note`. `content_pane` gains a `cx` param with the matching `render` call-site update.

**Known risks:** (1) duplicate titles resolve to the first match — deterministic but could surprise; acceptable for v0.1. (2) Outgoing-link resolution runs a query per link on each render — fine for typical note link counts; batch if it ever lags.
