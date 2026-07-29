# Silo

A fast, local-first, keyboard-driven notes app for macOS — a personal knowledge base in the Zettelkasten tradition, built in **Rust on [GPUI](https://www.gpui.rs/)** (Zed's GPU-accelerated UI framework).

Your notes are plain Markdown files in a folder you own. Silo is a fast, native lens over them — no Electron, no webview, no lock-in. The design wedge is **taste**: a calm, editorial "Modernist" look (square corners, warm paper background, one loud red-orange accent) that stays out of the way of writing.

## Why

- **Design-first.** The best-looking, most focused notes app on the Mac — polish is a feature, not a finishing step.
- **Native-fast.** GPU-rendered on GPUI, aiming for Zed-grade smoothness. Obsidian is Electron and feels it; Silo isn't.
- **Local-first & yours.** Plain `.md` on disk (YAML frontmatter). A rebuildable SQLite index sits beside them for search and backlinks — the files are always the source of truth.

## Status

Early foundation (**M0–M1**): open Silo, point it at a folder, browse the notebook (folder) tree, and read a note rendered in the Modernist theme.

On the roadmap: Markdown **editing + autosave** (M2), **full-text search + ⌘K command palette** (M3), and **`[[wiki-links]]` + backlinks** (M4) — completing the v0.1 Zettelkasten spine. Structured note types (tasks, journal, calendar, travel) and a future multi-device/sync story come after. See [`docs/superpowers/ROADMAP.md`](docs/superpowers/ROADMAP.md).

## Getting started

Prerequisites (macOS): **Rust** (via rustup) and **full Xcode** with the Metal Toolchain — GPUI compiles Metal shaders, which Command Line Tools alone can't do. See [`CLAUDE.md`](CLAUDE.md) for the exact setup and verification steps.

```bash
# create a vault of Markdown notes (or point at an existing folder)
mkdir -p vault && printf '# Welcome\n\nA note with a [[Link]] and a #tag.\n' > vault/welcome.md

cargo run -p silo -- ./vault
```

## Architecture

A Cargo workspace splits pure-Rust domain logic from the GPUI layer — the domain crates are unit-testable and UI-agnostic.

| Crate | Responsibility |
|---|---|
| `silo-core` | Domain types (`NoteId`/ULID, `Frontmatter`, `Note`, `Notebook`). No IO, no UI. |
| `silo-markdown` | Frontmatter split, title, `[[link]]` / `#tag` extraction. No IO, no UI. |
| `silo-vault` | Read notes and walk a folder into a notebook tree; graceful on malformed files. |
| `silo-index` | SQLite/FTS5 index (search, backlinks, tags) — *coming in M3*. |
| `silo-ui` | GPUI shell: theme, panes, interaction. |
| `app/silo` | Binary that wires it together and opens the window. |

Full design and rationale: [`docs/superpowers/specs/2026-07-29-silo-notes-design.md`](docs/superpowers/specs/2026-07-29-silo-notes-design.md).

## Development

```bash
cargo test                                # domain-crate tests
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## License

MIT
