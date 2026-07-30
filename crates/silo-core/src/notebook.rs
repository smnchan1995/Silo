use crate::note::{Note, NoteId};
use std::path::PathBuf;

/// A node in the unified vault tree. Notes and directories are the same thing:
/// every node may back a note file **and** may have child notes.
///
/// - The **root** has `note: None` (the vault directory itself is not a note).
/// - A normal note is a node with `note: Some(..)`; its children live in a sibling
///   folder named after it (`Foo.md` → children in `Foo/`), given by `path`.
/// - A directory that contains notes but has no backing `.md` yet is a **virtual**
///   node (`is_virtual: true`): it shows as a note and materializes into a real
///   `.md` on first edit.
#[derive(Clone, Debug)]
pub struct Notebook {
    /// Display name (note title, or folder name for the root/virtual nodes).
    pub name: String,
    /// The directory that holds this node's children (the vault dir for the root,
    /// or the note's sibling folder `<stem>/` otherwise — may not exist yet).
    pub path: PathBuf,
    /// The backing note file, if any. `None` only for the root.
    pub note: Option<Note>,
    /// True when `note` is a placeholder for a folder with no `.md` file yet.
    pub is_virtual: bool,
    /// Child notes, each a node in turn.
    pub children: Vec<Notebook>,
}

impl Notebook {
    /// Real notes (excludes the root and virtual folder-notes), depth-first.
    pub fn real_notes(&self) -> Vec<&Note> {
        let mut out = Vec::new();
        self.collect(&mut out, false);
        out
    }

    /// Every backing note including virtual folder-notes (for selection lookup).
    pub fn every_note(&self) -> Vec<&Note> {
        let mut out = Vec::new();
        self.collect(&mut out, true);
        out
    }

    fn collect<'a>(&'a self, out: &mut Vec<&'a Note>, include_virtual: bool) {
        if let Some(n) = &self.note {
            if include_virtual || !self.is_virtual {
                out.push(n);
            }
        }
        for c in &self.children {
            c.collect(out, include_virtual);
        }
    }

    /// Total real notes in this node and all descendants.
    pub fn note_count(&self) -> usize {
        self.real_notes().len()
    }

    /// Find a node (real or virtual) by its note id.
    pub fn find(&self, id: NoteId) -> Option<&Notebook> {
        if self.note.as_ref().map(|n| n.id) == Some(id) {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(id))
    }

    /// Does this node or any descendant back the given note id?
    pub fn contains(&self, id: NoteId) -> bool {
        self.find(id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(name: &str) -> Notebook {
        Notebook {
            name: name.into(),
            path: name.into(),
            note: None,
            is_virtual: false,
            children: vec![],
        }
    }

    #[test]
    fn note_count_ignores_nodes_without_notes() {
        let root = Notebook {
            children: vec![leaf("a")],
            ..leaf("root")
        };
        assert_eq!(root.note_count(), 0);
    }
}
