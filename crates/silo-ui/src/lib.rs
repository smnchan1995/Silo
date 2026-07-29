use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, Entity, KeyBinding, MouseButton, Rgba,
    Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;
use silo_core::Notebook;
use std::path::PathBuf;

mod app_state;
mod editor;
mod theme;

use app_state::AppState;
use editor::{EditEvent, NoteEditor};
use theme::Theme;

fn dot(color: Rgba) -> impl IntoElement {
    // Square corners: no rounding, per the Modernist system.
    div().w(px(12.0)).h(px(12.0)).bg(color)
}

fn titlebar(t: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .h(px(36.0))
        .px(px(12.0))
        .bg(t.surface)
        .border_b_1()
        .border_color(t.divider)
        .child(dot(rgb(0xff5f57)))
        .child(dot(rgb(0xfebc2e)))
        .child(dot(rgb(0x28c840)))
}

fn sidebar(t: &Theme, vault: &Notebook) -> impl IntoElement {
    let mut col = div()
        .flex()
        .flex_col()
        .w(px(220.0))
        .h_full()
        .bg(t.surface)
        .border_r_1()
        .border_color(t.divider)
        .p(px(12.0));
    col = col.child(div().text_color(t.text).child(vault.name.clone()));
    for child in &vault.children {
        col = col.child(
            div()
                .text_color(t.text)
                .pl(px(12.0))
                .child(child.name.clone()),
        );
    }
    col
}

fn note_list(t: &Theme, st: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let mut col = div()
        .flex()
        .flex_col()
        .w(px(280.0))
        .h_full()
        .bg(t.bg)
        .border_r_1()
        .border_color(t.divider);
    for n in st.flat_notes() {
        let id = n.id;
        let selected = st.selected == Some(id);
        col = col.child(
            div()
                .px(px(12.0))
                .py(px(8.0))
                .border_b_1()
                .border_color(t.divider)
                .when(selected, |d| d.bg(t.surface))
                .text_color(t.text)
                .child(n.title.clone())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |st, _ev, window, cx| {
                        st.selected = Some(id);
                        let body = st
                            .selected_note()
                            .map(|n| n.body.clone())
                            .unwrap_or_default();
                        if let Some(ed) = st.editor.clone() {
                            ed.update(cx, |e, cx| e.set_content(&body, cx));
                            cx.focus_view(&ed, window);
                        }
                        cx.notify();
                    }),
                ),
        );
    }
    col
}

fn reader(t: &Theme, st: &AppState) -> impl IntoElement {
    let pane = div().flex().flex_1().h_full().p(px(24.0)).bg(t.bg);
    match (st.selected_note().is_some(), st.editor.clone()) {
        (true, Some(ed)) => pane.child(ed),
        _ => pane.text_color(t.text).child("Select a note"),
    }
}

impl Render for AppState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme.clone();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.bg)
            .child(titlebar(&t))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .child(sidebar(&t, &self.vault))
                    .child(note_list(&t, self, cx))
                    .child(reader(&t, self)),
            )
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

pub fn run(vault_path: PathBuf) -> anyhow::Result<()> {
    let vault = silo_vault::walk_vault(&vault_path)?;
    application().run(move |cx: &mut App| {
        bind_editor_keys(cx);
        let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                let theme = Theme::light();
                let text_color = theme.text;
                let editor: Entity<NoteEditor> = cx.new(|cx| NoteEditor::new(cx, "", text_color));
                cx.new(|cx| {
                    let sub = cx.subscribe(
                        &editor,
                        |st: &mut AppState, _editor, _ev: &EditEvent, cx| {
                            st.schedule_save(cx);
                        },
                    );
                    AppState {
                        vault,
                        selected: None,
                        theme,
                        editor: Some(editor),
                        save_task: None,
                        _save_sub: Some(sub),
                    }
                })
            },
        )
        .expect("failed to open window");
    });
    Ok(())
}
