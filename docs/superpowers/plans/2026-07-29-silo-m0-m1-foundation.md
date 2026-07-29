# Silo M0–M1: Foundation (Skeleton + Read a Vault) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the first working slice of Silo: launch a native GPUI window in the Modernist look, point it at a folder, browse the notebook (folder) tree, and read a note rendered from Markdown.

**Architecture:** A Cargo workspace splits pure-Rust domain crates (`silo-core`, `silo-markdown`, `silo-vault`) — which know nothing about the UI and are fully unit-tested — from `silo-ui` (GPUI) and a thin `app/silo` binary. This plan builds M0 (skeleton + theme) and M1 (read a vault, read-only). Editing, indexing/search, and links are later plans.

**Tech Stack:** Rust (edition 2021), GPUI (git-pinned to a Zed revision), `pulldown-cmark`, `serde` + `serde_yaml`, `ulid`, `thiserror`/`anyhow`, `tempfile` (dev), `insta` (dev).

## Global Constraints

- **Rust edition 2021.** Workspace with crates `silo-core`, `silo-vault`, `silo-index`, `silo-markdown`, `silo-ui` and binary `app/silo`. (This plan touches all except `silo-index`, which is created empty here and filled in the M3 plan.)
- **`silo-core` and `silo-markdown` MUST NOT depend on GPUI, any GUI crate, or any filesystem IO.** They are the mobile-reuse boundary. `silo-vault` may do IO; only `silo-ui`/`app` may depend on GPUI.
- **No component hardcodes a hex color.** All colors come from the `Theme` struct.
- **GPUI is pinned to a specific Zed git revision** in `Cargo.toml`; record the exact rev in `CLAUDE.md`. GPUI's API is not semver-stable — GPUI code below is representative of the common pattern and MUST be reconciled against the examples in the pinned rev's `crates/gpui/examples/`.
- **Note identity is a ULID** stored in YAML frontmatter under key `id`. Every note write stamps `updated` (RFC3339 UTC). (Writes are in the M2 plan; this plan only reads.)
- **Square corners everywhere: all radii 0px.** Nothing rounds.
- **Errors:** `thiserror` in library crates, `anyhow` at the `app` boundary. Never panic on a malformed note — load it as plain text.

---

### Task 1: Cargo workspace + opening a GPUI window

Establishes the workspace and proves the GPUI dependency links and a window opens. There is no unit test for "a window appears" — the deliverable is verified by running. A trivial `silo-core` version test establishes the test harness so later tasks have a pattern to follow.

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `crates/silo-core/Cargo.toml`
- Create: `crates/silo-core/src/lib.rs`
- Create: `crates/silo-vault/Cargo.toml`, `crates/silo-vault/src/lib.rs` (stub)
- Create: `crates/silo-index/Cargo.toml`, `crates/silo-index/src/lib.rs` (stub — filled in M3 plan)
- Create: `crates/silo-markdown/Cargo.toml`, `crates/silo-markdown/src/lib.rs` (stub)
- Create: `crates/silo-ui/Cargo.toml`, `crates/silo-ui/src/lib.rs`
- Create: `app/silo/Cargo.toml`, `app/silo/src/main.rs`
- Create: `CLAUDE.md`

**Interfaces:**
- Consumes: nothing (first task).
- Produces:
  - `silo_core::VERSION: &str`
  - `silo_ui::run() -> anyhow::Result<()>` — opens the main window and runs the app event loop.

- [ ] **Step 1: Create the workspace root `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/silo-core",
    "crates/silo-vault",
    "crates/silo-index",
    "crates/silo-markdown",
    "crates/silo-ui",
    "app/silo",
]

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT"

[workspace.dependencies]
anyhow = "1"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
pulldown-cmark = "0.12"
ulid = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
tempfile = "3"
insta = "1"
# GPUI has no crates.io release. Pin to a specific Zed revision.
# Replace <REV> with a chosen commit SHA and record it in CLAUDE.md.
gpui = { git = "https://github.com/zed-industries/zed", rev = "<REV>" }
```

- [ ] **Step 2: Create `rust-toolchain.toml` and `.gitignore`**

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

`.gitignore`:
```gitignore
/target
**/*.rs.bk
.DS_Store
*.sqlite
/scratch-vault
```

- [ ] **Step 3: Create `silo-core` with a version constant and its test**

`crates/silo-core/Cargo.toml`:
```toml
[package]
name = "silo-core"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
ulid = { workspace = true }
thiserror = { workspace = true }
```

`crates/silo-core/src/lib.rs`:
```rust
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_not_empty() {
        assert!(!super::VERSION.is_empty());
    }
}
```

- [ ] **Step 4: Create the stub library crates**

`crates/silo-vault/Cargo.toml`:
```toml
[package]
name = "silo-vault"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
silo-core = { path = "../silo-core" }
silo-markdown = { path = "../silo-markdown" }
serde_yaml = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```
`crates/silo-vault/src/lib.rs`: `// filled in Task 5`

`crates/silo-markdown/Cargo.toml`:
```toml
[package]
name = "silo-markdown"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
pulldown-cmark = { workspace = true }

[dev-dependencies]
insta = { workspace = true }
```
`crates/silo-markdown/src/lib.rs`: `// filled in Task 4`

`crates/silo-index/Cargo.toml`:
```toml
[package]
name = "silo-index"
edition.workspace = true
version.workspace = true
license.workspace = true
```
`crates/silo-index/src/lib.rs`: `// filled in the M3 plan`

- [ ] **Step 5: Create `silo-ui` with a minimal window**

`crates/silo-ui/Cargo.toml`:
```toml
[package]
name = "silo-ui"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
silo-core = { path = "../silo-core" }
silo-vault = { path = "../silo-vault" }
gpui = { workspace = true }
anyhow = { workspace = true }
```

`crates/silo-ui/src/lib.rs` — **representative GPUI; reconcile against the pinned rev's `crates/gpui/examples/hello_world.rs`:**
```rust
use gpui::*;

mod theme; // added in Task 2

struct Silo;

impl Render for Silo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .justify_center()
            .items_center()
            .bg(rgb(0xf3f2f2))
            .child(
                div()
                    .text_color(rgb(0x201e1d))
                    .child("Silo"),
            )
    }
}

pub fn run() -> anyhow::Result<()> {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|_cx| Silo)
        })
        .expect("failed to open window");
    });
    Ok(())
}
```

- [ ] **Step 6: Create the `app/silo` binary**

`app/silo/Cargo.toml`:
```toml
[package]
name = "silo"
edition.workspace = true
version.workspace = true
license.workspace = true

[[bin]]
name = "silo"
path = "src/main.rs"

[dependencies]
silo-ui = { path = "../../crates/silo-ui" }
anyhow = { workspace = true }
tracing-subscriber = { workspace = true }
```

`app/silo/src/main.rs`:
```rust
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    silo_ui::run()
}
```

- [ ] **Step 7: Create `CLAUDE.md`**

```markdown
# Silo — build notes for agents

Native macOS notes app in Rust on GPUI. Design wedge: the "Modernist" look.
Spec: docs/superpowers/specs/2026-07-29-silo-notes-design.md

## Commands
- Build:  `cargo build`
- Run:    `cargo run -p silo`
- Test:   `cargo test`            (domain crates; silo-ui is verified by running)
- Format: `cargo fmt`
- Lint:   `cargo clippy --all-targets -- -D warnings`

## Architecture
- silo-core / silo-markdown: pure Rust, NO GPUI, NO IO — mobile-reuse boundary.
- silo-vault: filesystem read/write of .md notes.
- silo-index: SQLite/FTS5 (empty until the M3 plan).
- silo-ui: GPUI shell; app/silo: thin binary.

## GPUI
Pinned Zed revision: <REV>   <-- record the exact SHA used in Cargo.toml.
GPUI API is not semver-stable. When GPUI code won't compile, check the
examples in that rev's crates/gpui/examples/ and reconcile.
```

- [ ] **Step 8: Verify the workspace builds and the core test passes**

Run: `cargo build && cargo test -p silo-core`
Expected: build succeeds; `version_is_not_empty` passes. (If GPUI fails to build, reconcile `silo-ui/src/lib.rs` against the pinned rev's example before continuing.)

- [ ] **Step 9: Verify the window opens**

Run: `cargo run -p silo`
Expected: a native window opens showing "Silo" in warm-black text on a warm off-white background. Close it to exit.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat: cargo workspace + minimal GPUI window"
```

---

### Task 2: Modernist theme + custom titlebar

Introduces the `Theme` struct (the design wedge as a first-class subsystem) with light/dark variants, and makes the window use it. Colors are unit-tested for correctness; the visual result is verified by running.

**Files:**
- Create: `crates/silo-ui/src/theme.rs`
- Modify: `crates/silo-ui/src/lib.rs` (use the theme; add a titlebar)

**Interfaces:**
- Consumes: `silo_ui::run` from Task 1.
- Produces:
  - `theme::Theme` with fields `bg, surface, text, divider, accent: Rgba` (at least these).
  - `theme::Theme::light() -> Theme`
  - `theme::Theme::dark() -> Theme`
  - `theme::RADIUS: f32` (= `0.0`)

- [ ] **Step 1: Write the failing theme test**

`crates/silo-ui/src/theme.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_theme_matches_modernist_tokens() {
        let t = Theme::light();
        assert_eq!(t.bg, rgb(0xf3f2f2));
        assert_eq!(t.text, rgb(0x201e1d));
        assert_eq!(t.accent, rgb(0xec3013));
    }

    #[test]
    fn dark_differs_from_light_bg() {
        assert_ne!(Theme::dark().bg, Theme::light().bg);
    }

    #[test]
    fn radius_is_zero() {
        assert_eq!(RADIUS, 0.0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p silo-ui theme`
Expected: FAIL — `Theme`, `rgb`, `RADIUS` not defined.

- [ ] **Step 3: Implement the theme**

Prepend to `crates/silo-ui/src/theme.rs` (above the test module):
```rust
use gpui::{rgb, Rgba};

/// Modernist: every corner is square.
pub const RADIUS: f32 = 0.0;

#[derive(Clone, Debug)]
pub struct Theme {
    pub bg: Rgba,
    pub surface: Rgba,
    pub text: Rgba,
    pub divider: Rgba,
    pub accent: Rgba,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            bg: rgb(0xf3f2f2),
            surface: rgb(0xeae9e9),
            text: rgb(0x201e1d),
            divider: rgb(0x605d5d),
            accent: rgb(0xec3013),
        }
    }

    pub fn dark() -> Self {
        Self {
            bg: rgb(0x201e1d),
            surface: rgb(0x2d2b2b),
            text: rgb(0xf8f4f4),
            divider: rgb(0x605d5d),
            accent: rgb(0xff563c),
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p silo-ui theme`
Expected: PASS (3 tests).

- [ ] **Step 5: Make the window use the theme + a titlebar**

In `crates/silo-ui/src/lib.rs`, hold a `Theme` on the root view and use its colors instead of literal hex; add a top titlebar strip with three traffic-light dots. Reconcile against the pinned rev.
```rust
use gpui::*;
use theme::Theme;

mod theme;

struct Silo {
    theme: Theme,
}

impl Silo {
    fn dot(color: Rgba) -> impl IntoElement {
        div().w(px(12.0)).h(px(12.0)).bg(color) // square corners: no rounding
    }
}

impl Render for Silo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = &self.theme;
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.bg)
            .child(
                // titlebar
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .h(px(36.0))
                    .px(px(12.0))
                    .bg(t.surface)
                    .border_b_1()
                    .border_color(t.divider)
                    .child(Self::dot(rgb(0xff5f57)))
                    .child(Self::dot(rgb(0xfebc2e)))
                    .child(Self::dot(rgb(0x28c840))),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .justify_center()
                    .items_center()
                    .text_color(t.text)
                    .child("Silo"),
            )
    }
}

pub fn run() -> anyhow::Result<()> {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|_cx| Silo { theme: Theme::light() })
        })
        .expect("failed to open window");
    });
    Ok(())
}
```
(The traffic-light dot colors are chrome constants, not theme tokens — keeping them inline is acceptable.)

- [ ] **Step 6: Verify build, tests, and appearance**

Run: `cargo test -p silo-ui && cargo run -p silo`
Expected: tests pass; window shows a surface-colored titlebar with red/yellow/green dots over a paper background, "Silo" centered below.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: Modernist theme (light/dark) + traffic-light titlebar"
```

---

### Task 3: Domain types in `silo-core`

The pure data model: a device-independent `NoteId` (ULID), `Frontmatter`, `Note`, and the `Notebook` folder tree. No IO, no GPUI.

**Files:**
- Create: `crates/silo-core/src/note.rs`
- Create: `crates/silo-core/src/notebook.rs`
- Modify: `crates/silo-core/src/lib.rs` (module wiring + re-exports)

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `NoteId(pub Ulid)` with `NoteId::new() -> NoteId`, `Display`, `FromStr`.
  - `struct Frontmatter { id: NoteId, created: String, updated: String, tags: Vec<String>, pinned: bool }` (timestamps are RFC3339 strings in this plan).
  - `struct Note { id: NoteId, path: PathBuf, title: String, frontmatter: Frontmatter, body: String }`
  - `struct Notebook { name: String, path: PathBuf, children: Vec<Notebook>, notes: Vec<Note> }`

- [ ] **Step 1: Write failing tests for `NoteId`**

`crates/silo-core/src/note.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn note_id_roundtrips_through_string() {
        let id = NoteId::new();
        let s = id.to_string();
        assert_eq!(NoteId::from_str(&s).unwrap(), id);
    }

    #[test]
    fn two_new_ids_differ() {
        assert_ne!(NoteId::new(), NoteId::new());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p silo-core note_id`
Expected: FAIL — `NoteId` not defined.

- [ ] **Step 3: Implement `NoteId`, `Frontmatter`, `Note`**

Prepend to `crates/silo-core/src/note.rs`:
```rust
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use ulid::Ulid;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct NoteId(pub Ulid);

impl NoteId {
    pub fn new() -> Self {
        NoteId(Ulid::new())
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for NoteId {
    type Err = ulid::DecodeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(NoteId(Ulid::from_string(s)?))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frontmatter {
    pub id: NoteId,
    pub created: String, // RFC3339 UTC
    pub updated: String, // RFC3339 UTC
    pub tags: Vec<String>,
    pub pinned: bool,
}

#[derive(Clone, Debug)]
pub struct Note {
    pub id: NoteId,
    pub path: PathBuf,
    pub title: String,
    pub frontmatter: Frontmatter,
    pub body: String, // markdown, frontmatter stripped
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p silo-core note_id`
Expected: PASS.

- [ ] **Step 5: Add the `Notebook` tree type**

`crates/silo-core/src/notebook.rs`:
```rust
use std::path::PathBuf;
use crate::note::Note;

#[derive(Clone, Debug)]
pub struct Notebook {
    pub name: String,
    pub path: PathBuf,
    pub children: Vec<Notebook>,
    pub notes: Vec<Note>,
}

impl Notebook {
    /// Total notes in this notebook and all descendants.
    pub fn note_count(&self) -> usize {
        self.notes.len() + self.children.iter().map(Notebook::note_count).sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_count_sums_descendants() {
        let leaf = Notebook { name: "a".into(), path: "a".into(), children: vec![], notes: vec![] };
        let root = Notebook { name: "root".into(), path: ".".into(), children: vec![leaf], notes: vec![] };
        assert_eq!(root.note_count(), 0);
    }
}
```

- [ ] **Step 6: Wire modules in `lib.rs`**

Replace `crates/silo-core/src/lib.rs` contents above the existing test module with:
```rust
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod note;
pub mod notebook;

pub use note::{Frontmatter, Note, NoteId};
pub use notebook::Notebook;
```
(Keep the existing `version_is_not_empty` test module.)

- [ ] **Step 7: Run the full core test suite**

Run: `cargo test -p silo-core`
Expected: PASS (all: version, note_id x2, note_count).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(core): NoteId (ULID), Frontmatter, Note, Notebook types"
```

---

### Task 4: Markdown parsing in `silo-markdown`

Pure functions: derive a title, and extract `[[wiki-links]]` and `#tags` from note body text. (Frontmatter splitting lives here too since it's pure string work.) No IO.

**Files:**
- Create: `crates/silo-markdown/src/lib.rs` (replacing the stub)

**Interfaces:**
- Consumes: nothing (pure).
- Produces:
  - `split_frontmatter(raw: &str) -> (Option<&str>, &str)` — returns `(yaml_without_delimiters, body)`; `None` when no `---` block leads the file.
  - `derive_title(body: &str) -> String` — first `# ` heading, else first non-empty line, else `"Untitled"`.
  - `extract_links(body: &str) -> Vec<String>` — inner text of each `[[...]]`, in order, de-duplicated.
  - `extract_tags(body: &str) -> Vec<String>` — each `#tag` (word chars/`-`/`_`, not inside a heading `# `), de-duplicated, without the `#`.

- [ ] **Step 1: Write failing tests**

`crates/silo-markdown/src/lib.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_leading_frontmatter() {
        let raw = "---\nid: 01\n---\n# Hi\nbody";
        let (fm, body) = split_frontmatter(raw);
        assert_eq!(fm, Some("id: 01\n"));
        assert_eq!(body, "# Hi\nbody");
    }

    #[test]
    fn no_frontmatter_returns_none_and_full_body() {
        let raw = "# Hi\nbody";
        let (fm, body) = split_frontmatter(raw);
        assert_eq!(fm, None);
        assert_eq!(body, "# Hi\nbody");
    }

    #[test]
    fn title_prefers_first_h1() {
        assert_eq!(derive_title("intro\n# Real Title\nx"), "Real Title");
    }

    #[test]
    fn title_falls_back_to_first_nonempty_line() {
        assert_eq!(derive_title("\n\nplain first line\nmore"), "plain first line");
    }

    #[test]
    fn title_defaults_when_empty() {
        assert_eq!(derive_title("   \n\n"), "Untitled");
    }

    #[test]
    fn extracts_wiki_links_in_order_deduped() {
        let body = "see [[Alpha]] and [[Beta]] and [[Alpha]] again";
        assert_eq!(extract_links(body), vec!["Alpha", "Beta"]);
    }

    #[test]
    fn extracts_tags_without_hash_deduped() {
        let body = "tagged #method and #research and #method";
        assert_eq!(extract_tags(body), vec!["method", "research"]);
    }

    #[test]
    fn heading_hash_is_not_a_tag() {
        assert_eq!(extract_tags("# Heading\ntext"), Vec::<String>::new());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p silo-markdown`
Expected: FAIL — functions not defined.

- [ ] **Step 3: Implement the parsing functions**

Prepend to `crates/silo-markdown/src/lib.rs`:
```rust
/// Split a leading `---\n ... \n---\n` YAML block from the body.
/// Returns (Some(yaml_without_delimiters), body) or (None, whole_input).
pub fn split_frontmatter(raw: &str) -> (Option<&str>, &str) {
    let Some(rest) = raw.strip_prefix("---\n") else {
        return (None, raw);
    };
    // Find the closing delimiter line.
    if let Some(end) = rest.find("\n---\n") {
        let yaml = &rest[..end + 1]; // keep trailing newline of last yaml line
        let body = &rest[end + "\n---\n".len()..];
        (Some(yaml), body)
    } else if let Some(end) = rest.strip_suffix("\n---") {
        (Some(&rest[..end.len() + 1]), "")
    } else {
        (None, raw)
    }
}

pub fn derive_title(body: &str) -> String {
    for line in body.lines() {
        if let Some(h) = line.strip_prefix("# ") {
            let t = h.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    for line in body.lines() {
        let t = line.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    "Untitled".to_string()
}

pub fn extract_links(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = body.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(close) = body[i + 2..].find("]]") {
                let inner = body[i + 2..i + 2 + close].trim().to_string();
                if !inner.is_empty() && !out.contains(&inner) {
                    out.push(inner);
                }
                i += 2 + close + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

pub fn extract_tags(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        // Skip ATX headings so "# Heading" is not a tag.
        if line.trim_start().starts_with("# ") {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '#' {
                let start = i + 1;
                let mut j = start;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '-' || chars[j] == '_') {
                    j += 1;
                }
                if j > start {
                    let tag: String = chars[start..j].iter().collect();
                    if !out.contains(&tag) {
                        out.push(tag);
                    }
                }
                i = j;
            } else {
                i += 1;
            }
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p silo-markdown`
Expected: PASS (8 tests).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(markdown): frontmatter split, title, [[link]] and #tag extraction"
```

---

### Task 5: Read a note and walk a vault in `silo-vault`

Filesystem layer: parse one `.md` file into a `Note` (YAML frontmatter via `serde_yaml`, synthesizing missing fields), and walk a folder into a `Notebook` tree. Malformed frontmatter must NOT panic — the note loads as plain text with a fresh id.

**Files:**
- Create: `crates/silo-vault/src/lib.rs` (replacing the stub)

**Interfaces:**
- Consumes: `silo_core::{Note, NoteId, Frontmatter, Notebook}`; `silo_markdown::{split_frontmatter, derive_title}`.
- Produces:
  - `#[derive(thiserror::Error)] enum VaultError { Io(...), }`
  - `read_note(path: &Path) -> Result<Note, VaultError>`
  - `walk_vault(root: &Path) -> Result<Notebook, VaultError>` — recurses subfolders; `.md` files become notes; the `.silo` directory and dotfiles are skipped.

- [ ] **Step 1: Write failing tests using temp dirs**

`crates/silo-vault/src/lib.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn reads_note_with_frontmatter() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("n.md");
        fs::write(&p, "---\nid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\ncreated: 2026-01-01T00:00:00Z\nupdated: 2026-01-02T00:00:00Z\ntags: [x, y]\npinned: true\n---\n# Hello\nbody text").unwrap();
        let note = read_note(&p).unwrap();
        assert_eq!(note.title, "Hello");
        assert_eq!(note.frontmatter.tags, vec!["x", "y"]);
        assert!(note.frontmatter.pinned);
        assert_eq!(note.id.to_string(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert!(note.body.starts_with("# Hello"));
    }

    #[test]
    fn malformed_frontmatter_loads_as_plain_text() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("bad.md");
        fs::write(&p, "---\n: : not yaml : :\n---\n# Still Readable").unwrap();
        let note = read_note(&p).unwrap(); // must not error
        assert_eq!(note.title, "Still Readable");
    }

    #[test]
    fn note_without_frontmatter_gets_fresh_id_and_title() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("plain.md");
        fs::write(&p, "# Just A Note\ncontent").unwrap();
        let note = read_note(&p).unwrap();
        assert_eq!(note.title, "Just A Note");
        assert!(!note.frontmatter.created.is_empty());
    }

    #[test]
    fn walk_vault_builds_tree_and_skips_dot_silo() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("research")).unwrap();
        fs::create_dir(dir.path().join(".silo")).unwrap();
        fs::write(dir.path().join("inbox.md"), "# Inbox\n").unwrap();
        fs::write(dir.path().join("research/z.md"), "# Zettel\n").unwrap();
        fs::write(dir.path().join(".silo/index.sqlite"), "x").unwrap();
        let nb = walk_vault(dir.path()).unwrap();
        assert_eq!(nb.note_count(), 2); // inbox + research/z, NOT .silo
        assert!(nb.children.iter().any(|c| c.name == "research"));
        assert!(nb.children.iter().all(|c| c.name != ".silo"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p silo-vault`
Expected: FAIL — `read_note`/`walk_vault` not defined.

- [ ] **Step 3: Implement `read_note` and `walk_vault`**

Prepend to `crates/silo-vault/src/lib.rs`:
```rust
use std::fs;
use std::path::{Path, PathBuf};
use serde::Deserialize;
use silo_core::{Frontmatter, Note, NoteId, Notebook};
use silo_markdown::{derive_title, split_frontmatter};

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("io error reading {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
}

// Raw YAML shape; every field optional so malformed/partial frontmatter degrades gracefully.
#[derive(Deserialize, Default)]
struct RawFm {
    id: Option<String>,
    created: Option<String>,
    updated: Option<String>,
    tags: Option<Vec<String>>,
    pinned: Option<bool>,
}

fn now_rfc3339() -> String {
    // Avoid a chrono dependency here; SystemTime formatted as a stable RFC3339-ish UTC.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    // Minimal: seconds since epoch is monotonic enough for ordering; M2 replaces with chrono.
    format!("1970-01-01T00:00:{secs}Z")
}

pub fn read_note(path: &Path) -> Result<Note, VaultError> {
    let raw = fs::read_to_string(path).map_err(|source| VaultError::Io { path: path.to_path_buf(), source })?;
    let (yaml, body) = split_frontmatter(&raw);

    // Parse frontmatter if present; on any parse failure, fall back to defaults (never panic).
    let parsed: RawFm = yaml
        .and_then(|y| serde_yaml::from_str::<RawFm>(y).ok())
        .unwrap_or_default();

    let id = parsed
        .id
        .as_deref()
        .and_then(|s| s.parse::<NoteId>().ok())
        .unwrap_or_else(NoteId::new);

    let frontmatter = Frontmatter {
        id,
        created: parsed.created.unwrap_or_else(now_rfc3339),
        updated: parsed.updated.unwrap_or_else(now_rfc3339),
        tags: parsed.tags.unwrap_or_default(),
        pinned: parsed.pinned.unwrap_or(false),
    };

    Ok(Note {
        id,
        path: path.to_path_buf(),
        title: derive_title(body),
        frontmatter,
        body: body.to_string(),
    })
}

pub fn walk_vault(root: &Path) -> Result<Notebook, VaultError> {
    let name = root.file_name().and_then(|s| s.to_str()).unwrap_or("Vault").to_string();
    let mut children = Vec::new();
    let mut notes = Vec::new();

    let entries = fs::read_dir(root).map_err(|source| VaultError::Io { path: root.to_path_buf(), source })?;
    for entry in entries.flatten() {
        let path = entry.path();
        let fname = entry.file_name();
        let fname = fname.to_string_lossy();
        if fname.starts_with('.') {
            continue; // skip .silo and all dotfiles
        }
        if path.is_dir() {
            children.push(walk_vault(&path)?);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            notes.push(read_note(&path)?);
        }
    }
    // Stable order: notebooks then notes, each alphabetical.
    children.sort_by(|a, b| a.name.cmp(&b.name));
    notes.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

    Ok(Notebook { name, path: root.to_path_buf(), children, notes })
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p silo-vault`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(vault): read_note (graceful frontmatter) + walk_vault tree"
```

---

### Task 6: Wire the vault into the UI — browse and read (read-only)

The M1 payoff: the window shows a three-pane shell (sidebar notebook tree · note list · read-only note view) populated from a real folder. For this task the vault path is taken from the first CLI argument (a folder picker is deferred to M2). Verified by running against a scratch vault — GPUI view logic isn't unit-tested.

**Files:**
- Create: `crates/silo-ui/src/app_state.rs`
- Modify: `crates/silo-ui/src/lib.rs` (build the three panes; accept a vault path)
- Modify: `app/silo/src/main.rs` (pass the CLI arg through)

**Interfaces:**
- Consumes: `silo_vault::walk_vault`; `silo_core::{Notebook, Note, NoteId}`; `theme::Theme`.
- Produces:
  - `app_state::AppState { vault: Notebook, selected: Option<NoteId>, theme: Theme }`
  - `app_state::AppState::flat_notes(&self) -> Vec<&Note>` — all notes across the tree, depth-first.
  - `app_state::AppState::selected_note(&self) -> Option<&Note>`
  - `silo_ui::run(vault_path: std::path::PathBuf) -> anyhow::Result<()>` (signature change from Task 1).

- [ ] **Step 1: Write failing tests for `AppState` selection logic**

`crates/silo-ui/src/app_state.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use silo_core::{Frontmatter, Note, NoteId, Notebook};
    use std::path::PathBuf;

    fn note(title: &str) -> Note {
        let id = NoteId::new();
        Note {
            id,
            path: PathBuf::from(format!("{title}.md")),
            title: title.into(),
            frontmatter: Frontmatter { id, created: "".into(), updated: "".into(), tags: vec![], pinned: false },
            body: format!("# {title}"),
        }
    }

    #[test]
    fn flat_notes_collects_across_children() {
        let child = Notebook { name: "c".into(), path: ".".into(), children: vec![], notes: vec![note("B")] };
        let root = Notebook { name: "root".into(), path: ".".into(), children: vec![child], notes: vec![note("A")] };
        let st = AppState { vault: root, selected: None, theme: crate::theme::Theme::light() };
        let titles: Vec<_> = st.flat_notes().iter().map(|n| n.title.clone()).collect();
        assert!(titles.contains(&"A".to_string()) && titles.contains(&"B".to_string()));
    }

    #[test]
    fn selected_note_resolves_by_id() {
        let n = note("A");
        let id = n.id;
        let root = Notebook { name: "root".into(), path: ".".into(), children: vec![], notes: vec![n] };
        let st = AppState { vault: root, selected: Some(id), theme: crate::theme::Theme::light() };
        assert_eq!(st.selected_note().unwrap().title, "A");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p silo-ui app_state`
Expected: FAIL — `AppState` not defined.

- [ ] **Step 3: Implement `AppState`**

Prepend to `crates/silo-ui/src/app_state.rs`:
```rust
use silo_core::{Note, NoteId, Notebook};
use crate::theme::Theme;

pub struct AppState {
    pub vault: Notebook,
    pub selected: Option<NoteId>,
    pub theme: Theme,
}

impl AppState {
    pub fn flat_notes(&self) -> Vec<&Note> {
        fn go<'a>(nb: &'a Notebook, out: &mut Vec<&'a Note>) {
            out.extend(nb.notes.iter());
            for c in &nb.children {
                go(c, out);
            }
        }
        let mut out = Vec::new();
        go(&self.vault, &mut out);
        out
    }

    pub fn selected_note(&self) -> Option<&Note> {
        let id = self.selected?;
        self.flat_notes().into_iter().find(|n| n.id == id)
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p silo-ui app_state`
Expected: PASS (2 tests).

- [ ] **Step 5: Build the three-pane UI**

Rewrite `crates/silo-ui/src/lib.rs` to render sidebar + list + reader from `AppState`. Clicking a note in the list sets `selected`. Reconcile GPUI event/handler API against the pinned rev.
```rust
use gpui::*;
use silo_core::{Note, Notebook};
use std::path::PathBuf;

mod app_state;
mod theme;

use app_state::AppState;
use theme::Theme;

impl Render for AppState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme.clone();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.bg)
            .child(titlebar(&t))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .child(sidebar(&t, &self.vault))
                    .child(note_list(&t, self, cx))
                    .child(reader(&t, self)),
            )
    }
}

fn titlebar(t: &Theme) -> impl IntoElement {
    let dot = |c| div().w(px(12.0)).h(px(12.0)).bg(c);
    div()
        .flex().items_center().gap(px(8.0)).h(px(36.0)).px(px(12.0))
        .bg(t.surface).border_b_1().border_color(t.divider)
        .child(dot(rgb(0xff5f57))).child(dot(rgb(0xfebc2e))).child(dot(rgb(0x28c840)))
}

fn sidebar(t: &Theme, vault: &Notebook) -> impl IntoElement {
    let mut col = div().flex().flex_col().w(px(220.0)).h_full()
        .bg(t.surface).border_r_1().border_color(t.divider).p(px(12.0));
    col = col.child(div().text_color(t.text).child(vault.name.clone()));
    for child in &vault.children {
        col = col.child(div().text_color(t.text).pl(px(12.0)).child(child.name.clone()));
    }
    col
}

fn note_list(t: &Theme, st: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let mut col = div().flex().flex_col().w(px(280.0)).h_full()
        .bg(t.bg).border_r_1().border_color(t.divider);
    for n in st.flat_notes() {
        let id = n.id;
        let selected = st.selected == Some(id);
        col = col.child(
            div()
                .px(px(12.0)).py(px(8.0))
                .border_b_1().border_color(t.divider)
                .when(selected, |d| d.bg(t.surface))
                .text_color(t.text)
                .child(n.title.clone())
                .on_mouse_down(MouseButton::Left, cx.listener(move |st, _ev, _win, cx| {
                    st.selected = Some(id);
                    cx.notify();
                })),
        );
    }
    col
}

fn reader(t: &Theme, st: &AppState) -> impl IntoElement {
    let content: String = match st.selected_note() {
        Some(n) => format!("{}\n\n{}", n.title, n.body),
        None => "Select a note".to_string(),
    };
    div().flex().flex_1().h_full().p(px(24.0)).bg(t.bg)
        .text_color(t.text)
        .child(content)
}

pub fn run(vault_path: PathBuf) -> anyhow::Result<()> {
    let vault = silo_vault::walk_vault(&vault_path)?;
    Application::new().run(move |cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|_cx| AppState { vault: vault.clone(), selected: None, theme: Theme::light() })
        })
        .expect("failed to open window");
    });
    Ok(())
}
```
Note: the `reader` renders the raw markdown as text in M1; block rendering arrives in M2. `Notebook`/`Note` derive `Clone` (Task 3), so moving `vault` into the closure is fine.

- [ ] **Step 6: Pass the vault path from the CLI**

`app/silo/src/main.rs`:
```rust
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let vault_path: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./scratch-vault"));
    silo_ui::run(vault_path)
}
```

- [ ] **Step 7: Create a scratch vault and verify end-to-end**

```bash
mkdir -p scratch-vault/research
printf '# Inbox\n\nQuick note with a [[Zettel]] link and #idea tag.\n' > scratch-vault/inbox.md
printf '# Zettel\n\nZettelkasten beats folders. See [[Inbox]].\n' > scratch-vault/research/zettel.md
cargo run -p silo -- ./scratch-vault
```
Expected: window opens with a sidebar showing the vault name + "research", a note list showing "Inbox" and "Zettel", and clicking a note shows its text in the reader pane — all in the Modernist light theme with square corners.

- [ ] **Step 8: Run the whole test suite + lint**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all tests pass; no clippy warnings.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat(ui): three-pane browse + read-only note view from a vault"
```

---

## Self-Review

**1. Spec coverage (against the M0–M1 slice of the spec):**
- M0 skeleton + GPUI window → Task 1. ✓
- Modernist theme (light/dark, 0px radius, titlebar dots) → Task 2. ✓
- Domain types, ULID identity → Task 3. ✓
- Markdown parse (title, `[[links]]`, `#tags`), frontmatter split → Task 4. ✓
- Vault read + folder→notebook tree, graceful malformed handling → Task 5. ✓
- Sidebar tree + note list + read-only reader → Task 6. ✓
- Crate boundaries / mobile-reuse (`core`/`markdown` GPUI-free, IO only in `vault`) → enforced by Cargo deps in Tasks 1,3,4,5. ✓
- Out of this plan by design: editing/autosave (M2), SQLite/FTS5 index + search + ⌘K (M3), `[[link]]` navigation + backlinks (M4), folder picker (M2), real timestamps via chrono (M2). Flagged inline.

**2. Placeholder scan:** No "TBD"/"add error handling"/"similar to Task N" — each code step is concrete. The only intentional stubs are empty crate `lib.rs` files created in Task 1 and filled in named later tasks/plans, and `<REV>` which is a required user input (the pinned Zed SHA), documented in Global Constraints and `CLAUDE.md`. `now_rfc3339` is deliberately minimal with an inline note that M2 replaces it with `chrono`.

**3. Type consistency:** `NoteId`, `Frontmatter`, `Note`, `Notebook` field names are identical across Tasks 3, 5, 6. `split_frontmatter`/`derive_title`/`extract_links`/`extract_tags` signatures match between Task 4 (definition) and Task 5 (use). `AppState` fields match between Task 6 tests and impl. `run`'s signature change (Task 1 `run()` → Task 6 `run(PathBuf)`) is called out explicitly with the matching `main.rs` update.

**Known risk carried from the spec:** all GPUI code is representative of the common pattern and must be reconciled against the pinned Zed revision's `crates/gpui/examples/`. This is spec risk #1 and is surfaced in Global Constraints, Task 1 Step 8, and every GPUI step.
