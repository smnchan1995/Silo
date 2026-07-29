//! A multi-line text editor for note bodies, adapted from GPUI's `input.rs`
//! example (GPUI has no built-in text input). The example is single-line; this
//! shapes each logical line separately (`shape_line` asserts no newlines) and
//! stacks them, adding vertical cursor movement, per-line hit-testing, and a
//! Newline action.

use gpui::{
    actions, div, fill, point, prelude::*, px, relative, rgba, size, App, Bounds, ClipboardItem,
    Context, CursorStyle, Element, ElementId, ElementInputHandler, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, GlobalElementId, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, PaintQuad, Pixels, Point, Rgba, ShapedLine, SharedString, Style, TextAlign,
    TextRun, UTF16Selection, Window,
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

actions!(
    silo_editor,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        Home,
        End,
        SelectLeft,
        SelectRight,
        SelectAll,
        Newline,
        Paste,
        Cut,
        Copy,
    ]
);

/// Emitted whenever the buffer's content changes (not on cursor moves), so the
/// app can debounce a save. Programmatic `set_content` does not emit.
pub struct EditEvent;

/// Byte offset of the start of each logical line (split on `\n`).
fn line_starts(s: &str) -> Vec<usize> {
    let mut v = vec![0];
    for (i, b) in s.bytes().enumerate() {
        if b == b'\n' {
            v.push(i + 1);
        }
    }
    v
}

/// Map a byte offset to (line index, byte offset within that line).
fn offset_to_line_col(starts: &[usize], offset: usize) -> (usize, usize) {
    let line = match starts.binary_search(&offset) {
        Ok(i) => i,
        Err(i) => i - 1, // starts[0] == 0, so i >= 1 here
    };
    (line, offset - starts[line])
}

pub struct NoteEditor {
    focus_handle: FocusHandle,
    content: SharedString,
    text_color: Rgba,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_lines: Vec<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    line_height: Pixels,
}

impl NoteEditor {
    pub fn new(cx: &mut Context<Self>, content: &str, text_color: Rgba) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: content.to_string().into(),
            text_color,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_lines: Vec::new(),
            last_bounds: None,
            line_height: px(24.0),
        }
    }

    pub fn text(&self) -> String {
        self.content.to_string()
    }

    /// Replace the whole buffer (used when the selected note changes).
    pub fn set_content(&mut self, content: &str, cx: &mut Context<Self>) {
        self.content = content.to_string().into();
        let len = self.content.len();
        self.selected_range = len..len;
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    // --- vertical / line movement ------------------------------------------

    fn line_len(&self, starts: &[usize], line: usize) -> usize {
        let end = if line + 1 < starts.len() {
            starts[line + 1] - 1 // drop the '\n'
        } else {
            self.content.len()
        };
        end - starts[line]
    }

    fn vertical(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
        let starts = line_starts(&self.content);
        let cursor = self.cursor_offset();
        let (line, col) = offset_to_line_col(&starts, cursor);
        let x = self
            .last_lines
            .get(line)
            .map(|l| l.x_for_index(col))
            .unwrap_or(px(0.0));
        let target = if down {
            (line + 1).min(starts.len() - 1)
        } else {
            line.saturating_sub(1)
        };
        let new_col = self
            .last_lines
            .get(target)
            .map(|l| l.closest_index_for_x(x))
            .unwrap_or(0)
            .min(self.line_len(&starts, target));
        let new_offset = starts[target] + new_col;
        if extend {
            self.select_to(new_offset, cx);
        } else {
            self.move_to(new_offset, cx);
        }
    }

    // --- action handlers ----------------------------------------------------

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(false, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(true, false, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selection_reversed = false;
        self.selected_range = 0..self.content.len();
        cx.notify();
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let starts = line_starts(&self.content);
        let (line, _) = offset_to_line_col(&starts, self.cursor_offset());
        self.move_to(starts[line], cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let starts = line_starts(&self.content);
        let (line, _) = offset_to_line_col(&starts, self.cursor_offset());
        let offset = starts[line] + self.line_len(&starts, line);
        self.move_to(offset, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev {
                return;
            }
            self.select_to(prev, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                return;
            }
            self.select_to(next, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, "\n", window, cx);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    // --- mouse --------------------------------------------------------------

    fn on_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        // dragging with the primary button held extends the selection
        if event.pressed_button == Some(MouseButton::Left) {
            let offset = self.index_for_mouse_position(event.position);
            self.select_to(offset, cx);
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), false) = (self.last_bounds.as_ref(), self.last_lines.is_empty()) else {
            return 0;
        };
        let starts = line_starts(&self.content);
        let rel_y = (position.y - bounds.top()).max(px(0.0));
        let mut line = (f32::from(rel_y) / f32::from(self.line_height)) as usize;
        line = line.min(self.last_lines.len().saturating_sub(1));
        let rel_x = position.x - bounds.left();
        let col = self.last_lines[line]
            .closest_index_for_x(rel_x)
            .min(self.line_len(&starts, line));
        starts[line] + col
    }
}

// --- IME / platform text bridge --------------------------------------------

impl NoteEditor {
    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for ch in self.content.chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += ch.len_utf16();
            utf8 += ch.len_utf8();
        }
        utf8
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        let mut utf8 = 0;
        for ch in self.content.chars() {
            if utf8 >= offset {
                break;
            }
            utf8 += ch.len_utf8();
            utf16 += ch.len_utf16();
        }
        utf16
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }
}

impl EntityInputHandler for NoteEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range = None;
        cx.emit(EditEvent);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.marked_range = if new_text.is_empty() {
            None
        } else {
            Some(range.start..range.start + new_text.len())
        };
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .map(|r| r.start + range.start..r.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let starts = line_starts(&self.content);
        let range = self.range_from_utf16(&range_utf16);
        let (line, col) = offset_to_line_col(&starts, range.start);
        let shaped = self.last_lines.get(line)?;
        let x = shaped.x_for_index(col);
        let y = bounds.top() + self.line_height * (line as f32);
        Some(Bounds::from_corners(
            point(bounds.left() + x, y),
            point(bounds.left() + x, y + self.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.offset_to_utf16(self.index_for_mouse_position(point)))
    }
}

// --- rendering --------------------------------------------------------------

struct EditorElement {
    input: Entity<NoteEditor>,
}

struct PrepaintState {
    lines: Vec<ShapedLine>,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
}

impl IntoElement for EditorElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let line_count = self.input.read(cx).content.split('\n').count().max(1);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (window.line_height() * (line_count as f32)).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let text_color = input.text_color;
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();

        let starts = line_starts(&content);
        let mut lines = Vec::new();
        for seg in content.split('\n') {
            let text: SharedString = seg.to_string().into();
            let run = TextRun {
                len: text.len(),
                font: style.font(),
                color: text_color.into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let runs = if text.is_empty() { vec![] } else { vec![run] };
            let shaped = window
                .text_system()
                .shape_line(text, font_size, &runs, None);
            lines.push(shaped);
        }

        // cursor quad
        let (cl, cc) = offset_to_line_col(&starts, cursor);
        let cursor_quad = lines.get(cl).map(|line| {
            let x = line.x_for_index(cc);
            fill(
                Bounds::new(
                    point(bounds.left() + x, bounds.top() + line_height * (cl as f32)),
                    size(px(2.0), line_height),
                ),
                text_color,
            )
        });

        // selection quads (one per covered line)
        let mut selections = Vec::new();
        if !selected_range.is_empty() {
            let (sl, _) = offset_to_line_col(&starts, selected_range.start);
            let (el, _) = offset_to_line_col(&starts, selected_range.end);
            // li indexes starts, lines, and the vertical offset together.
            #[allow(clippy::needless_range_loop)]
            for li in sl..=el {
                let Some(line) = lines.get(li) else { continue };
                let line_start = starts[li];
                let line_len = line.len();
                let seg_start = selected_range
                    .start
                    .saturating_sub(line_start)
                    .min(line_len);
                let seg_end = if li == el {
                    selected_range.end.saturating_sub(line_start).min(line_len)
                } else {
                    line_len
                };
                if seg_end > seg_start {
                    let x0 = line.x_for_index(seg_start);
                    let x1 = line.x_for_index(seg_end);
                    let y = bounds.top() + line_height * (li as f32);
                    selections.push(fill(
                        Bounds::from_corners(
                            point(bounds.left() + x0, y),
                            point(bounds.left() + x1, y + line_height),
                        ),
                        rgba(0xec301330), // accent at low alpha
                    ));
                }
            }
        }

        PrepaintState {
            lines,
            cursor: cursor_quad,
            selections,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        let line_height = window.line_height();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        for quad in prepaint.selections.drain(..) {
            window.paint_quad(quad);
        }

        let lines = std::mem::take(&mut prepaint.lines);
        for (i, line) in lines.iter().enumerate() {
            let origin = point(bounds.left(), bounds.top() + line_height * (i as f32));
            let _ = line.paint(origin, line_height, TextAlign::Left, None, window, cx);
        }

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        self.input.update(cx, |input, _| {
            input.last_lines = lines;
            input.last_bounds = Some(bounds);
            input.line_height = line_height;
        });
    }
}

impl EventEmitter<EditEvent> for NoteEditor {}

impl Focusable for NoteEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for NoteEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("NoteEditor")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .size_full()
            .text_size(px(16.0))
            .line_height(px(24.0))
            .text_color(self.text_color)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(EditorElement { input: cx.entity() })
    }
}
