use chrono::Datelike;
use gpui::{
    div, ease_out_quint, img, linear_color_stop, linear_gradient, point, prelude::*, px, size,
    Animation, AnimationExt, App, Bounds, Context, CursorStyle, Div, Entity, FontWeight,
    KeyBinding, KeyDownEvent, MouseButton, MouseDownEvent, Rgba, SharedString, TitlebarOptions,
    Window, WindowBounds, WindowControlArea, WindowOptions,
};
use gpui_platform::application;
use silo_core::{NoteId, Notebook};
use silo_vault::AppConfig;
use std::path::PathBuf;
use std::time::Duration;

mod app_state;
mod editor;
mod palette;
mod planner;
mod theme;
mod travel;

use app_state::{AppState, NavAnim, TravelMode, View};
use editor::{EditEvent, NoteEditor};
use theme::Theme;

// --- small building blocks --------------------------------------------------

/// Uppercase, muted, small — the Modernist section/label style.
fn label(t: &Theme, text: &str) -> Div {
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

/// A small clickable sub-nav link (week / month), accent+underline when active.
fn subnav(t: &Theme, label: &str, active: bool, target: View, cx: &mut Context<AppState>) -> Div {
    let mut d = div()
        .text_xs()
        .cursor(CursorStyle::PointingHand)
        .text_color(if active { t.accent } else { t.faint })
        .child(label.to_string());
    if active {
        d = d.underline();
    }
    d.on_mouse_down(
        MouseButton::Left,
        cx.listener(move |st, _e, _w, cx| st.set_view(target, cx)),
    )
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

/// Height of the custom title bar (drawn into the transparent native titlebar).
const TITLEBAR_H: f32 = 38.0;

/// The app controls, rendered into the (transparent) native macOS title bar:
/// the wordmark + shortcuts + theme toggle on the right, with the whole bar a
/// window-drag region (double-click to zoom) and room on the left for the
/// traffic lights.
fn titlebar(t: &Theme, dark: bool, cx: &mut Context<AppState>) -> impl IntoElement {
    let theme_label = if dark { "Light ◐" } else { "Dark ◐" };
    div()
        .flex()
        .items_center()
        .h(px(TITLEBAR_H))
        .w_full()
        .bg(t.surface)
        .border_b_1()
        .border_color(t.divider)
        .pl(px(84.0)) // clear the traffic lights
        .pr(px(16.0))
        // The bar drags the window; double-click zooms it.
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|_st, ev: &MouseDownEvent, window, _cx| {
                if ev.click_count == 2 {
                    window.titlebar_double_click();
                }
            }),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.muted)
                .child("Silo"),
        )
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(16.0))
                .child(menu_item(t, "New ⌘N"))
                .child(menu_item(t, "⌘K"))
                .child(menu_item(t, "Day ⌘D"))
                .child(
                    menu_item(t, theme_label)
                        .cursor(CursorStyle::PointingHand)
                        .hover(|s| s.text_color(t.text))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|st, _e, _w, cx| st.toggle_theme(cx)),
                        ),
                ),
        )
}

/// A small, hover-revealed row action (× delete, ＋ new). Faint until the parent
/// row (identified by `group`) is hovered, then accent on its own hover.
fn row_action(
    t: &Theme,
    group: SharedString,
    glyph: &'static str,
    on_click: impl Fn(&mut AppState, &mut Window, &mut Context<AppState>) + 'static,
    cx: &mut Context<AppState>,
) -> Div {
    div()
        .invisible()
        .group_hover(group, |s| s.visible())
        .px(px(4.0))
        .text_xs()
        .text_color(t.faint)
        .cursor(CursorStyle::PointingHand)
        .hover(|s| s.text_color(t.accent))
        .child(glyph)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |st, _ev, window, cx| on_click(st, window, cx)),
        )
}

/// The chain of note-nodes from the top-level down to `target` (excludes the
/// vault root). `acc` is filled deepest-first; the caller reverses it. Built once
/// per render, so the tree render can look things up in O(1) rather than
/// re-scanning each node's subtree (which would make rendering O(n²)).
fn ancestor_chain<'a>(node: &'a Notebook, target: NoteId, acc: &mut Vec<&'a Notebook>) -> bool {
    if node.note.as_ref().map(|n| n.id) == Some(target) {
        acc.push(node);
        return true;
    }
    for c in &node.children {
        if ancestor_chain(c, target, acc) {
            if node.note.is_some() {
                acc.push(node);
            }
            return true;
        }
    }
    false
}

/// How many nesting levels the sidebar shows at once. Clicking into a deeper note
/// re-roots the view (via `focus`) so the note stays within this window.
const TREE_MAX_LEVELS: usize = 3;

/// A right-edge gradient that fades an overflowing title into the sidebar,
/// matching the row background (surface normally, hover on hover).
fn title_fade(t: &Theme, group: SharedString) -> Div {
    let surface_t = Rgba {
        a: 0.0,
        ..t.surface
    };
    let hover_t = Rgba { a: 0.0, ..t.hover };
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .right_0()
        .w(px(60.0))
        .bg(linear_gradient(
            90.0,
            linear_color_stop(surface_t, 0.35),
            linear_color_stop(t.surface, 1.0),
        ))
        .group_hover(group, move |s| {
            s.bg(linear_gradient(
                90.0,
                linear_color_stop(hover_t, 0.35),
                linear_color_stop(t.hover, 1.0),
            ))
        })
}

/// One node in the unified note tree: a note that may contain child notes.
/// Clicking the dot expands/collapses; the title opens the note; hover reveals
/// ＋ (add a child note) and × (delete this note + its subtree).
fn note_tree(
    t: &Theme,
    node: &Notebook,
    depth: usize,
    path: &std::collections::HashSet<NoteId>,
    st: &AppState,
    cx: &mut Context<AppState>,
) -> Div {
    let Some(note) = &node.note else {
        return div();
    };
    let id = note.id;
    let selected = st.selected == Some(id);
    let on_path = path.contains(&id);
    let has_children = !node.children.is_empty();
    let expanded = !st.collapsed.contains(&id);
    // At the deepest visible level a parent can't expand in place — its dot
    // "drills in" instead (opening it re-roots the window onto its children).
    let at_leaf_level = depth + 1 >= TREE_MAX_LEVELS;
    let drillable = has_children && at_leaf_level;
    let group: SharedString = format!("row-{id}").into();
    let branch = if on_path { t.accent } else { t.divider };
    let dot_color = if on_path || drillable {
        t.accent
    } else {
        t.faint
    };

    // A square dot marker on the connector line. Clicking it toggles collapse, or
    // drills into the note when it sits at the deepest visible level.
    let mut marker = div()
        .w(px(14.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(div().w(px(6.0)).h(px(6.0)).bg(dot_color));
    if drillable {
        marker = marker.cursor(CursorStyle::PointingHand).on_mouse_down(
            MouseButton::Left,
            cx.listener(move |st, _e, window, cx| st.open_note(id, window, cx)),
        );
    } else if has_children {
        marker = marker.cursor(CursorStyle::PointingHand).on_mouse_down(
            MouseButton::Left,
            cx.listener(move |st, _e, _w, cx| st.toggle_collapsed(id, cx)),
        );
    }

    // Title on a single line; overflow fades into the sidebar at the right edge.
    let mut title_text = div()
        .whitespace_nowrap()
        .text_color(if selected {
            t.accent
        } else if node.is_virtual {
            t.muted
        } else {
            t.text
        })
        .child(node.name.clone());
    if selected {
        title_text = title_text.font_weight(FontWeight::SEMIBOLD).underline();
    }
    let title = div()
        .relative()
        .flex_1()
        .overflow_hidden()
        .cursor(CursorStyle::PointingHand)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |st, _ev, window, cx| st.open_note(id, window, cx)),
        )
        .child(title_text)
        .child(title_fade(t, group.clone()));

    let child_dir = node.path.clone();
    let note_path = note.path.clone();
    let children_dir = node.path.clone();

    // The ＋/× actions overlay the row's right edge (absolute, so they don't
    // reserve layout width — otherwise the title, and its fade, would stop short
    // of the sidebar edge).
    let actions = div()
        .absolute()
        .top_0()
        .bottom_0()
        .right(px(6.0))
        .flex()
        .items_center()
        .gap(px(2.0))
        .child(row_action(
            t,
            group.clone(),
            "＋",
            move |st, window, cx| st.new_note_in(child_dir.clone(), window, cx),
            cx,
        ))
        .child(row_action(
            t,
            group.clone(),
            "×",
            move |st, _w, cx| st.delete_node(note_path.clone(), children_dir.clone(), cx),
            cx,
        ));

    let mut row = div()
        .group(group)
        .relative()
        .flex()
        .items_center()
        .gap(px(6.0))
        .pl(px(6.0))
        .pr(px(6.0))
        .py(px(4.0))
        .text_sm()
        .hover(|s| s.bg(t.hover));
    // Elbow connector: a short horizontal stub from the parent's vertical spine
    // to this row's dot, giving the branch lines a proper tree character.
    if depth > 0 {
        row = row.child(
            div()
                .absolute()
                .left(px(-10.0))
                .top(px(11.0))
                .w(px(13.0))
                .h(px(1.0))
                .bg(branch),
        );
    }
    let row = row.child(marker).child(title).child(actions);

    let mut col = div().flex().flex_col().child(row);
    if has_children && expanded && depth + 1 < TREE_MAX_LEVELS {
        // Nested children hang off an indented vertical connector line, which
        // glows accent when the selected note is inside this subtree.
        let mut nested = div()
            .flex()
            .flex_col()
            .ml(px(10.0))
            .pl(px(10.0))
            .border_l_1()
            .border_color(branch);
        for c in &node.children {
            nested = nested.child(note_tree(t, c, depth + 1, path, st, cx));
        }
        col = col.child(nested);
    }
    col
}

fn sidebar(t: &Theme, st: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    // Ancestor chain to the selected note (top-level → selected).
    let mut chain = Vec::new();
    if let Some(sel) = st.selected {
        ancestor_chain(&st.vault, sel, &mut chain);
    }
    chain.reverse();
    let path: std::collections::HashSet<NoteId> = chain
        .iter()
        .filter_map(|n| n.note.as_ref().map(|x| x.id))
        .collect();

    // Keep the selected note within a 3-level window by re-rooting on a suitable
    // ancestor ("focus"); the trimmed ancestors show as a breadcrumb.
    let focus: Option<&Notebook> = if chain.len() >= TREE_MAX_LEVELS {
        Some(chain[chain.len() - TREE_MAX_LEVELS])
    } else {
        None
    };
    let top_children = focus.map_or(&st.vault.children, |f| &f.children);

    let mut tree = div().flex().flex_col().gap(px(1.0)).pt(px(4.0));
    if focus.is_some() {
        // Breadcrumb of trimmed ancestors — click one to pop the window up to it.
        let crumbs = &chain[..chain.len() - TREE_MAX_LEVELS + 1];
        let mut bc = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(3.0))
            .px(px(6.0))
            .pb(px(6.0))
            .text_xs()
            .text_color(t.muted);
        for (i, n) in crumbs.iter().enumerate() {
            let Some(nn) = n.note.as_ref() else { continue };
            let id = nn.id;
            if i > 0 {
                bc = bc.child(div().text_color(t.faint).child("/"));
            }
            bc = bc.child(
                div()
                    .cursor(CursorStyle::PointingHand)
                    .hover(|s| s.text_color(t.accent))
                    .child(n.name.clone())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |st, _e, window, cx| st.open_note(id, window, cx)),
                    ),
            );
        }
        tree = tree.child(bc);
    }
    for c in top_children {
        tree = tree.child(note_tree(t, c, 0, &path, st, cx));
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
                        .hover(|s| s.bg(t.hover))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|st, _e, _w, cx| st.set_view(View::Today, cx)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(12.0))
                        .pl(px(20.0))
                        .pb(px(2.0))
                        .child(subnav(t, "week", st.view == View::Week, View::Week, cx))
                        .child(subnav(t, "month", st.view == View::Month, View::Month, cx)),
                )
                // real notebooks
                .child(tree)
                // more placeholders
                .child(
                    nav_placeholder(t, "Training", "wk 3/4", st.view == View::Training)
                        .cursor(CursorStyle::PointingHand)
                        .hover(|s| s.bg(t.hover))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|st, _e, _w, cx| st.set_view(View::Training, cx)),
                        ),
                )
                .child(
                    nav_placeholder(t, "Travel", "2 trips", st.view == View::Travel)
                        .cursor(CursorStyle::PointingHand)
                        .hover(|s| s.bg(t.hover))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|st, _e, _w, cx| st.set_view(View::Travel, cx)),
                        ),
                )
                .child(
                    nav_placeholder(t, "Journal", "", st.view == View::Journal)
                        .cursor(CursorStyle::PointingHand)
                        .hover(|s| s.bg(t.hover))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|st, _e, _w, cx| st.set_view(View::Journal, cx)),
                        ),
                ),
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
                .hover(|s| s.bg(t.hover))
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
                .hover(|s| s.bg(t.hover))
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

/// A task row: click the checkbox or label to toggle done; hover reveals × to
/// delete. Shared by Today / Week / Month.
fn task_line(t: &Theme, task: &planner::Task, cx: &mut Context<AppState>) -> Div {
    let id = task.id;
    let group: SharedString = format!("task-{id}").into();
    let mut lbl = div()
        .flex_1()
        .whitespace_nowrap()
        .overflow_hidden()
        .text_color(if task.done { t.muted } else { t.text })
        .child(task.text.clone());
    if task.done {
        lbl = lbl.line_through();
    }
    div()
        .group(group.clone())
        .relative()
        .flex()
        .items_center()
        .gap(px(10.0))
        .py(px(4.0))
        .pr(px(16.0))
        .cursor(CursorStyle::PointingHand)
        .hover(|s| s.bg(t.hover))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |st, _e, _w, cx| st.toggle_task(id, cx)),
        )
        .child(checkbox(t, task.done))
        .child(lbl)
        .child(
            div()
                .absolute()
                .right(px(2.0))
                .top_0()
                .bottom_0()
                .flex()
                .items_center()
                .invisible()
                .group_hover(group, |s| s.visible())
                .child(
                    div()
                        .text_xs()
                        .text_color(t.faint)
                        .cursor(CursorStyle::PointingHand)
                        .hover(|s| s.text_color(t.accent))
                        .child("×")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |st, _e, _w, cx| st.delete_task(id, cx)),
                        ),
                ),
        )
}

/// The "add a task…" affordance for a given day (opens the naming prompt).
fn add_task_line(t: &Theme, date: chrono::NaiveDate, cx: &mut Context<AppState>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .py(px(4.0))
        .cursor(CursorStyle::PointingHand)
        .text_color(t.faint)
        .hover(|s| s.text_color(t.accent))
        .child(
            div()
                .w(px(15.0))
                .h(px(15.0))
                .border_1()
                .border_color(t.faint),
        )
        .child(div().child("add a task…"))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |st, _e, window, cx| st.begin_add_task(date, window, cx)),
        )
}

fn today_view(t: &Theme, st: &AppState, cx: &mut Context<AppState>) -> Div {
    let today = st.today;
    let mut col = div()
        .flex()
        .flex_col()
        .flex_1()
        .max_w(px(720.0))
        .child(view_header(
            t,
            &format!("Planner / {}", planner::day_title(today)),
            "Today",
            "",
        ))
        .child(div().pt(px(20.0)).pb(px(6.0)).child(label(t, "Tasks")));
    for task in st.tasks_on(today) {
        col = col.child(task_line(t, task, cx));
    }
    col.child(add_task_line(t, today, cx))
}

/// The heading block shared by planner views: breadcrumb, title + accent
/// underline, and an optional right-aligned note.
fn view_header(t: &Theme, crumb: &str, title: &str, right: &str) -> Div {
    let mut head = div().flex().items_start().justify_between().child(
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_xs()
                    .text_color(t.muted)
                    .child(crumb.to_uppercase()),
            )
            .child(
                div()
                    .pt(px(6.0))
                    .text_size(px(30.0))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .text_color(t.text)
                    .child(title.to_string()),
            )
            .child(div().w(px(56.0)).h(px(2.0)).bg(t.accent).mt(px(6.0))),
    );
    if !right.is_empty() {
        head = head.child(div().text_xs().text_color(t.faint).child(right.to_string()));
    }
    head
}

fn week_view(t: &Theme, st: &AppState, cx: &mut Context<AppState>) -> Div {
    let week = planner::week_of(st.today);
    let range = format!(
        "{} — {}",
        planner::day_title(week[0]),
        planner::day_title(week[6])
    );
    let mut cols = div()
        .flex()
        .flex_1()
        .border_t_1()
        .border_color(t.divider)
        .mt(px(18.0));
    for (i, day) in week.iter().enumerate() {
        let is_today = *day == st.today;
        let mut col = div().flex().flex_col().flex_1().pt(px(10.0)).px(px(8.0));
        if i > 0 {
            col = col.border_l_1().border_color(t.divider);
        }
        let mut hd = div()
            .text_xs()
            .pb(px(8.0))
            .whitespace_nowrap()
            .overflow_hidden()
            .text_color(if is_today { t.accent } else { t.muted })
            .child(planner::day_title(*day).to_uppercase());
        if is_today {
            hd = hd.font_weight(FontWeight::SEMIBOLD);
        }
        col = col.child(hd);
        for task in st.tasks_on(*day) {
            col = col.child(task_line(t, task, cx));
        }
        let day = *day;
        col = col.child(div().flex_1()).child(
            div()
                .pt(px(6.0))
                .text_xs()
                .text_color(t.faint)
                .cursor(CursorStyle::PointingHand)
                .hover(|s| s.text_color(t.accent))
                .child("＋")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |st, _e, window, cx| st.begin_add_task(day, window, cx)),
                ),
        );
        cols = cols.child(col);
    }
    div()
        .flex()
        .flex_col()
        .flex_1()
        .child(view_header(t, "Planner / Week", "This week", &range))
        .child(cols)
}

fn month_view(t: &Theme, st: &AppState, cx: &mut Context<AppState>) -> Div {
    let weekdays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let grid = planner::month_grid(st.planner_day);

    let mut header = div().flex().pb(px(6.0));
    for w in weekdays {
        header = header.child(div().flex_1().text_xs().text_color(t.muted).child(w));
    }

    let mut cal = div().flex().flex_col().border_t_1().border_color(t.text);
    for week in &grid {
        let mut row = div().flex().h(px(64.0));
        for cell in week {
            let mut c = div()
                .flex()
                .flex_col()
                .flex_1()
                .pt(px(6.0))
                .px(px(6.0))
                .border_b_1()
                .border_color(t.divider);
            if let Some(day) = cell {
                let day = *day;
                let is_today = day == st.today;
                let selected = day == st.planner_day;
                let mut num = div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if is_today { t.bg } else { t.text })
                    .child(day.day().to_string());
                if is_today {
                    num = num.px(px(5.0)).py(px(1.0)).bg(t.accent);
                }
                // one dot per task on the day (up to 4).
                let n = st.tasks_on(day).len().min(4);
                let mut dots = div().flex().gap(px(3.0)).pt(px(6.0));
                for _ in 0..n {
                    dots = dots.child(div().w(px(5.0)).h(px(5.0)).bg(t.faint));
                }
                c = c
                    .cursor(CursorStyle::PointingHand)
                    .hover(|s| s.bg(t.hover))
                    .when(selected, |d| d.bg(t.surface))
                    .child(div().flex().child(num))
                    .child(dots)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |st, _e, _w, cx| st.select_day(day, cx)),
                    );
            }
            row = row.child(c);
        }
        cal = cal.child(row);
    }

    // Selected day's tasks, with CRUD.
    let sel = st.planner_day;
    let mut day_panel = div()
        .flex()
        .flex_col()
        .pt(px(14.0))
        .child(label(t, &planner::day_title(sel)).pb(px(4.0)));
    for task in st.tasks_on(sel) {
        day_panel = day_panel.child(task_line(t, task, cx));
    }
    day_panel = day_panel.child(add_task_line(t, sel, cx));

    div()
        .flex()
        .flex_col()
        .flex_1()
        .child(view_header(
            t,
            "Planner / Month",
            &planner::month_title(st.planner_day),
            "",
        ))
        .child(div().pt(px(18.0)).child(header))
        .child(cal)
        .child(day_panel)
}

/// The text-entry prompt overlay (e.g. naming a new task), rendered on top.
fn render_prompt(t: &Theme, st: &AppState) -> Option<gpui::AnyElement> {
    if !st.prompt_open {
        return None;
    }
    let body = if st.prompt_text.is_empty() {
        div().text_color(t.faint).child("Type a name…")
    } else {
        div()
            .text_color(t.text)
            .child(format!("{}\u{258f}", st.prompt_text))
    };
    let card = div()
        .w(px(440.0))
        .bg(t.bg)
        .border_1()
        .border_color(t.divider)
        .shadow_lg()
        .child(
            div()
                .px(px(14.0))
                .py(px(10.0))
                .border_b_1()
                .border_color(t.divider)
                .child(label(t, &st.prompt_title)),
        )
        .child(div().px(px(14.0)).py(px(12.0)).child(body))
        .child(
            div()
                .px(px(14.0))
                .pb(px(10.0))
                .text_xs()
                .text_color(t.faint)
                .child("↩ add · esc cancel"),
        );
    let backdrop = div()
        .absolute()
        .size_full()
        .flex()
        .justify_center()
        .pt(px(140.0))
        .bg(gpui::rgba(0x00000026))
        .child(card);
    Some(gpui::deferred(backdrop).into_any_element())
}

/// Journal: dated entries are real notes on disk; full CRUD via the note system.
fn journal_view(t: &Theme, st: &AppState, cx: &mut Context<AppState>) -> Div {
    let entries = st.journal_entries();
    let new_btn = div()
        .text_xs()
        .text_color(t.accent)
        .cursor(CursorStyle::PointingHand)
        .hover(|s| s.underline())
        .child("＋ New entry")
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|st, _e, window, cx| st.new_journal(window, cx)),
        );

    let mut list = div().flex().flex_col().gap(px(1.0)).mt(px(16.0));
    if entries.is_empty() {
        list = list.child(
            div()
                .text_sm()
                .text_color(t.faint)
                .child("No entries yet — “＋ New entry” starts today's journal."),
        );
    }
    for note in entries {
        let id = note.id;
        let group: SharedString = format!("jr-{id}").into();
        let path = note.path.clone();
        let cdir = silo_vault::children_dir(&note.path);
        list = list.child(
            div()
                .group(group.clone())
                .relative()
                .flex()
                .items_center()
                .gap(px(10.0))
                .py(px(6.0))
                .pr(px(18.0))
                .border_b_1()
                .border_color(t.divider)
                .hover(|s| s.bg(t.hover))
                .child(
                    div()
                        .flex_1()
                        .cursor(CursorStyle::PointingHand)
                        .text_color(t.text)
                        .child(note.title.clone())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |st, _e, window, cx| st.open_note(id, window, cx)),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .right(px(2.0))
                        .top_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .invisible()
                        .group_hover(group, |s| s.visible())
                        .child(
                            div()
                                .text_xs()
                                .text_color(t.faint)
                                .cursor(CursorStyle::PointingHand)
                                .hover(|s| s.text_color(t.accent))
                                .child("×")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |st, _e, _w, cx| {
                                        st.delete_node(path.clone(), cdir.clone(), cx)
                                    }),
                                ),
                        ),
                ),
        );
    }

    div()
        .flex()
        .flex_col()
        .flex_1()
        .max_w(px(720.0))
        .child(view_header(t, "Journal", "Journal", ""))
        .child(new_btn)
        .child(list)
}

fn training_view(t: &Theme) -> Div {
    // Heatmap: 7 rows (days) × 20 cols (weeks), static intensity pattern.
    let mut heat = div().flex().flex_col().gap(px(3.0));
    let row_labels = ["M", "", "W", "", "F", "", ""];
    for (r, rl) in row_labels.iter().enumerate() {
        let mut row = div().flex().items_center().gap(px(3.0));
        row = row.child(
            div()
                .w(px(16.0))
                .text_xs()
                .text_color(t.muted)
                .child(rl.to_string()),
        );
        for c in 0..20usize {
            let level = (r * 3 + c * 7) % 5; // 0..4
            let pr = (r + c).is_multiple_of(19) && c > 2;
            let mut cell = div().w(px(13.0)).h(px(13.0));
            cell = if pr {
                cell.bg(t.accent)
            } else {
                cell.bg(t.text).opacity(0.06 + 0.16 * level as f32)
            };
            row = row.child(cell);
        }
        heat = heat.child(row);
    }

    let sessions: [(&str, &str, &str, bool); 3] = [
        ("27.7", "Push A", "48 min · 12,400 kg", true),
        ("25.7", "Run 8k", "41 min · 5:08/km", false),
        ("23.7", "Pull A", "52 min", false),
    ];
    let mut recent = div()
        .flex()
        .flex_col()
        .pt(px(8.0))
        .border_t_1()
        .border_color(t.text);
    for (date, name, stat, pr) in sessions.iter() {
        recent = recent.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .py(px(10.0))
                .border_b_1()
                .border_color(t.divider)
                .child(
                    div()
                        .flex()
                        .gap(px(20.0))
                        .child(
                            div()
                                .w(px(44.0))
                                .text_color(if *pr { t.accent } else { t.faint })
                                .child(date.to_string()),
                        )
                        .child(
                            div()
                                .text_color(if *pr { t.accent } else { t.text })
                                .child(name.to_string()),
                        ),
                )
                .child(div().text_sm().text_color(t.muted).child(stat.to_string())),
        );
    }

    div()
        .flex()
        .flex_col()
        .flex_1()
        .child(view_header(t, "Training", "Training", "last 20 weeks"))
        .child(
            div()
                .flex()
                .gap(px(20.0))
                .pt(px(18.0))
                .text_sm()
                .text_color(t.text)
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("14 sessions in July"),
                )
                .child(div().text_color(t.muted).child("12 wk streak"))
                .child(div().text_color(t.muted).child("3.2/wk average")),
        )
        .child(div().pt(px(16.0)).child(heat))
        .child(
            div()
                .pt(px(10.0))
                .text_xs()
                .text_color(t.faint)
                .child("less → more · intensity = volume × duration"),
        )
        .child(
            div()
                .pt(px(24.0))
                .pb(px(8.0))
                .child(label(t, "Recent sessions")),
        )
        .child(recent)
}

/// The Travel view: trip tabs, an itinerary timeline (click a day for its
/// route), a Google static map of the selected day, and a bookings checklist.
// Schedule timeline geometry.
const SCHED_START: u32 = 7 * 60; // 07:00
const SCHED_END: u32 = 22 * 60; // 22:00
const SCHED_SLOT_PX: f32 = 26.0; // height of one grid slot
const SCHED_COL_W: f32 = 232.0;
const SCHED_COL_GAP: f32 = 8.0;

/// A small pill button (used for the mode toggle and zoom levels).
fn pill(
    t: &Theme,
    text: &str,
    active: bool,
    on_click: impl Fn(&mut AppState, &mut Context<AppState>) + 'static,
    cx: &mut Context<AppState>,
) -> Div {
    div()
        .px(px(8.0))
        .py(px(2.0))
        .text_xs()
        .cursor(CursorStyle::PointingHand)
        .text_color(if active { t.accent } else { t.muted })
        .when(active, |d| d.font_weight(FontWeight::SEMIBOLD))
        .when(!active, |d| d.hover(|s| s.text_color(t.text)))
        .child(text.to_string())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |st, _e, _w, cx| on_click(st, cx)),
        )
}

fn travel_view(t: &Theme, st: &AppState, cx: &mut Context<AppState>) -> Div {
    let ti = st.trip.min(st.trips.len().saturating_sub(1));
    let trip = &st.trips[ti];

    // Trip tabs.
    let mut tabs = div().flex().gap(px(24.0)).mt(px(16.0));
    for (i, tp) in st.trips.iter().enumerate() {
        let active = i == ti;
        let mut tab = div()
            .cursor(CursorStyle::PointingHand)
            .pb(px(6.0))
            .flex()
            .flex_col()
            .hover(|s| s.bg(t.hover));
        if active {
            tab = tab.border_b_2().border_color(t.accent);
        }
        tab = tab
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if active { t.accent } else { t.text })
                    .child(tp.tab.to_uppercase()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(t.faint)
                    .mt(px(1.0))
                    .child(tp.sub.clone()),
            );
        tabs = tabs.child(tab.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |st, _e, _w, cx| st.set_trip(i, cx)),
        ));
    }

    // Toolbar: Schedule/Itinerary toggle, and (schedule only) zoom + day nav.
    let schedule = st.travel_mode == TravelMode::Schedule;
    let mut toolbar = div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .mt(px(14.0))
        .child(pill(
            t,
            "Schedule",
            schedule,
            |st, cx| st.set_travel_mode(TravelMode::Schedule, cx),
            cx,
        ))
        .child(pill(
            t,
            "Itinerary",
            !schedule,
            |st, cx| st.set_travel_mode(TravelMode::Itinerary, cx),
            cx,
        ));
    if schedule {
        let ndays = trip.days.len() as f32;
        let min_x = -((ndays - 1.0).max(0.0) * (SCHED_COL_W + SCHED_COL_GAP));
        toolbar = toolbar
            .child(div().w(px(16.0)))
            .child(div().text_xs().text_color(t.faint).child("zoom"));
        for (lbl, slot) in [
            ("5m", 5u32),
            ("15m", 15),
            ("30m", 30),
            ("1h", 60),
            ("3h", 180),
        ] {
            let active = st.slot_min == slot;
            toolbar = toolbar.child(pill(
                t,
                lbl,
                active,
                move |st, cx| st.set_slot_min(slot, cx),
                cx,
            ));
        }
        toolbar = toolbar
            .child(div().flex_1())
            .child(pill(
                t,
                "◀ day",
                false,
                move |st, cx| st.sched_nudge(SCHED_COL_W + SCHED_COL_GAP, min_x, cx),
                cx,
            ))
            .child(pill(
                t,
                "day ▶",
                false,
                move |st, cx| st.sched_nudge(-(SCHED_COL_W + SCHED_COL_GAP), min_x, cx),
                cx,
            ));
    }

    let body = if schedule {
        schedule_body(t, st, trip, cx)
    } else {
        itinerary_body(t, st, trip, ti, cx)
    };

    div()
        .flex()
        .flex_col()
        .flex_1()
        .child(view_header(
            t,
            &format!("Travel / {}", trip.crumb),
            &trip.title,
            &trip.when,
        ))
        .child(tabs)
        .child(toolbar)
        .child(body)
}

/// The schedule timeline: a fixed time gutter + horizontally-pannable day columns.
fn schedule_body(t: &Theme, st: &AppState, trip: &travel::Trip, cx: &mut Context<AppState>) -> Div {
    let slot = st.slot_min.max(5);
    let ppm = SCHED_SLOT_PX / slot as f32; // px per minute
    let total_h = (SCHED_END - SCHED_START) as f32 * ppm;
    let ndays = trip.days.len() as f32;
    let min_x = -((ndays - 1.0).max(0.0) * (SCHED_COL_W + SCHED_COL_GAP));

    // Time gutter: an hour label + line at each hour.
    let mut gutter = div().w(px(52.0)).flex_none().relative().h(px(total_h));
    let mut h = SCHED_START.div_ceil(60) * 60;
    while h <= SCHED_END {
        let y = (h - SCHED_START) as f32 * ppm;
        gutter = gutter.child(
            div()
                .absolute()
                .top(px(y - 6.0))
                .right(px(8.0))
                .text_xs()
                .text_color(t.faint)
                .child(travel::fmt_time(h)),
        );
        h += 60;
    }

    // Day columns, laid out in a row that pans horizontally.
    let mut cols = div()
        .flex()
        .flex_row()
        .gap(px(SCHED_COL_GAP))
        .ml(px(st.sched_x));
    for (i, day) in trip.days.iter().enumerate() {
        cols = cols.child(day_column(t, st, i, day, ppm, total_h, cx));
    }
    let viewport = div()
        .relative()
        .flex_1()
        .overflow_hidden()
        .h(px(total_h))
        .child(cols)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|st, ev: &gpui::MouseDownEvent, _w, cx| {
                st.sched_pan_start(f32::from(ev.position.x), cx)
            }),
        )
        .on_mouse_move(cx.listener(move |st, ev: &gpui::MouseMoveEvent, _w, cx| {
            st.sched_pan_to(f32::from(ev.position.x), min_x, cx)
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|st, _e, _w, cx| st.sched_pan_end(cx)),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|st, _e, _w, cx| st.sched_pan_end(cx)),
        );

    div()
        .flex()
        .flex_col()
        .flex_1()
        .mt(px(12.0))
        .child(
            div()
                .text_xs()
                .text_color(t.faint)
                .pb(px(8.0))
                .child("drag to move across days · + to add an event · × to remove"),
        )
        .child(
            div()
                .id("sched-scroll")
                .flex()
                .flex_1()
                .overflow_y_scroll()
                .child(gutter)
                .child(viewport),
        )
}

/// One day as a vertical time column with its events placed by time.
fn day_column(
    t: &Theme,
    st: &AppState,
    day_idx: usize,
    day: &travel::Day,
    ppm: f32,
    total_h: f32,
    cx: &mut Context<AppState>,
) -> Div {
    let selected = day_idx
        == st
            .trip_day
            .min(st.trips[st.trip].days.len().saturating_sub(1));
    // Header: date + title + "+ event".
    let header = div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .h(px(30.0))
        .px(px(6.0))
        .border_b_1()
        .border_color(if selected { t.accent } else { t.divider })
        .cursor(CursorStyle::PointingHand)
        .child(
            div()
                .flex_1()
                .flex()
                .items_baseline()
                .gap(px(6.0))
                .overflow_hidden()
                .child(div().text_xs().text_color(t.faint).child(day.date.clone()))
                .child(
                    div()
                        .text_sm()
                        .whitespace_nowrap()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if selected { t.accent } else { t.text })
                        .child(day.title.clone()),
                ),
        )
        .child(
            div()
                .text_sm()
                .text_color(t.muted)
                .cursor(CursorStyle::PointingHand)
                .hover(|s| s.text_color(t.accent))
                .child("+")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |st, _e, _w, cx| st.add_event(day_idx, cx)),
                ),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |st, _e, _w, cx| st.set_trip_day(day_idx, cx)),
        );

    // Timeline body with hour gridlines + event blocks.
    let mut grid = div()
        .relative()
        .w(px(SCHED_COL_W))
        .h(px(total_h))
        .bg(t.surface);
    let mut hh = SCHED_START.div_ceil(60) * 60;
    while hh <= SCHED_END {
        let y = (hh - SCHED_START) as f32 * ppm;
        grid = grid.child(
            div()
                .absolute()
                .top(px(y))
                .left(px(0.0))
                .w(px(SCHED_COL_W))
                .h(px(1.0))
                .bg(t.divider),
        );
        hh += 60;
    }
    for stop in day.timed() {
        grid = grid.child(sched_event(t, day_idx, stop, ppm, cx));
    }

    div()
        .flex()
        .flex_col()
        .w(px(SCHED_COL_W))
        .flex_none()
        .child(header)
        .child(grid)
}

/// A single event block positioned on the timeline by start + duration.
fn sched_event(
    t: &Theme,
    day_idx: usize,
    stop: &travel::Stop,
    ppm: f32,
    cx: &mut Context<AppState>,
) -> Div {
    let start = stop.start_min.max(SCHED_START);
    let y = (start - SCHED_START) as f32 * ppm;
    let height = (stop.dur_min as f32 * ppm).max(18.0);
    let tint = Rgba {
        a: 0.14,
        ..t.accent
    };
    let id = stop.id;
    let group: SharedString = format!("ev-{id}").into();
    let time = format!(
        "{}–{}",
        travel::fmt_time(stop.start_min),
        travel::fmt_time(stop.start_min + stop.dur_min)
    );

    let mut block = div()
        .group(group.clone())
        .absolute()
        .top(px(y))
        .left(px(3.0))
        .w(px(SCHED_COL_W - 6.0))
        .h(px(height))
        .bg(tint)
        .border_l_2()
        .border_color(t.accent)
        .px(px(6.0))
        .py(px(2.0))
        .overflow_hidden()
        .child(
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .whitespace_nowrap()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child(stop.name.clone()),
                )
                .child(
                    div()
                        .invisible()
                        .group_hover(group, |s| s.visible())
                        .text_xs()
                        .text_color(t.faint)
                        .cursor(CursorStyle::PointingHand)
                        .hover(|s| s.text_color(t.accent))
                        .child("×")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |st, _e, _w, cx| st.delete_event(day_idx, id, cx)),
                        ),
                ),
        );
    if height >= 30.0 {
        block = block.child(div().text_xs().text_color(t.muted).child(time));
    }
    block
}

/// The itinerary list + map + bookings (the non-schedule presentation).
fn itinerary_body(
    t: &Theme,
    st: &AppState,
    trip: &travel::Trip,
    ti: usize,
    cx: &mut Context<AppState>,
) -> Div {
    let day_idx = st.trip_day.min(trip.days.len().saturating_sub(1));

    // Itinerary timeline (left column).
    let mut timeline = div().relative().flex().flex_col().pl(px(18.0)).child(
        div()
            .absolute()
            .left(px(3.0))
            .top(px(10.0))
            .bottom(px(10.0))
            .w(px(1.0))
            .bg(t.divider),
    );
    for (i, day) in trip.days.iter().enumerate() {
        let selected = i == day_idx;
        let dot = if selected { px(7.0) } else { px(5.0) };
        let mut row = div()
            .relative()
            .flex()
            .flex_col()
            .py(px(7.0))
            .cursor(CursorStyle::PointingHand)
            .hover(|s| s.bg(t.hover))
            .child(
                div()
                    .absolute()
                    .left(px(-19.0))
                    .top(px(13.0))
                    .w(dot)
                    .h(dot)
                    .bg(if selected { t.accent } else { t.faint }),
            );
        let mut title = div()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .text_color(if selected { t.accent } else { t.text })
            .child(day.title.clone());
        if selected {
            title = title.underline();
        }
        row = row.child(
            div()
                .flex()
                .items_baseline()
                .gap(px(10.0))
                .child(
                    div()
                        .w(px(46.0))
                        .flex_none()
                        .text_xs()
                        .text_color(t.faint)
                        .child(day.date.clone()),
                )
                .child(title)
                .child(
                    div()
                        .text_xs()
                        .text_color(t.faint)
                        .child(format!("{} stops", day.stops.len())),
                ),
        );
        if selected {
            let mut stops = div()
                .flex()
                .flex_col()
                .gap(px(5.0))
                .mt(px(6.0))
                .ml(px(56.0));
            for stop in day.timed() {
                let mut item = div().flex().flex_col();
                if let Some(c) = &stop.commute {
                    item = item.child(
                        div()
                            .text_xs()
                            .text_color(t.faint)
                            .pl(px(14.0))
                            .pb(px(2.0))
                            .child(format!("⟶ {c}")),
                    );
                }
                let mut line = div()
                    .flex()
                    .items_baseline()
                    .gap(px(8.0))
                    .child(div().w(px(6.0)).h(px(6.0)).flex_none().bg(t.muted))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(stop.name.clone()),
                    );
                if !stop.activity.is_empty() {
                    line = line.child(
                        div()
                            .text_xs()
                            .text_color(t.muted)
                            .child(stop.activity.clone()),
                    );
                }
                stops = stops.child(item.child(line));
            }
            row = row.child(stops);
        } else {
            row = row.child(
                div()
                    .text_xs()
                    .text_color(t.faint)
                    .ml(px(56.0))
                    .mt(px(2.0))
                    .child(day.detail.clone()),
            );
        }
        timeline = timeline.child(row.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |st, _e, _w, cx| st.set_trip_day(i, cx)),
        ));
    }
    let left = div()
        .flex_1()
        .min_w(px(300.0))
        .child(label(t, "Itinerary — click a day for its route").pb(px(10.0)))
        .child(timeline);

    // Map + bookings (right column).
    let map = if let Some(path) = st.map_image_path() {
        img(path)
            .w_full()
            .h(px(360.0))
            .border_1()
            .border_color(t.divider)
            .into_any_element()
    } else {
        let msg = if st.maps_key.is_none() {
            "Set SILO_GOOGLE_MAPS_API_KEY to render the route map here."
        } else {
            "Loading map…"
        };
        div()
            .w_full()
            .h(px(360.0))
            .border_1()
            .border_color(t.divider)
            .bg(t.surface)
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_xs()
                    .text_color(t.faint)
                    .px(px(24.0))
                    .child(msg.to_string()),
            )
            .into_any_element()
    };
    let open_btn = div()
        .mt(px(8.0))
        .mb(px(18.0))
        .text_xs()
        .text_color(t.accent)
        .cursor(CursorStyle::PointingHand)
        .hover(|s| s.underline())
        .child("Open route in Google Maps ↗")
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|st, _e, _w, cx| st.open_route(cx)),
        );
    let mut books = div().flex().flex_col().gap(px(7.0));
    for (i, bk) in trip.bookings.iter().enumerate() {
        let is_done = bk.done;
        let mut lbl = div()
            .text_sm()
            .text_color(if is_done { t.muted } else { t.text })
            .child(bk.label.clone());
        if is_done {
            lbl = lbl.line_through();
        }
        let row = div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .py(px(1.0))
            .cursor(CursorStyle::PointingHand)
            .hover(|s| s.bg(t.hover))
            .child(checkbox(t, is_done))
            .child(lbl);
        books = books.child(row.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |st, _e, _w, cx| st.toggle_booking(ti, i, cx)),
        ));
    }
    let right = div()
        .flex_1()
        .min_w(px(340.0))
        .child(map)
        .child(open_btn)
        .child(label(t, "Bookings").pb(px(10.0)))
        .child(books);

    div()
        .flex()
        .flex_wrap()
        .items_start()
        .gap(px(32.0))
        .mt(px(18.0))
        .max_w(px(1100.0))
        .child(left)
        .child(right)
}

fn content_pane(t: &Theme, st: &AppState, cx: &mut Context<AppState>) -> Div {
    let pane = div()
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .bg(t.bg)
        .px(px(48.0))
        .pt(px(32.0));
    match st.view {
        View::Today => return pane.child(today_view(t, st, cx)),
        View::Week => return pane.child(week_view(t, st, cx)),
        View::Month => return pane.child(month_view(t, st, cx)),
        View::Training => return pane.child(training_view(t)),
        View::Travel => return pane.child(travel_view(t, st, cx)),
        View::Journal => return pane.child(journal_view(t, st, cx)),
        View::Note => {}
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
        let nav_seq = self.nav_seq;
        let nav_anim = self.nav_anim;
        // The entrance animation is only mounted briefly after a navigation
        // (see `begin_nav`), so a plain resize/maximize never runs it.
        let content = {
            let c = content_pane(&t, self, cx);
            if self.animating {
                c.with_animation(
                    ("content", nav_seq as usize),
                    Animation::new(Duration::from_millis(320)).with_easing(ease_out_quint()),
                    move |el, d| match nav_anim {
                        NavAnim::Drill => el.opacity(d).ml(px(20.0 * (1.0 - d))),
                        NavAnim::BackUp => el.opacity(d).ml(px(-20.0 * (1.0 - d))),
                        NavAnim::Rise => el.opacity(d).mt(px(6.0 * (1.0 - d))),
                    },
                )
                .into_any_element()
            } else {
                c.into_any_element()
            }
        };

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
                .on_action(cx.listener(AppState::palette_close));
        }
        // Key capture for the palette query and the text prompt (naming a task).
        if palette_open || self.prompt_open {
            root = root.on_key_down(cx.listener(|st, ev: &KeyDownEvent, window, cx| {
                let ks = &ev.keystroke;
                // Text prompt (e.g. naming a task) captures keys first.
                if st.prompt_open {
                    match ks.key.as_str() {
                        "enter" => st.prompt_confirm(window, cx),
                        "escape" => st.prompt_cancel(window, cx),
                        "backspace" => st.prompt_backspace(cx),
                        _ => {
                            if ks.key.chars().count() == 1
                                && !ks.modifiers.platform
                                && !ks.modifiers.control
                            {
                                if let Some(c) = ks.key_char.as_ref() {
                                    st.prompt_push(c, cx);
                                }
                            }
                        }
                    }
                    return;
                }
                if !st.palette.open {
                    return;
                }
                if ks.key == "backspace" {
                    st.palette.query.pop();
                    st.palette.selected = 0;
                    cx.notify();
                    return;
                }
                // single-character keys only (skip named keys like enter/up)
                if ks.key.chars().count() == 1 && !ks.modifiers.platform && !ks.modifiers.control {
                    if let Some(c) = ks.key_char.as_ref() {
                        st.palette.query.push_str(c);
                        st.palette.selected = 0;
                        cx.notify();
                    }
                }
            }));
        }

        root.child(titlebar(&t, dark, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(sidebar(&t, self, cx))
                    .child(content)
                    .child(day_rail(&t)),
            )
            .child(footer_bar(&t, word_count))
            .children(palette::render(&t, self))
            .children(render_prompt(&t, self))
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
    nb.find(id)
        .and_then(|n| n.note.as_ref())
        .map(|n| n.body.clone())
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
    let today = chrono::Local::now().date_naive();
    let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Silo".into()),
                // Transparent native titlebar so our own bar draws into it.
                appears_transparent: true,
                // Vertically center the traffic lights in our taller title bar.
                traffic_light_position: Some(point(px(13.0), px(13.0))),
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
                    animating: false,
                    nav_seq: 0,
                    nav_anim: NavAnim::Rise,
                    trips: travel::initial_trips(),
                    trip: 0,
                    trip_day: 0,
                    travel_mode: app_state::TravelMode::Schedule,
                    slot_min: 30,
                    sched_x: 0.0,
                    sched_drag: None,
                    next_event_id: travel::initial_trips()
                        .iter()
                        .flat_map(|t| t.days.iter())
                        .flat_map(|d| d.stops.iter())
                        .map(|s| s.id)
                        .max()
                        .unwrap_or(0)
                        + 1,
                    maps_key: std::env::var("SILO_GOOGLE_MAPS_API_KEY")
                        .ok()
                        .filter(|k| !k.trim().is_empty()),
                    map_inflight: std::collections::HashSet::new(),
                    collapsed: std::collections::HashSet::new(),
                    today,
                    planner_day: today,
                    tasks: planner::initial_tasks(today),
                    next_task_id: planner::initial_tasks(today)
                        .iter()
                        .map(|t| t.id)
                        .max()
                        .unwrap_or(0)
                        + 1,
                    prompt_open: false,
                    prompt_title: String::new(),
                    prompt_text: String::new(),
                    prompt_date: None,
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
