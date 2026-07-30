use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, CursorStyle, Div, Entity, FontWeight,
    KeyBinding, KeyDownEvent, MouseButton, Rgba, TitlebarOptions, Window, WindowBounds,
    WindowOptions,
};
use gpui_platform::application;
use silo_core::{Note, NoteId, Notebook};
use silo_vault::AppConfig;
use std::path::PathBuf;
use std::time::Duration;

mod app_state;
mod editor;
mod palette;
mod theme;

use app_state::{AppState, View};
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

fn menu_item(t: &Theme, text: &str) -> Div {
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
/// `active` renders it like the mockup's "Today" (accent + underline).
fn nav_placeholder(t: &Theme, name: &str, meta: &str, active: bool) -> Div {
    let mut name_el = div()
        .flex_1()
        .text_sm()
        .text_color(if active { t.accent } else { t.muted })
        .child(name.to_string());
    if active {
        name_el = name_el.font_weight(FontWeight::MEDIUM).underline();
    }
    let mut row = div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(6.0))
        .py(px(4.0))
        .child(bullet(if active { t.accent } else { t.faint }))
        .child(name_el);
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

/// A vertical divider inset from top/bottom so it never touches the toolbar or
/// footer rules (avoids intersecting lines). The outer column is full-height and
/// transparent; the inner 1px line is shortened by vertical margin.
fn vdivider(t: &Theme) -> impl IntoElement {
    div()
        .w(px(1.0))
        .h_full()
        .flex()
        .flex_col()
        .child(div().flex_1().my(px(12.0)).bg(t.divider))
}

// --- panes ------------------------------------------------------------------

/// Slim app toolbar under the native title bar. The theme item is live; the
/// others are placeholders.
fn toolbar(t: &Theme, dark: bool, cx: &mut Context<AppState>) -> impl IntoElement {
    let theme_label = if dark { "Light ◐" } else { "Dark ◐" };
    div()
        .flex()
        .items_center()
        .justify_end()
        .gap(px(16.0))
        .h(px(40.0))
        .px(px(16.0))
        .bg(t.bg)
        .border_b_1()
        .border_color(t.divider)
        .child(menu_item(t, "New ⌘N"))
        .child(menu_item(t, "⌘K"))
        .child(menu_item(t, "Day ⌘D"))
        .child(
            menu_item(t, theme_label)
                .cursor(CursorStyle::PointingHand)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|st, _e, _w, cx| st.toggle_theme(cx)),
                ),
        )
}

fn note_row(t: &Theme, note: &Note, selected: bool, cx: &mut Context<AppState>) -> Div {
    let id = note.id;
    let mut row = div()
        .px(px(6.0))
        .py(px(4.0))
        .cursor(CursorStyle::PointingHand)
        .text_sm()
        .text_color(if selected { t.accent } else { t.text })
        .child(note.title.clone());
    if selected {
        row = row.font_weight(FontWeight::SEMIBOLD).underline();
    }
    row.on_mouse_down(
        MouseButton::Left,
        cx.listener(move |st, _ev, window, cx| st.open_note(id, window, cx)),
    )
}

/// Does this subtree contain the given note?
fn subtree_contains(nb: &Notebook, id: NoteId) -> bool {
    nb.notes.iter().any(|n| n.id == id) || nb.children.iter().any(|c| subtree_contains(c, id))
}

/// A folder and its notes/subfolders as a nested tree node. The connector rule
/// and folder name light up in accent when the selected note is inside this
/// subtree, so the path to the open note is highlighted.
fn tree_node(t: &Theme, nb: &Notebook, st: &AppState, cx: &mut Context<AppState>) -> Div {
    let on_path = st
        .selected
        .map(|id| subtree_contains(nb, id))
        .unwrap_or(false);
    let rule = if on_path { t.accent } else { t.divider };
    let name_color = if on_path { t.text } else { t.muted };

    let mut nested = div()
        .flex()
        .flex_col()
        .gap(px(1.0))
        .ml(px(7.0))
        .pl(px(11.0))
        .border_l_1()
        .border_color(rule);
    for n in &nb.notes {
        nested = nested.child(note_row(t, n, st.selected == Some(n.id), cx));
    }
    for c in &nb.children {
        nested = nested.child(tree_node(t, c, st, cx));
    }

    div()
        .flex()
        .flex_col()
        .pt(px(4.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(6.0))
                .py(px(3.0))
                .child(bullet(if on_path { t.accent } else { t.faint }))
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(name_color)
                        .child(nb.name.clone()),
                ),
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
        tree = tree.child(tree_node(t, child, st, cx));
    }

    div()
        .flex()
        .flex_col()
        .w(px(232.0))
        .h_full()
        .bg(t.surface)
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
                .child(
                    nav_placeholder(t, "Today", "4 left", st.view == View::Today)
                        .cursor(CursorStyle::PointingHand)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|st, _e, _w, cx| st.show_today(cx)),
                        ),
                )
                .child(
                    div()
                        .pl(px(20.0))
                        .pb(px(2.0))
                        .text_xs()
                        .text_color(t.faint)
                        .child("week   month"),
                )
                .child(nav_placeholder(t, "Inbox", "2", false))
                // real notebooks
                .child(tree)
                // more placeholders
                .child(nav_placeholder(t, "Training", "wk 3/4", false))
                .child(nav_placeholder(t, "Travel", "2 trips", false))
                .child(nav_placeholder(t, "Journal", "", false)),
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

/// Outgoing `[[link]]` chips + "Linked mentions" for the selected note.
fn links_panel(t: &Theme, st: &AppState, cx: &mut Context<AppState>) -> Div {
    let mut chips = div().flex().flex_wrap().gap(px(6.0));
    for (title, id) in st.outgoing_links() {
        let follow = title.clone();
        chips = chips.child(
            div()
                .px(px(8.0))
                .py(px(3.0))
                .border_1()
                .border_color(t.divider)
                .cursor(CursorStyle::PointingHand)
                .text_xs()
                .text_color(if id.is_some() { t.accent } else { t.faint })
                .child(format!("[[{title}]]"))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |st, _e, w, cx| st.follow_link(follow.clone(), w, cx)),
                ),
        );
    }
    let mut mentions = div().flex().flex_col().gap(px(2.0));
    for b in st.backlinks_of_selected() {
        let id = b.from_id;
        mentions = mentions.child(
            div()
                .text_sm()
                .text_color(t.text)
                .cursor(CursorStyle::PointingHand)
                .child(b.from_title.clone())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |st, _e, w, cx| st.open_note(id, w, cx)),
                ),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .pt(px(14.0))
        .pb(px(18.0))
        .border_t_1()
        .border_color(t.divider)
        .child(label(t, "Links"))
        .child(chips)
        .child(div().pt(px(8.0)).child(label(t, "Linked mentions")))
        .child(mentions)
}

// --- Today planner (static placeholder layout) ------------------------------

fn checkbox(t: &Theme, done: bool) -> Div {
    if done {
        div().w(px(15.0)).h(px(15.0)).bg(t.accent)
    } else {
        div()
            .w(px(15.0))
            .h(px(15.0))
            .border_1()
            .border_color(t.faint)
    }
}

fn planner_task(t: &Theme, text: &str, done: bool, note: &str) -> Div {
    let mut lbl = div()
        .flex_1()
        .text_color(if done { t.muted } else { t.text })
        .child(text.to_string());
    if done {
        lbl = lbl.line_through();
    }
    let mut row = div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .py(px(5.0))
        .child(checkbox(t, done))
        .child(lbl);
    if !note.is_empty() {
        row = row.child(div().text_xs().text_color(t.faint).child(note.to_string()));
    }
    row
}

fn planner_habit(t: &Theme, text: &str, done: bool, count: &str, faded: bool) -> Div {
    let color = if faded {
        t.faint
    } else if done {
        t.muted
    } else {
        t.text
    };
    let mut lbl = div().flex_1().text_color(color).child(text.to_string());
    if done {
        lbl = lbl.line_through();
    }
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .py(px(5.0))
        .child(checkbox(t, done))
        .child(lbl)
        .child(div().text_xs().text_color(t.faint).child(count.to_string()))
}

fn week_strip(t: &Theme) -> Div {
    let days = ["T21", "W22", "T23", "F24", "S25", "S26", "M27"];
    let logged = [true, true, false, true, true, true, true];
    let mut row = div().flex().gap(px(18.0));
    for (i, d) in days.iter().enumerate() {
        let active = i == days.len() - 1;
        let mut day_lbl = div()
            .text_xs()
            .text_color(if active { t.accent } else { t.muted })
            .child(d.to_string());
        if active {
            day_lbl = day_lbl.font_weight(FontWeight::SEMIBOLD);
        }
        let dot = if !logged[i] {
            t.divider
        } else if active {
            t.accent
        } else {
            t.faint
        };
        row = row.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.0))
                .child(day_lbl)
                .child(div().w(px(6.0)).h(px(6.0)).bg(dot)),
        );
    }
    row
}

fn metrics_line(t: &Theme) -> Div {
    let pair = |k: &str, v: &str| {
        div()
            .flex()
            .gap(px(5.0))
            .child(div().text_color(t.text).child(k.to_string()))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text)
                    .child(v.to_string()),
            )
    };
    div()
        .flex()
        .items_center()
        .gap(px(20.0))
        .child(pair("sleep", "7.5h"))
        .child(pair("mood", "6"))
        .child(pair("run", "5k"))
        .child(div().text_color(t.faint).child("log: mood 7"))
}

fn today_view(t: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .max_w(px(720.0))
        .child(
            div()
                .text_xs()
                .text_color(t.muted)
                .child("JOURNAL  /  MON, JUL 27"),
        )
        .child(
            div()
                .pt(px(6.0))
                .text_size(px(34.0))
                .font_weight(FontWeight::EXTRA_BOLD)
                .text_color(t.text)
                .child("Today"),
        )
        .child(
            div()
                .w(px(56.0))
                .h(px(2.0))
                .bg(t.accent)
                .mt(px(6.0))
                .mb(px(18.0)),
        )
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .child(week_strip(t))
                .child(
                    div()
                        .text_xs()
                        .text_color(t.faint)
                        .child("dots = logged days"),
                ),
        )
        .child(div().pt(px(22.0)).pb(px(8.0)).child(label(t, "Tasks")))
        .child(planner_task(
            t,
            "send draft to Ana",
            false,
            "from Project notes",
        ))
        .child(planner_task(t, "book dentist", false, ""))
        .child(planner_task(t, "morning run 5k", true, ""))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .py(px(5.0))
                .child(
                    div()
                        .w(px(15.0))
                        .h(px(15.0))
                        .border_1()
                        .border_color(t.faint),
                )
                .child(div().text_color(t.faint).child("add a task…")),
        )
        .child(
            div()
                .pt(px(2.0))
                .text_xs()
                .text_color(t.faint)
                .child("drag to reorder"),
        )
        .child(div().h(px(1.0)).bg(t.divider).mt(px(16.0)).mb(px(16.0)))
        .child(metrics_line(t))
        .child(div().pt(px(22.0)).pb(px(8.0)).child(label(t, "Habits")))
        .child(planner_habit(t, "meds", true, "21d", false))
        .child(planner_habit(t, "stretch 10 min", false, "6d", false))
        .child(planner_habit(t, "read 20 pages", false, "skipped 4d", true))
        .child(
            div()
                .pt(px(2.0))
                .text_xs()
                .text_color(t.faint)
                .child("neglected habits fade out instead of nagging"),
        )
        .child(
            div()
                .pt(px(26.0))
                .text_color(t.muted)
                .child("Journal prose flows below — the planner is just the top of today's note."),
        )
}

fn content_pane(t: &Theme, st: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let pane = div()
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .bg(t.bg)
        .px(px(48.0))
        .pt(px(32.0));
    if st.view == View::Today {
        return pane.child(today_view(t));
    }
    match (st.selected_note().is_some(), st.editor.clone()) {
        (true, Some(ed)) => {
            let note = st.selected_note().unwrap();
            let dir = note
                .path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_uppercase();
            let crumb = format!("{dir}  /  {}", note.title);
            // Readable left-aligned column (content doesn't span the whole pane).
            pane.child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .max_w(px(720.0))
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.muted)
                            .pb(px(16.0))
                            .child(crumb),
                    )
                    .child(div().flex_1().child(ed))
                    .child(links_panel(t, st, cx)),
            )
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
        .w(px(248.0))
        .h_full()
        .bg(t.surface)
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
        .h(px(32.0))
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
        let palette_open = self.palette.open;
        let dark = self.is_dark();

        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.bg)
            .text_color(t.text);
        if let Some(h) = &self.focus_handle {
            root = root.track_focus(h);
        }
        // ⌘K works regardless of focus (bubbles up from the editor).
        root = root.on_action(cx.listener(AppState::toggle_palette));
        if palette_open {
            root = root
                .key_context("Palette")
                .on_action(cx.listener(AppState::palette_up))
                .on_action(cx.listener(AppState::palette_down))
                .on_action(cx.listener(AppState::palette_confirm))
                .on_action(cx.listener(AppState::palette_close))
                .on_key_down(cx.listener(|st, ev: &KeyDownEvent, _w, cx| {
                    if !st.palette.open {
                        return;
                    }
                    let ks = &ev.keystroke;
                    if ks.key == "backspace" {
                        st.palette.query.pop();
                        st.palette.selected = 0;
                        cx.notify();
                        return;
                    }
                    // single-character keys only (skip named keys like enter/up)
                    if ks.key.chars().count() == 1
                        && !ks.modifiers.platform
                        && !ks.modifiers.control
                    {
                        if let Some(c) = ks.key_char.as_ref() {
                            st.palette.query.push_str(c);
                            st.palette.selected = 0;
                            cx.notify();
                        }
                    }
                }));
        }

        root.child(toolbar(&t, dark, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(sidebar(&t, self, cx))
                    .child(vdivider(&t))
                    .child(content_pane(&t, self, cx))
                    .child(vdivider(&t))
                    .child(day_rail(&t)),
            )
            .child(footer_bar(&t, word_count))
            .children(palette::render(&t, self))
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

fn bind_palette_keys(cx: &mut App) {
    use palette::*;
    cx.bind_keys([
        KeyBinding::new("cmd-k", TogglePalette, None),
        KeyBinding::new("up", PaletteUp, Some("Palette")),
        KeyBinding::new("down", PaletteDown, Some("Palette")),
        KeyBinding::new("enter", PaletteConfirm, Some("Palette")),
        KeyBinding::new("escape", PaletteClose, Some("Palette")),
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
    let theme = if config.theme == "dark" {
        Theme::dark()
    } else {
        Theme::light()
    };
    let text_color = theme.text;
    let index = silo_index::Index::open_or_build(&vault_path, &vault).ok();
    let selected: Option<NoteId> = config.last_note.as_deref().and_then(|s| s.parse().ok());
    let view = if selected.is_some() {
        View::Note
    } else {
        View::Today
    };
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
                    view,
                    editor: Some(editor),
                    save_task: None,
                    _save_sub: Some(sub),
                    config,
                    config_path,
                    last_self_write: None,
                    saved_text: Some(initial_body),
                    index,
                    palette: palette::PaletteState::default(),
                    focus_handle: Some(cx.focus_handle()),
                }
            })
        },
    )
    .expect("failed to open window");
}

pub fn run() -> anyhow::Result<()> {
    application().run(|cx: &mut App| {
        bind_editor_keys(cx);
        bind_palette_keys(cx);
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
