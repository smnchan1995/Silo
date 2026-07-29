# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Silo is a fast, local-first, keyboard-driven macOS notes app (Zettelkasten-style) written in **Rust on GPUI** (Zed's GPU-accelerated UI framework). The product wedge is design/taste — a "Modernist" aesthetic (square corners, warm paper background, one red-orange accent). Notes are plain Markdown files on disk; a rebuildable SQLite index sits alongside.

Design source of truth: `docs/superpowers/specs/2026-07-29-silo-notes-design.md`. Delivery is milestone-driven — see `docs/superpowers/ROADMAP.md` (M0–M1 foundation is built; M2 edit/save, M3 index/search, M4 links are sub-specced and get a bite-sized plan each before implementation).

## Commands

```bash
cargo build                      # build everything
cargo run -p silo -- ./vault     # run the app against a vault folder (arg optional; defaults to ./scratch-vault)
cargo test                       # all tests (domain crates; silo-ui is verified by running, not unit tests)
cargo test -p silo-markdown split_frontmatter   # a single test by crate + name
cargo fmt                        # format (CI-checked with --check)
cargo clippy --all-targets -- -D warnings        # lint gate: warnings are errors
```

If `cargo` isn't on PATH: `. "$HOME/.cargo/env"`.

## Environment prerequisites (do not skip)

- **Rust** via rustup.
- **Full Xcode** — *not* Command Line Tools. GPUI compiles Metal shaders with the `metal`/`metallib` toolchain that CLT lacks.
- **Metal Toolchain** (Xcode 26+ ships it as a separate component): `sudo xcodebuild -downloadComponent MetalToolchain`, or Xcode → Settings → Components. Verify with `xcrun -sdk macosx metal --version`.

Without these, the GPUI crates (`silo-ui`, `app/silo`) will not build; the pure-Rust crates still do.

## Architecture

A Cargo workspace deliberately splits **pure-Rust domain logic** from **GPUI**. The domain crates know nothing about the UI, which keeps them unit-testable (GPUI is hard to test) and is the reuse boundary for a possible future iOS app (GPUI does not target iOS).

- `crates/silo-core` — domain types (`NoteId` ULID, `Frontmatter`, `Note`, `Notebook` folder-tree). **No IO, no GPUI.**
- `crates/silo-markdown` — pure string parsing: `split_frontmatter`, `derive_title`, `extract_links` (`[[wiki-links]]`), `extract_tags` (`#tags`). **No IO, no GPUI.** Block-tree parsing lands here later without changing consumers.
- `crates/silo-vault` — filesystem: `read_note` (parses YAML frontmatter; **degrades to plain text on malformed frontmatter, never panics**), `walk_vault` (folder → `Notebook` tree, skips dotfiles/`.silo`).
- `crates/silo-index` — SQLite/FTS5 index. **Empty stub until the M3 plan.**
- `crates/silo-ui` — GPUI: `Theme` (design system), `AppState`, three-pane render (sidebar / note list / reader). Thin over the domain crates.
- `app/silo` — the binary; wires crates and opens the window. Takes the vault path as `argv[1]`.

**Load-bearing invariants** (violating these breaks the design intent):
- `silo-core` and `silo-markdown` must stay GPUI-free and IO-free.
- The vault (`.md` files) is the single source of truth; the index is **rebuildable and disposable** (`<vault>/.silo/index.sqlite`) and never has to sync.
- Data is kept **sync-ready** for a future multi-device story: stable ULIDs in frontmatter, an `updated` timestamp on every write, and no assumption that one process owns the files (an external-edit watcher reconciles).

## Design system

The "Modernist" look is a first-class subsystem in `crates/silo-ui/src/theme.rs`, not styling-as-you-go. A single `Theme` struct (light/dark). **No component hardcodes a hex color** — everything comes from `Theme`. **All radii are 0px** (square corners are the signature move). Tokens (paper `#f3f2f2`, ink `#201e1d`, accent `#ec3013`, Archivo font) come from the design spec.

## GPUI gotchas (this is the project's biggest risk area)

GPUI has no crates.io release and its API is **not semver-stable**. It is pinned to a specific Zed revision in the root `Cargo.toml` — currently `82aef44308540b576e4e51fb379efa71614e5c91`.

- The entry point at this rev is `gpui_platform::application()` (a `gpui` + `gpui_platform` crate split), **not** the older `Application::new().run()`. Both crates are depended on at the same pinned rev.
- Zed uses **edition 2024**; its `[patch.crates-io]` forks of `async-task`/`async-process` are replicated in our root `Cargo.toml` so the git dependency resolves.
- **`gpui_platform` must be depended on with `features = ["font-kit"]`** — it is not a default feature and `gpui`'s own `font-kit` does not forward across the crate split; without it, `gpui_macos` renders **no text**.
- When GPUI code won't compile after a rev bump, reconcile against that rev's `crates/gpui/examples/` (fetch the raw files at the exact SHA). Record the rev here and in `Cargo.toml`.
- `silo-ui` is verified by **running the app** (`cargo run -p silo -- ./scratch-vault`) and observing it, not by unit tests. `AppState`'s pure logic (`flat_notes`, `selected_note`) and the `Theme` tokens are the only unit-tested parts.

## Workflow conventions

Work is spec-driven via the superpowers skills: brainstorm → spec (`docs/superpowers/specs/`) → per-milestone sub-spec → bite-sized plan (`docs/superpowers/plans/`) → TDD execution → verify by running. Each milestone gets its full plan written only when it's about to be built.
