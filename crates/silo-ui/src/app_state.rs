use crate::editor::NoteEditor;
use crate::palette::{
    PaletteClose, PaletteConfirm, PaletteDown, PaletteState, PaletteUp, TogglePalette, COMMANDS,
    LIMIT,
};
use crate::planner;
use crate::theme::Theme;
use crate::travel;
use chrono::NaiveDate;
use gpui::{Context, Entity, FocusHandle, Subscription, Task, Window};
use silo_core::{Note, NoteId, Notebook};
use silo_vault::AppConfig;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Which surface the center pane shows.
#[derive(Clone, Copy, PartialEq)]
pub enum View {
    Note,
    Today,
    Week,
    Month,
    Training,
    Travel,
    Journal,
}

/// How the Travel view presents a trip.
#[derive(Clone, Copy, PartialEq)]
pub enum TravelMode {
    /// A day-schedule timeline (default): day columns, vertical time axis.
    Schedule,
    /// The itinerary list + map + bookings.
    Itinerary,
}

/// Direction of the content entrance animation for the current navigation.
#[derive(Clone, Copy, PartialEq)]
pub enum NavAnim {
    /// Drilling into a note — slide in from the right.
    Drill,
    /// Going back up to an overview — slide in from the left.
    BackUp,
    /// A gentle rise (fade + up).
    Rise,
}

pub struct AppState {
    pub vault: Notebook,
    pub selected: Option<NoteId>,
    pub theme: Theme,
    /// Center-pane view (a note, or the Today planner).
    pub view: View,
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
    /// ⌘K command palette state.
    pub palette: PaletteState,
    /// Focus target for the palette. `None` in unit tests.
    pub focus_handle: Option<FocusHandle>,
    /// True only briefly after a navigation, so the content entrance animation
    /// is mounted just for that window (never during a resize/maximize).
    pub animating: bool,
    /// Increments on each navigation; keys the entrance animation.
    pub nav_seq: u64,
    /// Direction for the current entrance animation.
    pub nav_anim: NavAnim,
    /// Travel view: owned, editable trips (CRUD lives here).
    pub trips: Vec<travel::Trip>,
    /// Travel view: selected trip index.
    pub trip: usize,
    /// Travel view: selected itinerary day (drives the map + route).
    pub trip_day: usize,
    /// Travel view: schedule timeline vs itinerary+map.
    pub travel_mode: TravelMode,
    /// Schedule zoom: minutes per grid slot (5/15/30/60/180).
    pub slot_min: u32,
    /// Schedule horizontal pan offset (px; ≤ 0 pans to later days).
    pub sched_x: f32,
    /// Active horizontal drag: the grab anchor (pointer_x − sched_x).
    pub sched_drag: Option<f32>,
    /// Next event id to hand out when creating a schedule event.
    pub next_event_id: u64,
    /// Google Maps API key (from `SILO_GOOGLE_MAPS_API_KEY`); `None` disables the
    /// inline static map (the stop list + browser button still work).
    pub maps_key: Option<String>,
    /// Static-map cache files currently being downloaded (avoids duplicate fetches).
    pub map_inflight: HashSet<PathBuf>,
    /// Sidebar tree nodes the user has collapsed (everything else is expanded).
    pub collapsed: HashSet<NoteId>,
    /// Today's date, captured at launch (drives the planner views).
    pub today: NaiveDate,
    /// The day the Month/Today views focus on (click a day to change it).
    pub planner_day: NaiveDate,
    /// Planner tasks (owned, editable) shared by Today/Week/Month.
    pub tasks: Vec<planner::Task>,
    /// Next task id to hand out.
    pub next_task_id: u64,
    /// A lightweight text prompt (e.g. naming a new task); reuses the palette's
    /// keystroke capture. `open` gates it; `date` is the task's day when adding.
    pub prompt_open: bool,
    pub prompt_title: String,
    pub prompt_text: String,
    pub prompt_date: Option<NaiveDate>,
}

impl AppState {
    /// Full-text search via the index (empty when there is no index).
    pub fn search(&self, query: &str, limit: usize) -> Vec<silo_index::SearchHit> {
        self.index
            .as_ref()
            .and_then(|i| i.search(query, limit).ok())
            .unwrap_or_default()
    }

    pub fn is_dark(&self) -> bool {
        self.config.theme == "dark"
    }

    /// Start a content entrance animation; auto-clears after the duration so the
    /// wrapper is absent during ordinary re-renders (e.g. window resize).
    fn begin_nav(&mut self, anim: NavAnim, cx: &mut Context<Self>) {
        self.nav_anim = anim;
        self.nav_seq = self.nav_seq.wrapping_add(1);
        self.animating = true;
        let seq = self.nav_seq;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(340))
                .await;
            let _ = this.update(cx, |st, cx| {
                if st.nav_seq == seq {
                    st.animating = false;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    // --- Travel view -------------------------------------------------------

    /// Cache file for the current trip/day/theme's static map, under `<vault>/.silo`.
    /// Theme is part of the key so the light and dark map styles cache separately.
    fn map_cache_path(&self) -> PathBuf {
        let theme = if self.is_dark() { "d" } else { "l" };
        self.vault
            .path
            .join(".silo")
            .join("mapcache")
            .join(format!("{}-{}-{theme}.png", self.trip, self.trip_day))
    }

    /// Path to the current trip/day's static map, if it has finished downloading.
    pub fn map_image_path(&self) -> Option<PathBuf> {
        let p = self.map_cache_path();
        p.exists().then_some(p)
    }

    /// Download the static map for the selected trip/day if we have a key and it
    /// isn't cached or already in flight. Notifies when the file lands.
    pub fn ensure_map(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.maps_key.clone() else {
            return;
        };
        let Some(trip) = self.trips.get(self.trip) else {
            return;
        };
        let Some(day) = trip.days.get(self.trip_day) else {
            return;
        };
        let Some(url) = travel::static_map_url(&day.stops, self.is_dark(), &key) else {
            return;
        };
        let path = self.map_cache_path();
        if path.exists() || self.map_inflight.contains(&path) {
            return;
        }
        self.map_inflight.insert(path.clone());
        let dir = path.parent().map(PathBuf::from);
        let dl = path.clone();
        cx.spawn(async move |this, cx| {
            let ok = cx
                .background_executor()
                .spawn(async move {
                    if let Some(d) = dir {
                        let _ = std::fs::create_dir_all(d);
                    }
                    std::process::Command::new("curl")
                        .args(["-sSfL", "--max-time", "20", "-o"])
                        .arg(&dl)
                        .arg(&url)
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false)
                        && dl.exists()
                })
                .await;
            let _ = this.update(cx, |st, cx| {
                st.map_inflight.remove(&path);
                if ok {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Switch to a trip (resets the selected day + pan).
    pub fn set_trip(&mut self, trip: usize, cx: &mut Context<Self>) {
        if trip >= self.trips.len() || trip == self.trip {
            return;
        }
        self.trip = trip;
        self.trip_day = 0;
        self.sched_x = 0.0;
        self.ensure_map(cx);
        self.begin_nav(NavAnim::Rise, cx);
    }

    /// Select an itinerary day (drives the map + route).
    pub fn set_trip_day(&mut self, day: usize, cx: &mut Context<Self>) {
        self.trip_day = day;
        self.ensure_map(cx);
        cx.notify();
    }

    /// Toggle a booking's done state.
    pub fn toggle_booking(&mut self, trip: usize, idx: usize, cx: &mut Context<Self>) {
        if let Some(b) = self
            .trips
            .get_mut(trip)
            .and_then(|t| t.bookings.get_mut(idx))
        {
            b.done = !b.done;
            cx.notify();
        }
    }

    /// Open the selected day's route in Google Maps in the system browser.
    pub fn open_route(&self, cx: &mut Context<Self>) {
        let Some(trip) = self.trips.get(self.trip) else {
            return;
        };
        let Some(day) = trip.days.get(self.trip_day) else {
            return;
        };
        if let Some(url) = travel::directions_url(trip, day) {
            cx.open_url(&url);
        }
    }

    // --- Travel schedule: mode, zoom, pan, CRUD ---------------------------

    pub fn set_travel_mode(&mut self, mode: TravelMode, cx: &mut Context<Self>) {
        self.travel_mode = mode;
        if mode == TravelMode::Itinerary {
            self.ensure_map(cx);
        }
        cx.notify();
    }

    /// Set the schedule zoom (minutes per grid slot).
    pub fn set_slot_min(&mut self, slot_min: u32, cx: &mut Context<Self>) {
        self.slot_min = slot_min.clamp(5, 180);
        cx.notify();
    }

    /// Begin a horizontal pan: record the grab anchor (pointer_x − current offset).
    pub fn sched_pan_start(&mut self, pointer_x: f32, cx: &mut Context<Self>) {
        self.sched_drag = Some(pointer_x - self.sched_x);
        cx.notify();
    }

    /// Continue a horizontal pan; clamps so at least part of the days stays in view.
    pub fn sched_pan_to(&mut self, pointer_x: f32, min_x: f32, cx: &mut Context<Self>) {
        if let Some(anchor) = self.sched_drag {
            self.sched_x = (pointer_x - anchor).clamp(min_x.min(0.0), 0.0);
            cx.notify();
        }
    }

    pub fn sched_pan_end(&mut self, cx: &mut Context<Self>) {
        if self.sched_drag.take().is_some() {
            cx.notify();
        }
    }

    /// Nudge the horizontal pan by `dx` px (◀ ▶ buttons), clamped.
    pub fn sched_nudge(&mut self, dx: f32, min_x: f32, cx: &mut Context<Self>) {
        self.sched_x = (self.sched_x + dx).clamp(min_x.min(0.0), 0.0);
        cx.notify();
    }

    /// Add a new event to a day, starting after the day's last event (snapped to
    /// the current slot), defaulting to 9:00 for an empty day.
    pub fn add_event(&mut self, day_idx: usize, cx: &mut Context<Self>) {
        let id = self.next_event_id;
        let slot = self.slot_min.max(5);
        if let Some(day) = self
            .trips
            .get_mut(self.trip)
            .and_then(|t| t.days.get_mut(day_idx))
        {
            let after = day
                .stops
                .iter()
                .map(|s| s.start_min + s.dur_min)
                .max()
                .unwrap_or(9 * 60);
            let start = (after / slot) * slot;
            day.stops.push(travel::Stop {
                id,
                name: "New stop".into(),
                activity: String::new(),
                commute: None,
                lat: 0.0,
                lng: 0.0,
                start_min: start,
                dur_min: 60,
            });
            self.next_event_id += 1;
            cx.notify();
        }
    }

    /// Delete an event by id from a day.
    pub fn delete_event(&mut self, day_idx: usize, event_id: u64, cx: &mut Context<Self>) {
        if let Some(day) = self
            .trips
            .get_mut(self.trip)
            .and_then(|t| t.days.get_mut(day_idx))
        {
            day.stops.retain(|s| s.id != event_id);
            cx.notify();
        }
    }

    // --- Planner tasks (Today / Week / Month) -----------------------------

    /// Tasks on a given day, undone first then by id.
    pub fn tasks_on(&self, date: NaiveDate) -> Vec<&planner::Task> {
        let mut v: Vec<&planner::Task> = self.tasks.iter().filter(|t| t.date == date).collect();
        v.sort_by_key(|t| (t.done, t.id));
        v
    }

    /// Focus the planner on a day (Month → click a day).
    pub fn select_day(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        self.planner_day = date;
        cx.notify();
    }

    pub fn toggle_task(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.done = !t.done;
            cx.notify();
        }
    }

    pub fn delete_task(&mut self, id: u64, cx: &mut Context<Self>) {
        self.tasks.retain(|t| t.id != id);
        cx.notify();
    }

    // --- Text prompt (name a new task) ------------------------------------

    /// Open the prompt to add a task on `date`.
    pub fn begin_add_task(&mut self, date: NaiveDate, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt_open = true;
        self.prompt_title = "New task".into();
        self.prompt_text.clear();
        self.prompt_date = Some(date);
        if let Some(h) = &self.focus_handle {
            window.focus(h, cx);
        }
        cx.notify();
    }

    pub fn prompt_push(&mut self, s: &str, cx: &mut Context<Self>) {
        self.prompt_text.push_str(s);
        cx.notify();
    }

    pub fn prompt_backspace(&mut self, cx: &mut Context<Self>) {
        self.prompt_text.pop();
        cx.notify();
    }

    pub fn prompt_cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt_open = false;
        self.prompt_text.clear();
        self.prompt_date = None;
        if let Some(ed) = self.editor.clone() {
            cx.focus_view(&ed, window);
        }
        cx.notify();
    }

    /// Confirm the prompt: create the task (if non-empty) on the pending day.
    pub fn prompt_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.prompt_text.trim().to_string();
        if let (false, Some(date)) = (text.is_empty(), self.prompt_date) {
            let id = self.next_task_id;
            self.next_task_id += 1;
            self.tasks.push(planner::Task {
                id,
                date,
                text,
                done: false,
            });
        }
        self.prompt_cancel(window, cx);
    }

    // --- Journal (entries are dated notes on disk) ------------------------

    /// The `Journal` folder in the vault (where dated entries live).
    fn journal_dir(&self) -> PathBuf {
        self.vault.path.join("Journal")
    }

    /// Journal entry notes, newest first (by title, which is the date).
    pub fn journal_entries(&self) -> Vec<&Note> {
        let Some(folder) = self.vault.children.iter().find(|c| c.name == "Journal") else {
            return vec![];
        };
        let mut entries: Vec<&Note> = folder
            .children
            .iter()
            .filter(|n| !n.is_virtual)
            .filter_map(|n| n.note.as_ref())
            .collect();
        entries.sort_by(|a, b| b.title.cmp(&a.title));
        entries
    }

    /// Create today's journal entry (a dated note) and open it.
    pub fn new_journal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.today.format("%Y-%m-%d").to_string();
        match silo_vault::create_note(&self.journal_dir(), &title) {
            Ok(note) => {
                let id = note.id;
                self.refresh_vault();
                self.open_note(id, window, cx);
            }
            Err(e) => eprintln!("new journal entry failed: {e}"),
        }
    }

    /// Switch the center pane to a planner view.
    pub fn set_view(&mut self, view: View, cx: &mut Context<Self>) {
        if view == View::Travel {
            self.ensure_map(cx);
        }
        // Leaving a note for an overview reads as "back up"; overview→overview rises.
        let anim = if self.view == View::Note {
            NavAnim::BackUp
        } else {
            NavAnim::Rise
        };
        self.view = view;
        self.begin_nav(anim, cx);
    }

    /// Flip light/dark, recolor the editor, and persist the choice.
    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        let dark = !self.is_dark();
        self.config.theme = if dark { "dark".into() } else { "light".into() };
        self.theme = if dark { Theme::dark() } else { Theme::light() };
        if let Some(ed) = self.editor.clone() {
            let c = self.theme.text;
            ed.update(cx, |e, cx| e.set_text_color(c, cx));
        }
        // The map is styled to the theme, so fetch the new-theme version if shown.
        if self.view == View::Travel {
            self.ensure_map(cx);
        }
        let _ = silo_vault::save_config(&self.config_path, &self.config);
        cx.notify();
    }

    /// Open a note by id: load its body into the editor, focus it, persist last_note.
    pub fn open_note(&mut self, id: NoteId, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_nav(NavAnim::Drill, cx);
        self.view = View::Note;
        self.selected = Some(id);
        let body = self
            .selected_note()
            .map(|n| n.body.clone())
            .unwrap_or_default();
        if let Some(ed) = self.editor.clone() {
            ed.update(cx, |e, cx| e.set_content(&body, cx));
            cx.focus_view(&ed, window);
        }
        self.saved_text = Some(body);
        self.config.last_note = Some(id.to_string());
        let _ = silo_vault::save_config(&self.config_path, &self.config);
        cx.notify();
    }

    /// The `[[titles]]` in the selected note's body, each resolved to an id if it exists.
    pub fn outgoing_links(&self) -> Vec<(String, Option<NoteId>)> {
        let Some(note) = self.selected_note() else {
            return vec![];
        };
        let idx = self.index.as_ref();
        silo_markdown::extract_links(&note.body)
            .into_iter()
            .map(|title| {
                let id = idx.and_then(|i| i.resolve_title(&title).ok().flatten());
                (title, id)
            })
            .collect()
    }

    /// Notes that link to the selected note ("Linked mentions").
    pub fn backlinks_of_selected(&self) -> Vec<silo_index::Backlink> {
        match (self.selected, self.index.as_ref()) {
            (Some(id), Some(idx)) => idx.backlinks(id).unwrap_or_default(),
            _ => vec![],
        }
    }

    /// Open the linked note, creating it (titled `title`) if it doesn't exist.
    pub fn follow_link(&mut self, title: String, window: &mut Window, cx: &mut Context<Self>) {
        let existing = self
            .index
            .as_ref()
            .and_then(|i| i.resolve_title(&title).ok().flatten());
        match existing {
            Some(id) => self.open_note(id, window, cx),
            None => {
                let dir = self.vault.path.clone();
                if let Ok(note) = silo_vault::create_note(&dir, &title) {
                    let id = note.id;
                    if let Ok(v) = silo_vault::walk_vault(&dir) {
                        self.vault = v;
                    }
                    if let Some(idx) = &self.index {
                        let _ = idx.upsert_note(&note);
                        let _ = idx.resolve_links();
                    }
                    self.open_note(id, window, cx);
                }
            }
        }
    }

    /// Re-walk the vault from disk and rebuild the index (used after CRUD ops).
    fn refresh_vault(&mut self) {
        let root = self.vault.path.clone();
        if let Ok(v) = silo_vault::walk_vault(&root) {
            self.vault = v;
        }
        self.index = silo_index::Index::open_or_build(&root, &self.vault).ok();
    }

    /// Create an "Untitled" note in `dir`, reindex, and open it.
    pub fn new_note_in(&mut self, dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        match silo_vault::create_note(&dir, "Untitled") {
            Ok(note) => {
                let id = note.id;
                self.refresh_vault();
                self.open_note(id, window, cx);
            }
            Err(e) => eprintln!("new note failed: {e}"),
        }
    }

    fn new_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dir = self.vault.path.clone();
        self.new_note_in(dir, window, cx);
    }

    /// Expand/collapse a tree node.
    pub fn toggle_collapsed(&mut self, id: NoteId, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
        cx.notify();
    }

    /// Soft-delete a note and everything under it: its `.md` file and its child
    /// folder both go to `.silo/trash`. Clears the editor if the open note was
    /// inside the deleted subtree.
    pub fn delete_node(
        &mut self,
        note_path: PathBuf,
        children_dir: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let root = self.vault.path.clone();
        let deleting_open = self
            .selected_note()
            .map(|n| n.path == note_path || n.path.starts_with(&children_dir))
            .unwrap_or(false);
        // Trash the children folder first, then the note file. Missing paths are fine.
        if children_dir.exists() {
            if let Err(e) = silo_vault::trash(&root, &children_dir) {
                eprintln!("delete failed for {}: {e}", children_dir.display());
            }
        }
        if note_path.exists() {
            if let Err(e) = silo_vault::trash(&root, &note_path) {
                eprintln!("delete failed for {}: {e}", note_path.display());
            }
        }
        if deleting_open {
            self.selected = None;
            self.saved_text = None;
            if let Some(ed) = self.editor.clone() {
                ed.update(cx, |e, cx| e.set_content("", cx));
            }
        }
        self.refresh_vault();
        cx.notify();
    }

    // --- palette actions ----------------------------------------------------

    pub fn toggle_palette(
        &mut self,
        _: &TogglePalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.palette.open = !self.palette.open;
        self.palette.query.clear();
        self.palette.selected = 0;
        if self.palette.open {
            if let Some(h) = &self.focus_handle {
                window.focus(h, cx);
            }
        } else if let Some(ed) = self.editor.clone() {
            cx.focus_view(&ed, window);
        }
        cx.notify();
    }

    pub fn palette_up(&mut self, _: &PaletteUp, _w: &mut Window, cx: &mut Context<Self>) {
        self.palette.selected = self.palette.selected.saturating_sub(1);
        cx.notify();
    }

    pub fn palette_down(&mut self, _: &PaletteDown, _w: &mut Window, cx: &mut Context<Self>) {
        self.palette.selected += 1;
        cx.notify();
    }

    pub fn palette_close(&mut self, _: &PaletteClose, window: &mut Window, cx: &mut Context<Self>) {
        self.palette.open = false;
        if let Some(ed) = self.editor.clone() {
            cx.focus_view(&ed, window);
        }
        cx.notify();
    }

    pub fn palette_confirm(
        &mut self,
        _: &PaletteConfirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let hits = self.search(&self.palette.query, LIMIT);
        let total = hits.len() + COMMANDS.len();
        self.palette.open = false;
        if total > 0 {
            let sel = self.palette.selected.min(total - 1);
            if sel < hits.len() {
                self.open_note(hits[sel].id, window, cx);
            } else if COMMANDS[sel - hits.len()] == "New note" {
                self.new_note(window, cx);
            }
        }
        cx.notify();
    }
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
                    let _ = idx.resolve_links();
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
    pub fn selected_note(&self) -> Option<&Note> {
        let id = self.selected?;
        self.vault.find(id).and_then(|n| n.note.as_ref())
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

    fn node(n: Note, children: Vec<Notebook>) -> Notebook {
        Notebook {
            name: n.title.clone(),
            path: n.path.with_extension(""),
            note: Some(n),
            is_virtual: false,
            children,
        }
    }

    #[test]
    fn flat_notes_collects_across_children() {
        let a = node(note("A"), vec![node(note("B"), vec![])]);
        let root = Notebook {
            name: "root".into(),
            path: ".".into(),
            note: None,
            is_virtual: false,
            children: vec![a],
        };
        let st = AppState {
            vault: root,
            selected: None,
            theme: Theme::light(),
            view: View::Note,
            editor: None,
            save_task: None,
            _save_sub: None,
            config: AppConfig::default(),
            config_path: PathBuf::from("/tmp/silo-test-config.json"),
            last_self_write: None,
            saved_text: None,
            index: None,
            palette: PaletteState::default(),
            focus_handle: None,
            animating: false,
            nav_seq: 0,
            nav_anim: NavAnim::Rise,
            trips: travel::initial_trips(),
            trip: 0,
            trip_day: 0,
            travel_mode: TravelMode::Schedule,
            slot_min: 30,
            sched_x: 0.0,
            sched_drag: None,
            next_event_id: 1,
            maps_key: None,
            map_inflight: HashSet::new(),
            collapsed: HashSet::new(),
            today: NaiveDate::from_ymd_opt(2026, 8, 2).unwrap(),
            planner_day: NaiveDate::from_ymd_opt(2026, 8, 2).unwrap(),
            tasks: Vec::new(),
            next_task_id: 1,
            prompt_open: false,
            prompt_title: String::new(),
            prompt_text: String::new(),
            prompt_date: None,
        };
        let titles: Vec<_> = st
            .vault
            .every_note()
            .iter()
            .map(|n| n.title.clone())
            .collect();
        assert!(titles.contains(&"A".to_string()) && titles.contains(&"B".to_string()));
    }

    #[test]
    fn selected_note_resolves_by_id() {
        let n = note("A");
        let id = n.id;
        let root = Notebook {
            name: "root".into(),
            path: ".".into(),
            note: None,
            is_virtual: false,
            children: vec![node(n, vec![])],
        };
        let st = AppState {
            vault: root,
            selected: Some(id),
            theme: Theme::light(),
            view: View::Note,
            editor: None,
            save_task: None,
            _save_sub: None,
            config: AppConfig::default(),
            config_path: PathBuf::from("/tmp/silo-test-config.json"),
            last_self_write: None,
            saved_text: None,
            index: None,
            palette: PaletteState::default(),
            focus_handle: None,
            animating: false,
            nav_seq: 0,
            nav_anim: NavAnim::Rise,
            trips: travel::initial_trips(),
            trip: 0,
            trip_day: 0,
            travel_mode: TravelMode::Schedule,
            slot_min: 30,
            sched_x: 0.0,
            sched_drag: None,
            next_event_id: 1,
            maps_key: None,
            map_inflight: HashSet::new(),
            collapsed: HashSet::new(),
            today: NaiveDate::from_ymd_opt(2026, 8, 2).unwrap(),
            planner_day: NaiveDate::from_ymd_opt(2026, 8, 2).unwrap(),
            tasks: Vec::new(),
            next_task_id: 1,
            prompt_open: false,
            prompt_title: String::new(),
            prompt_text: String::new(),
            prompt_date: None,
        };
        assert_eq!(st.selected_note().unwrap().title, "A");
    }
}
