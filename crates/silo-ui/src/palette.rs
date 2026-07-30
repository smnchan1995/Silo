//! ⌘K command palette: a modal overlay that fuzzy-finds notes (title + full
//! text via the index) and runs commands. Rendered on top via `deferred`.

use crate::app_state::AppState;
use crate::theme::Theme;
use gpui::{actions, deferred, div, prelude::*, px, rgba, AnyElement, Div, FontWeight};

actions!(
    silo_palette,
    [
        TogglePalette,
        PaletteUp,
        PaletteDown,
        PaletteConfirm,
        PaletteClose,
    ]
);

/// Commands shown after the search hits (order matters for confirm resolution).
pub const COMMANDS: [&str; 1] = ["New note"];

/// How many search hits the palette requests/shows.
pub const LIMIT: usize = 12;

#[derive(Default)]
pub struct PaletteState {
    pub open: bool,
    pub query: String,
    pub selected: usize,
}

fn row(t: &Theme, title: &str, sub: &str, selected: bool) -> Div {
    div()
        .flex()
        .flex_col()
        .px(px(14.0))
        .py(px(7.0))
        .when(selected, |d| d.bg(t.surface))
        .child(
            div()
                .text_color(if selected { t.accent } else { t.text })
                .when(selected, |d| d.font_weight(FontWeight::SEMIBOLD))
                .child(title.to_string()),
        )
        .when(!sub.is_empty(), |d| {
            d.child(div().text_xs().text_color(t.faint).child(sub.to_string()))
        })
}

/// The overlay element when the palette is open, else `None`.
pub fn render(t: &Theme, st: &AppState) -> Option<AnyElement> {
    if !st.palette.open {
        return None;
    }
    let hits = st.search(&st.palette.query, LIMIT);
    let total = hits.len() + COMMANDS.len();
    let sel = if total == 0 {
        0
    } else {
        st.palette.selected.min(total - 1)
    };

    let mut list = div().flex().flex_col().py(px(4.0));
    for (i, hit) in hits.iter().enumerate() {
        list = list.child(row(t, &hit.title, &hit.snippet, i == sel));
    }
    for (j, cmd) in COMMANDS.iter().enumerate() {
        let i = hits.len() + j;
        list = list.child(row(t, cmd, "command", i == sel));
    }

    let query_line = if st.palette.query.is_empty() {
        div().text_color(t.faint).child("Search notes and text…")
    } else {
        div().text_color(t.text).child(st.palette.query.clone())
    };

    let card = div()
        .w(px(560.0))
        .bg(t.bg)
        .border_1()
        .border_color(t.divider)
        .shadow_lg()
        .child(
            div()
                .px(px(14.0))
                .py(px(12.0))
                .border_b_1()
                .border_color(t.divider)
                .child(query_line),
        )
        .child(list);

    let backdrop = div()
        .absolute()
        .size_full()
        .flex()
        .justify_center()
        .pt(px(110.0))
        .bg(rgba(0x00000026))
        .child(card);

    Some(deferred(backdrop).into_any_element())
}
