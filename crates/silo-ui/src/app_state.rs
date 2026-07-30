use crate::editor::NoteEditor;
use crate::theme::Theme;
use gpui::{Context, Entity, Subscription, Task};
use silo_core::{Note, NoteId, Notebook};
use silo_vault::AppConfig;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
    /// Persisted app config (last vault, last note, theme).
    pub config: AppConfig,
    /// Where `config` is stored.
    pub config_path: PathBuf,
    /// Path + time of our last autosave, so the watcher can ignore self-writes.
    pub last_self_write: Option<(PathBuf, Instant)>,
    /// The editor's content as we last read/wrote it — used to detect "dirty".
    pub saved_text: Option<String>,
    /// Rebuildable SQLite/FTS5 index. `None` in unit tests / on index failure.
    pub index: Option<silo_index::Index>,
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
        let updated = match self.selected_note() {
            Some(note) => Note {
                id: note.id,
                path: note.path.clone(),
                title: note.title.clone(), // not persisted; derived on read
                frontmatter: note.frontmatter.clone(),
                body: text.clone(),
            },
            None => return,
        };
        match silo_vault::write_note(&updated) {
            Ok(()) => {
                self.last_self_write = Some((updated.path.clone(), Instant::now()));
                self.saved_text = Some(text);
                if let Some(idx) = &self.index {
                    let _ = idx.upsert_note(&updated);
                }
            }
            Err(e) => eprintln!("autosave failed for {}: {e}", updated.path.display()),
        }
    }

    /// Reconcile external on-disk changes. Ignores our own recent autosave,
    /// re-walks the vault, and refreshes the open note's editor when it isn't dirty.
    pub fn reload_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let now = Instant::now();
        let paths: Vec<PathBuf> = paths
            .into_iter()
            .filter(|p| {
                !matches!(&self.last_self_write,
                    Some((sp, t)) if sp == p && now.duration_since(*t) < Duration::from_secs(1))
            })
            .collect();
        if paths.is_empty() {
            return;
        }

        let root = self.vault.path.clone();
        if let Ok(v) = silo_vault::walk_vault(&root) {
            self.vault = v;
        }

        let open = self
            .selected_note()
            .map(|n| (n.path.clone(), n.body.clone()));
        if let Some((note_path, new_body)) = open {
            if paths.iter().any(|p| p == &note_path) {
                let current = self.editor.as_ref().map(|ed| ed.read(cx).text());
                let dirty = current.as_deref() != self.saved_text.as_deref();
                if !dirty {
                    // no unsaved edits: adopt the on-disk version
                    if let Some(ed) = self.editor.clone() {
                        ed.update(cx, |e, cx| e.set_content(&new_body, cx));
                    }
                    self.saved_text = Some(new_body);
                } else if let Ok(disk) = std::fs::read_to_string(&note_path) {
                    // unsaved edits + external change: preserve both. Keep our
                    // edits (autosave persists them to the original); write the
                    // incoming disk version to a conflict sibling.
                    let stamp = silo_core::now_rfc3339().replace(':', "-");
                    let cp = silo_vault::conflict_path(&note_path, &stamp);
                    if let Err(e) = silo_vault::write_raw(&cp, &disk) {
                        eprintln!("failed to write conflict file {}: {e}", cp.display());
                    } else {
                        eprintln!(
                            "external change while editing; preserved incoming version at {}",
                            cp.display()
                        );
                    }
                }
            }
        }
        cx.notify();
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
            config: AppConfig::default(),
            config_path: PathBuf::from("/tmp/silo-test-config.json"),
            last_self_write: None,
            saved_text: None,
            index: None,
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
            config: AppConfig::default(),
            config_path: PathBuf::from("/tmp/silo-test-config.json"),
            last_self_write: None,
            saved_text: None,
            index: None,
        };
        assert_eq!(st.selected_note().unwrap().title, "A");
    }
}
