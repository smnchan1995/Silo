# Silo — build notes for agents

Native macOS notes app in Rust on GPUI. Design wedge: the "Modernist" look.
Spec: docs/superpowers/specs/2026-07-29-silo-notes-design.md
Roadmap: docs/superpowers/ROADMAP.md

## Commands
- Build:  `cargo build`
- Run:    `cargo run -p silo`            (once silo-ui/app exist — needs Xcode)
- Test:   `cargo test`                   (domain crates; silo-ui is verified by running)
- Format: `cargo fmt`
- Lint:   `cargo clippy --all-targets -- -D warnings`

## Architecture
- silo-core / silo-markdown: pure Rust, NO GPUI, NO IO — mobile-reuse boundary.
- silo-vault: filesystem read/write of .md notes.
- silo-index: SQLite/FTS5 (empty until the M3 plan).
- silo-ui: GPUI shell; app/silo: thin binary. (Added once Xcode is installed.)

## Environment prerequisites
- Rust (installed via rustup; `. "$HOME/.cargo/env"` if cargo isn't on PATH).
- FULL Xcode (not just Command Line Tools) is required to build GPUI — it compiles
  Metal shaders with the `metal`/`metallib` toolchain that CLT lacks. Until Xcode is
  installed, the GPUI crates (silo-ui, app/silo) are commented out of the workspace
  `Cargo.toml` and only the pure-Rust crates build/test.

## GPUI
Pinned Zed revision: <REV>   <-- choose a commit SHA, set it in Cargo.toml, record here.
GPUI API is not semver-stable. When GPUI code won't compile, check the examples in
that rev's crates/gpui/examples/ and reconcile.
