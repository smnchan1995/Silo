//! The planner's task model (shared by the Today / Week / Month views) and the
//! date helpers those views need. Tasks are owned, editable runtime state in
//! `AppState` (in-memory, like the travel demo) so the views support CRUD.

use chrono::{Datelike, Days, NaiveDate};

/// A dated to-do item shown on the planner.
#[derive(Clone)]
pub struct Task {
    pub id: u64,
    pub date: NaiveDate,
    pub text: String,
    pub done: bool,
}

/// Seed a handful of tasks around `today` so the views aren't empty on first run.
pub fn initial_tasks(today: NaiveDate) -> Vec<Task> {
    let mut id = 0u64;
    let mut mk = |offset: i64, text: &str, done: bool| {
        id += 1;
        let date = if offset >= 0 {
            today
                .checked_add_days(Days::new(offset as u64))
                .unwrap_or(today)
        } else {
            today
                .checked_sub_days(Days::new((-offset) as u64))
                .unwrap_or(today)
        };
        Task {
            id,
            date,
            text: text.to_string(),
            done,
        }
    };
    vec![
        mk(0, "send draft to Ana", false),
        mk(0, "book dentist", false),
        mk(0, "morning run — 5k", true),
        mk(1, "review Kyoto itinerary", false),
        mk(2, "weekly review", false),
        mk(4, "ship draft", false),
        mk(-1, "pay rent", true),
    ]
}

/// The seven days of the week containing `date`, Monday first.
pub fn week_of(date: NaiveDate) -> [NaiveDate; 7] {
    let back = date.weekday().num_days_from_monday() as u64;
    let monday = date.checked_sub_days(Days::new(back)).unwrap_or(date);
    std::array::from_fn(|i| {
        monday
            .checked_add_days(Days::new(i as u64))
            .unwrap_or(monday)
    })
}

/// The calendar grid (weeks × 7) covering `date`'s month; `None` = padding cell
/// from an adjacent month.
pub fn month_grid(date: NaiveDate) -> Vec<[Option<NaiveDate>; 7]> {
    let first = NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap();
    let lead = first.weekday().num_days_from_monday() as usize;
    let mut weeks = Vec::new();
    let mut cur = 0usize; // cells filled so far
    let mut week = [None; 7];
    let mut day = first;
    // leading blanks
    cur += lead;
    while day.month() == date.month() {
        week[cur % 7] = Some(day);
        cur += 1;
        if cur.is_multiple_of(7) {
            weeks.push(week);
            week = [None; 7];
        }
        day = match day.checked_add_days(Days::new(1)) {
            Some(d) => d,
            None => break,
        };
    }
    if !cur.is_multiple_of(7) {
        weeks.push(week);
    }
    weeks
}

/// "August 2026" for a month header.
pub fn month_title(date: NaiveDate) -> String {
    const M: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    format!("{} {}", M[(date.month0()) as usize], date.year())
}

/// "Mon, Aug 2" for a day header.
pub fn day_title(date: NaiveDate) -> String {
    const WD: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const M: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{}, {} {}",
        WD[date.weekday().num_days_from_monday() as usize],
        M[date.month0() as usize],
        date.day()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn week_of_starts_monday_and_contains_date() {
        let w = week_of(d(2026, 8, 6)); // a Thursday
        assert_eq!(w[0].weekday().num_days_from_monday(), 0); // Monday
        assert!(w.contains(&d(2026, 8, 6)));
        assert_eq!(w[6], w[0].checked_add_days(Days::new(6)).unwrap());
    }

    #[test]
    fn month_grid_covers_every_day_once() {
        let grid = month_grid(d(2026, 8, 15));
        let days: Vec<NaiveDate> = grid.iter().flatten().flatten().copied().collect();
        assert_eq!(days.len(), 31); // August
        assert!(days.contains(&d(2026, 8, 1)));
        assert!(days.contains(&d(2026, 8, 31)));
    }

    #[test]
    fn initial_tasks_are_dated_and_unique() {
        let tasks = initial_tasks(d(2026, 8, 2));
        assert!(!tasks.is_empty());
        let today = tasks.iter().filter(|t| t.date == d(2026, 8, 2)).count();
        assert_eq!(today, 3);
    }
}
