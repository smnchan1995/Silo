use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, CursorStyle, Div, Entity, FontWeight,
    KeyBinding, MouseButton, Rgba, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;
use silo_core::{Note, NoteId, Notebook};
use silo_vault::AppConfig;
use std::path::PathBuf;
use std::time::Duration;

mod app_state;
mod editor;
mod theme;

use app_state::AppState;
use editor::{EditEvent, NoteEditor};
use theme::Theme;

// --- small building blocks --------------------------------------------------

/// Uppercase, muted, small — the Modernist section/label style.
fn label(t: &Theme, text: &str) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(t.muted)
        .child(text.to_uppercase())
}

fn menu_item(t: &Theme, text: &str) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(t.muted)
        .child(text.to_uppercase())
}

/// A small square bullet marker.
fn bullet(color: Rgba) -> impl IntoElement {
    div().w(px(6.0)).h(px(6.0)).bg(color)
}

/// A non-interactive sidebar nav entry for a feature that isn't built yet.
fn nav_placeholder(t: &Theme, name: &str, meta: &str) -> Div {
    let mut row = div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(6.0))
        .py(px(4.0))
        .child(bullet(t.faint))
        .child(
            div()
                .flex_1()
                .text_sm()
                .text_color(t.muted)
                .child(name.to_string()),
        );
    if !meta.is_empty() {
        row = row.child(div().text_xs().text_color(t.faint).child(meta.to_string()));
    }
    row
}

/// A placeholder day-view task/habit row (static).
fn day_task(t: &Theme, text: &str, done: bool, faded: bool) -> Div {
    let checkbox = if done {
        div().w(px(13.0)).h(px(13.0)).bg(t.accent)
    } else {
        div()
            .w(px(13.0))
            .h(px(13.0))
            .border_1()
            .border_color(t.faint)
    };
    let mut lbl = div().text_sm().child(text.to_string());
    lbl = if done {
        lbl.text_color(t.muted).line_through()
    } else if faded {
        lbl.text_color(t.faint)
    } else {
        lbl.text_color(t.text)
    };
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(checkbox)
        .child(lbl)
}

/// A static sparkline placeholder (a row of bars).
fn sparkline(t: &Theme) -> Div {
    let heights = [
        4.0, 7.0, 5.0, 9.0, 6.0, 8.0, 5.0, 10.0, 7.0, 6.0, 8.0, 4.0, 9.0, 7.0,
    ];
    let mut row = div().flex().items_end().gap(px(2.0)).h(px(12.0));
    for h in heights {
        row = row.child(div().w(px(3.0)).h(px(h)).bg(t.faint));
    }
    row
}

// --- panes ------------------------------------------------------------------

/// Slim app toolbar under the native title bar (menu actions are placeholders).
fn toolbar(t: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_end()
        .gap(px(16.0))
        .h(px(38.0))
        .px(px(16.0))
        .bg(t.bg)
        .border_b_1()
        .border_color(t.divider)
        .child(menu_item(t, "New ⌘N"))
        .child(menu_item(t, "⌘K"))
        .child(menu_item(t, "Day ⌘D"))
        .child(menu_item(t, "Dark ◐"))
}

/// Select a note: load its body into the editor, focus it, and persist last_note.
fn select_note(st: &mut AppState, id: NoteId, window: &mut Window, cx: &mut Context<AppState>) {
    st.selected = Some(id);
    let body = st
        .selected_note()
        .map(|n| n.body.clone())
        .unwrap_or_default();
    if let Some(ed) = st.editor.clone() {
        ed.update(cx, |e, cx| e.set_content(&body, cx));
        cx.focus_view(&ed, window);
    }
    st.saved_text = Some(body);
    st.config.last_note = Some(id.to_string());
    let _ = silo_vault::save_config(&st.config_path, &st.config);
    cx.notify();
}

fn note_row(t: &Theme, note: &Note, selected: bool, cx: &mut Context<AppState>) -> Div {
    let id = note.id;
    div()
        .flex()
        .items_center()
        .px(px(6.0))
        .py(px(4.0))
        .cursor(CursorStyle::PointingHand)
        .text_sm()
        .text_color(if selected { t.accent } else { t.text })
        .when(selected, |d| d.font_weight(FontWeight::SEMIBOLD))
        .child(note.title.clone())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |st, _ev, window, cx| select_note(st, id, window, cx)),
        )
}

/// A notebook and its notes/children, nested under a left rule (concrete `Div`
/// return type so it can recurse).
fn notebook_group(t: &Theme, nb: &Notebook, st: &AppState, cx: &mut Context<AppState>) -> Div {
    let count = nb.note_count();
    let mut nested = div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .ml(px(9.0))
        .pl(px(10.0))
        .border_l_1()
        .border_color(t.divider);
    for n in &nb.notes {
        nested = nested.child(note_row(t, n, st.selected == Some(n.id), cx));
    }
    for c in &nb.children {
        nested = nested.child(notebook_group(t, c, st, cx));
    }
    div()
        .flex()
        .flex_col()
        .pt(px(6.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(6.0))
                .py(px(4.0))
                .child(bullet(t.faint))
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child(nb.name.clone()),
                )
                .child(div().text_xs().text_color(t.faint).child(count.to_string())),
        )
        .child(nested)
}

fn sidebar(t: &Theme, st: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    // Real notebook tree.
    let mut tree = div().flex().flex_col().gap(px(2.0)).pt(px(4.0));
    for n in &st.vault.notes {
        tree = tree.child(note_row(t, n, st.selected == Some(n.id), cx));
    }
    for child in &st.vault.children {
        tree = tree.child(notebook_group(t, child, st, cx));
    }

    div()
        .flex()
        .flex_col()
        .w(px(248.0))
        .h_full()
        .bg(t.surface)
        .border_r_1()
        .border_color(t.divider)
        .child(
            // header: wordmark + section label
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(16.0))
                .pt(px(16.0))
                .pb(px(10.0))
                .child(
                    div()
                        .font_weight(FontWeight::EXTRA_BOLD)
                        .text_color(t.text)
                        .child("S."),
                )
                .child(label(t, "Notebooks")),
        )
        .child(
            // scrollable-ish body
            div()
                .flex()
                .flex_col()
                .flex_1()
                .px(px(10.0))
                // placeholders (planner views — not built yet)
                .child(nav_placeholder(t, "Today", "4 left"))
                .child(
                    div()
                        .pl(px(20.0))
                        .pb(px(2.0))
                        .text_xs()
                        .text_color(t.faint)
                        .child("week   month"),
                )
                .child(nav_placeholder(t, "Inbox", "2"))
                // real notebooks
                .child(tree)
                // more placeholders
                .child(nav_placeholder(t, "Training", "wk 3/4"))
                .child(nav_placeholder(t, "Travel", "2 trips"))
                .child(nav_placeholder(t, "Journal", "")),
        )
        .child(
            div()
                .px(px(16.0))
                .py(px(12.0))
                .text_xs()
                .text_color(t.faint)
                .child("⌘K search · ⌘N new · ⌘D day"),
        )
}

fn content_pane(t: &Theme, st: &AppState) -> impl IntoElement {
    let pane = div()
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .bg(t.bg)
        .px(px(40.0))
        .pt(px(28.0));
    match (st.selected_note(), st.editor.clone()) {
        (Some(note), Some(ed)) => {
            let dir = note
                .path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_uppercase();
            let crumb = format!("{dir}  /  {}", note.title);
            pane.child(
                div()
                    .text_xs()
                    .text_color(t.muted)
                    .pb(px(16.0))
                    .child(crumb),
            )
            .child(div().flex_1().child(ed))
        }
        _ => pane
            .items_center()
            .justify_center()
            .child(div().text_color(t.faint).child("Select a note")),
    }
}

fn day_rail(t: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w(px(272.0))
        .h_full()
        .bg(t.surface)
        .border_l_1()
        .border_color(t.divider)
        .px(px(16.0))
        .pt(px(16.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pb(px(4.0))
                .child(label(t, "Today"))
                .child(div().text_xs().text_color(t.faint).child("⌘D")),
        )
        .child(
            div()
                .pb(px(12.0))
                .text_xs()
                .text_color(t.faint)
                .child("preview — day view arrives in a later milestone"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(9.0))
                .child(day_task(t, "send draft to Ana", false, false))
                .child(day_task(t, "book dentist", false, false))
                .child(day_task(t, "morning run 5k", true, false))
                .child(day_task(t, "meds", true, false))
                .child(day_task(t, "stretch 10 min", false, false))
                .child(day_task(t, "read 20 pages", false, true)),
        )
        .child(
            div()
                .pt(px(16.0))
                .text_sm()
                .text_color(t.muted)
                .child("sleep 7.5h · mood 6 · run 5k"),
        )
        .child(div().pt(px(8.0)).child(sparkline(t)))
        .child(
            div()
                .pt(px(4.0))
                .text_xs()
                .text_color(t.faint)
                .child("sleep · 30 days"),
        )
        .child(div().flex_1())
        .child(
            div()
                .pb(px(14.0))
                .text_sm()
                .text_color(t.accent)
                .child("open Today ↗"),
        )
}

fn footer_bar(t: &Theme, word_count: usize) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(30.0))
        .px(px(16.0))
        .bg(t.bg)
        .border_t_1()
        .border_color(t.divider)
        .child(
            div()
                .text_xs()
                .text_color(t.faint)
                .child("local-first · plain markdown"),
        )
        .child(
            div()
                .text_xs()
                .text_color(t.muted)
                .child(format!("{word_count} words · saved locally")),
        )
}

impl Render for AppState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme.clone();
        let word_count = self
            .editor
            .as_ref()
            .map(|ed| ed.read(cx).text().split_whitespace().count())
            .unwrap_or(0);
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.bg)
            .text_color(t.text)
            .child(toolbar(&t))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(sidebar(&t, self, cx))
                    .child(content_pane(&t, self))
                    .child(day_rail(&t)),
            )
            .child(footer_bar(&t, word_count))
    }
}

fn bind_editor_keys(cx: &mut App) {
    use editor::*;
    let ctx = Some("NoteEditor");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("cmd-a", SelectAll, ctx),
        KeyBinding::new("enter", Newline, ctx),
        KeyBinding::new("cmd-c", Copy, ctx),
        KeyBinding::new("cmd-v", Paste, ctx),
        KeyBinding::new("cmd-x", Cut, ctx),
    ]);
}

/// Find a note's body by id anywhere in the tree.
fn find_note_body(nb: &Notebook, id: NoteId) -> Option<String> {
    if let Some(n) = nb.notes.iter().find(|n| n.id == id) {
        return Some(n.body.clone());
    }
    nb.children.iter().find_map(|c| find_note_body(c, id))
}

/// Walk the vault and open the main window on it, restoring the last-open note.
fn open_main_window(cx: &mut App, config_path: PathBuf, config: AppConfig, vault_path: PathBuf) {
    let vault = match silo_vault::walk_vault(&vault_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("failed to open vault {}: {e}", vault_path.display());
            return;
        }
    };
    let theme = Theme::light();
    let text_color = theme.text;
    let index = silo_index::Index::open_or_build(&vault_path, &vault).ok();
    let selected: Option<NoteId> = config.last_note.as_deref().and_then(|s| s.parse().ok());
    let initial_body = selected
        .and_then(|id| find_note_body(&vault, id))
        .unwrap_or_default();
    let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Silo".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |_, cx| {
            let editor: Entity<NoteEditor> =
                cx.new(|cx| NoteEditor::new(cx, &initial_body, text_color));
            cx.new(|cx| {
                let sub = cx.subscribe(
                    &editor,
                    |st: &mut AppState, _editor, _ev: &EditEvent, cx| {
                        st.schedule_save(cx);
                    },
                );
                // Watch the vault; drain change batches on a 300ms poll and reconcile.
                let rx = silo_vault::watch(&vault_path);
                cx.spawn(async move |this, cx| loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(300))
                        .await;
                    let mut paths = Vec::new();
                    while let Ok(batch) = rx.try_recv() {
                        paths.extend(batch);
                    }
                    if paths.is_empty() {
                        continue;
                    }
                    if this
                        .update(cx, |st, cx| st.reload_paths(paths, cx))
                        .is_err()
                    {
                        break;
                    }
                })
                .detach();
                AppState {
                    vault,
                    selected,
                    theme,
                    editor: Some(editor),
                    save_task: None,
                    _save_sub: Some(sub),
                    config,
                    config_path,
                    last_self_write: None,
                    saved_text: Some(initial_body),
                    index,
                }
            })
        },
    )
    .expect("failed to open window");
}

pub fn run() -> anyhow::Result<()> {
    application().run(|cx: &mut App| {
        bind_editor_keys(cx);
        let config_path = silo_vault::config_path();
        let config = silo_vault::load_config(&config_path);
        let existing = config.vault_path.as_ref().filter(|p| p.is_dir()).cloned();
        match existing {
            Some(vault_path) => open_main_window(cx, config_path, config, vault_path),
            None => {
                let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Choose a vault folder".into()),
                });
                cx.spawn(async move |cx| match rx.await {
                    Ok(Ok(Some(paths))) if !paths.is_empty() => {
                        let vault_path = paths.into_iter().next().unwrap();
                        cx.update(|cx| {
                            let mut config = config;
                            config.vault_path = Some(vault_path.clone());
                            let _ = silo_vault::save_config(&config_path, &config);
                            open_main_window(cx, config_path, config, vault_path);
                        });
                    }
                    _ => {
                        cx.update(|cx| cx.quit());
                    }
                })
                .detach();
            }
        }
    });
    Ok(())
}
