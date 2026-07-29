use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, MouseButton, Rgba, Window, WindowBounds,
    WindowOptions,
};
use gpui_platform::application;
use silo_core::Notebook;
use std::path::PathBuf;

mod app_state;
mod theme;

use app_state::AppState;
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
                    cx.listener(move |st, _ev, _win, cx| {
                        st.selected = Some(id);
                        cx.notify();
                    }),
                ),
        );
    }
    col
}

fn reader(t: &Theme, st: &AppState) -> impl IntoElement {
    let content: String = match st.selected_note() {
        Some(n) => format!("{}\n\n{}", n.title, n.body),
        None => "Select a note".to_string(),
    };
    div()
        .flex()
        .flex_1()
        .h_full()
        .p(px(24.0))
        .bg(t.bg)
        .text_color(t.text)
        .child(content)
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

pub fn run(vault_path: PathBuf) -> anyhow::Result<()> {
    let vault = silo_vault::walk_vault(&vault_path)?;
    application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| AppState {
                    vault,
                    selected: None,
                    theme: Theme::light(),
                })
            },
        )
        .expect("failed to open window");
    });
    Ok(())
}
