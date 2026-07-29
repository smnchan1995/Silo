use crate::theme::Theme;
use silo_core::{Note, NoteId, Notebook};

pub struct AppState {
    pub vault: Notebook,
    pub selected: Option<NoteId>,
    pub theme: Theme,
}

impl AppState {
    /// All notes across the notebook tree, depth-first.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use silo_core::{Frontmatter, Note, NoteId, Notebook};
    use std::path::PathBuf;

    fn note(title: &str) -> Note {
        let id = NoteId::new();
        Note {
            id,
            path: PathBuf::from(format!("{title}.md")),
            title: title.into(),
            frontmatter: Frontmatter {
                id,
                created: "".into(),
                updated: "".into(),
                tags: vec![],
                pinned: false,
            },
            body: format!("# {title}"),
        }
    }

    #[test]
    fn flat_notes_collects_across_children() {
        let child = Notebook {
            name: "c".into(),
            path: ".".into(),
            children: vec![],
            notes: vec![note("B")],
        };
        let root = Notebook {
            name: "root".into(),
            path: ".".into(),
            children: vec![child],
            notes: vec![note("A")],
        };
        let st = AppState {
            vault: root,
            selected: None,
            theme: Theme::light(),
        };
        let titles: Vec<_> = st.flat_notes().iter().map(|n| n.title.clone()).collect();
        assert!(titles.contains(&"A".to_string()) && titles.contains(&"B".to_string()));
    }

    #[test]
    fn selected_note_resolves_by_id() {
        let n = note("A");
        let id = n.id;
        let root = Notebook {
            name: "root".into(),
            path: ".".into(),
            children: vec![],
            notes: vec![n],
        };
        let st = AppState {
            vault: root,
            selected: Some(id),
            theme: Theme::light(),
        };
        assert_eq!(st.selected_note().unwrap().title, "A");
    }
}
