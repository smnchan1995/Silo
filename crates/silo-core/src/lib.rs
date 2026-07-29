pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod note;
pub mod notebook;

pub use note::{Frontmatter, Note, NoteId};
pub use notebook::Notebook;

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_not_empty() {
        assert!(!super::VERSION.is_empty());
    }
}
