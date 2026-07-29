use crate::note::Note;
use std::path::PathBuf;

/// A notebook is a folder in the vault: subfolders are children, `.md` files are notes.
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
        self.notes.len()
            + self
                .children
                .iter()
                .map(Notebook::note_count)
                .sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_count_sums_descendants() {
        let leaf = Notebook {
            name: "a".into(),
            path: "a".into(),
            children: vec![],
            notes: vec![],
        };
        let root = Notebook {
            name: "root".into(),
            path: ".".into(),
            children: vec![leaf],
            notes: vec![],
        };
        assert_eq!(root.note_count(), 0);
    }
}
