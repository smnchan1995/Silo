use crate::editor::NoteEditor;
use crate::theme::Theme;
use gpui::{Context, Entity, Subscription, Task};
use silo_core::{Note, NoteId, Notebook};
use std::time::Duration;

pub struct AppState {
    pub vault: Notebook,
    pub selected: Option<NoteId>,
    pub theme: Theme,
    /// The body editor for the selected note. `None` in unit tests (no GPUI app).
    pub editor: Option<Entity<NoteEditor>>,
    /// Pending debounced autosave; replacing it cancels the previous timer.
    pub save_task: Option<Task<()>>,
    /// Keeps the editor edit-event subscription alive.
    pub _save_sub: Option<Subscription>,
}

impl AppState {
    /// Debounce a save ~500ms after the last edit.
    pub fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            this.update(cx, |st, cx| st.save_now(cx)).ok();
        }));
    }

    /// Write the editor's current text to the selected note's file.
    fn save_now(&mut self, cx: &mut Context<Self>) {
        let Some(ed) = self.editor.clone() else {
            return;
        };
        let text = ed.read(cx).text();
        let Some(note) = self.selected_note() else {
            return;
        };
        let updated = Note {
            id: note.id,
            path: note.path.clone(),
            title: note.title.clone(), // not persisted; derived on read
            frontmatter: note.frontmatter.clone(),
            body: text,
        };
        if let Err(e) = silo_vault::write_note(&updated) {
            eprintln!("autosave failed for {}: {e}", updated.path.display());
        }
    }
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
            editor: None,
            save_task: None,
            _save_sub: None,
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
            editor: None,
            save_task: None,
            _save_sub: None,
        };
        assert_eq!(st.selected_note().unwrap().title, "A");
    }
}
