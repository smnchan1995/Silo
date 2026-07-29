pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod note;
pub mod notebook;

pub use note::{Frontmatter, Note, NoteId};
pub use notebook::Notebook;

/// Current time as an RFC3339 UTC string (e.g. `2026-07-29T08:55:21Z`).
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_not_empty() {
        assert!(!super::VERSION.is_empty());
    }
}
