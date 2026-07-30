//! The Travel view's data (two demo trips) and Google Maps URL builders.
//!
//! Unlike the other planner views, the trips are **owned, editable runtime
//! state** (held in `AppState`) so the schedule supports CRUD. Each stop is a
//! scheduled event: a place (name + real coordinates, for the map) plus a start
//! time and duration on the day's timeline.
//!
//! Silo is a native GPUI app with no embedded browser, so the day's route is
//! rendered as a **Google Static Maps** image styled to the Modernist palette
//! (fetched to a cache file, drawn by path); a button opens the interactive
//! route in the system browser.

/// A scheduled stop on a day's itinerary/timeline.
#[derive(Clone)]
pub struct Stop {
    /// Stable id (for CRUD — delete/move a specific event).
    pub id: u64,
    pub name: String,
    pub activity: String,
    /// How you get here from the previous stop (`None` for the first).
    pub commute: Option<String>,
    pub lat: f64,
    pub lng: f64,
    /// Start time, minutes from midnight.
    pub start_min: u32,
    /// Duration in minutes.
    pub dur_min: u32,
}

/// A single day of an itinerary.
#[derive(Clone)]
pub struct Day {
    pub date: String,
    pub title: String,
    pub detail: String,
    pub stops: Vec<Stop>,
}

impl Day {
    /// Stops sorted by start time (the timeline order).
    pub fn timed(&self) -> Vec<&Stop> {
        let mut v: Vec<&Stop> = self.stops.iter().collect();
        v.sort_by_key(|s| s.start_min);
        v
    }
}

/// A trip booking (flight, hotel, …) — a checklist item.
#[derive(Clone)]
pub struct Booking {
    pub label: String,
    pub done: bool,
}

/// A trip: tabs + itinerary + bookings.
#[derive(Clone)]
pub struct Trip {
    pub crumb: String,
    pub tab: String,
    pub sub: String,
    pub title: String,
    pub when: String,
    /// Appended to each stop name for the browser directions link (e.g. "Japan").
    pub country: String,
    pub days: Vec<Day>,
    pub bookings: Vec<Booking>,
}

/// A stop row for the builder: (name, activity, commute, lat, lng, duration min).
type Row = (
    &'static str,
    &'static str,
    Option<&'static str>,
    f64,
    f64,
    u32,
);

/// Build a day, auto-sequencing start times from 9:00 with a 45-min gap.
fn day(next: &mut u64, date: &str, title: &str, detail: &str, rows: &[Row]) -> Day {
    let mut t = 9 * 60;
    let stops = rows
        .iter()
        .map(|(name, activity, commute, lat, lng, dur)| {
            let s = Stop {
                id: *next,
                name: (*name).to_string(),
                activity: (*activity).to_string(),
                commute: commute.map(|c| c.to_string()),
                lat: *lat,
                lng: *lng,
                start_min: t,
                dur_min: *dur,
            };
            *next += 1;
            t += dur + 45;
            s
        })
        .collect();
    Day {
        date: date.to_string(),
        title: title.to_string(),
        detail: detail.to_string(),
        stops,
    }
}

fn booking(label: &str, done: bool) -> Booking {
    Booking {
        label: label.to_string(),
        done,
    }
}

/// The initial (demo) trips. Fresh owned data each call.
pub fn initial_trips() -> Vec<Trip> {
    let mut id: u64 = 1;
    vec![
        Trip {
            crumb: "Kyoto".into(),
            tab: "Kyoto — October".into(),
            sub: "Oct 3 – 10 · in 68 days".into(),
            title: "Kyoto — October".into(),
            when: "Oct 3 — 10 · in 68 days".into(),
            country: "Japan".into(),
            days: vec![
                day(
                    &mut id,
                    "Oct 3",
                    "Arrive — Gion",
                    "airport → hotel · evening walk",
                    &[
                        (
                            "Kyoto Station",
                            "Haruka express in from KIX",
                            None,
                            34.9858,
                            135.7588,
                            60,
                        ),
                        (
                            "Gion",
                            "check in, drop bags",
                            Some("taxi · 15 min"),
                            35.0036,
                            135.7752,
                            60,
                        ),
                        (
                            "Yasaka Shrine",
                            "evening walk down Hanamikoji",
                            Some("walk · 8 min"),
                            35.0036,
                            135.7785,
                            75,
                        ),
                    ],
                ),
                day(
                    &mut id,
                    "Oct 4",
                    "East side temples",
                    "Kiyomizu → Philosopher's Path",
                    &[
                        (
                            "Kiyomizu-dera",
                            "at opening, 6:00",
                            Some("walk · 20 min from hotel"),
                            34.9949,
                            135.7850,
                            90,
                        ),
                        (
                            "Ninenzaka",
                            "coffee, shops",
                            Some("walk · 10 min"),
                            34.9973,
                            135.7808,
                            60,
                        ),
                        (
                            "Nanzen-ji",
                            "aqueduct + zen garden",
                            Some("bus 202 · 25 min"),
                            35.0111,
                            135.7937,
                            75,
                        ),
                        (
                            "Ginkaku-ji",
                            "silver pavilion before close",
                            Some("walk · 30 min along the path"),
                            35.0270,
                            135.7982,
                            60,
                        ),
                    ],
                ),
                day(
                    &mut id,
                    "Oct 5",
                    "Arashiyama",
                    "bamboo at 7:00 · river afternoon",
                    &[
                        (
                            "Saga-Arashiyama Station",
                            "",
                            Some("JR Sagano line · 17 min"),
                            35.0189,
                            135.6787,
                            30,
                        ),
                        (
                            "Arashiyama Bamboo Grove",
                            "before the crowds",
                            Some("walk · 10 min"),
                            35.0169,
                            135.6717,
                            60,
                        ),
                        (
                            "Tenryu-ji",
                            "garden, lunch nearby",
                            Some("walk · 5 min"),
                            35.0159,
                            135.6737,
                            90,
                        ),
                        (
                            "Togetsukyo Bridge",
                            "river, afternoon light",
                            Some("walk · 8 min"),
                            35.0128,
                            135.6776,
                            45,
                        ),
                    ],
                ),
                day(
                    &mut id,
                    "Oct 6",
                    "Nara day trip",
                    "Todai-ji, deer park",
                    &[
                        (
                            "Nara Park",
                            "deer, mochi stand",
                            Some("Kintetsu limited express · 35 min"),
                            34.6851,
                            135.8430,
                            60,
                        ),
                        (
                            "Todai-ji",
                            "daibutsu hall",
                            Some("walk · 12 min"),
                            34.6889,
                            135.8398,
                            90,
                        ),
                        (
                            "Kasuga-taisha",
                            "lantern paths · back for dinner",
                            Some("walk · 15 min"),
                            34.6819,
                            135.8480,
                            75,
                        ),
                    ],
                ),
                day(
                    &mut id,
                    "Oct 7–10",
                    "Loose days",
                    "markets, gardens, one rest day",
                    &[
                        (
                            "Nishiki Market",
                            "morning graze",
                            None,
                            35.0050,
                            135.7649,
                            90,
                        ),
                        (
                            "Kyoto Gyoen",
                            "gardens from the reading list",
                            Some("subway · 12 min"),
                            35.0242,
                            135.7620,
                            120,
                        ),
                    ],
                ),
            ],
            bookings: vec![
                booking("Flight — SFO → KIX (ANA)", true),
                booking("Hotel — Gion, 7 nights", true),
                booking("JR Pass — 7 day", false),
                booking("Kaiseki dinner — Oct 5", false),
            ],
        },
        Trip {
            crumb: "Lisbon".into(),
            tab: "Lisbon — March".into(),
            sub: "Mar 14 – 18 · in planning".into(),
            title: "Lisbon — March".into(),
            when: "Mar 14 — 18 · in planning".into(),
            country: "Portugal".into(),
            days: vec![
                day(
                    &mut id,
                    "Mar 14",
                    "Arrive — Alfama",
                    "airport → hotel · first wander",
                    &[
                        (
                            "Lisbon Airport",
                            "TAP in, midday",
                            None,
                            38.7742,
                            -9.1342,
                            60,
                        ),
                        (
                            "Alfama",
                            "check in, drop bags",
                            Some("metro + walk · 35 min"),
                            38.7119,
                            -9.1298,
                            60,
                        ),
                        (
                            "Praça do Comércio",
                            "river front at dusk",
                            Some("walk · 12 min"),
                            38.7077,
                            -9.1366,
                            75,
                        ),
                    ],
                ),
                day(
                    &mut id,
                    "Mar 15",
                    "Belém morning",
                    "monastery, tower, pastéis",
                    &[
                        (
                            "Jerónimos Monastery",
                            "at opening",
                            Some("tram 15 · 25 min"),
                            38.6979,
                            -9.2065,
                            75,
                        ),
                        (
                            "Pastéis de Belém",
                            "the originals, still warm",
                            Some("walk · 3 min"),
                            38.6976,
                            -9.2032,
                            45,
                        ),
                        (
                            "Belém Tower",
                            "river walk",
                            Some("walk · 15 min"),
                            38.6916,
                            -9.2160,
                            60,
                        ),
                        (
                            "MAAT Lisbon",
                            "if energy holds",
                            Some("walk · 12 min"),
                            38.6961,
                            -9.1938,
                            60,
                        ),
                    ],
                ),
                day(
                    &mut id,
                    "Mar 16",
                    "Sintra day trip",
                    "Regaleira + Pena",
                    &[
                        (
                            "Sintra Station",
                            "",
                            Some("train from Rossio · 40 min"),
                            38.7985,
                            -9.3868,
                            30,
                        ),
                        (
                            "Quinta da Regaleira",
                            "initiation well, early",
                            Some("walk · 15 min"),
                            38.7967,
                            -9.3966,
                            90,
                        ),
                        (
                            "Pena Palace",
                            "park first, palace after lunch",
                            Some("bus 434 · 15 min"),
                            38.7876,
                            -9.3905,
                            120,
                        ),
                    ],
                ),
                day(
                    &mut id,
                    "Mar 17–18",
                    "Loose days",
                    "LX Factory, food, fado?",
                    &[
                        ("LX Factory", "bookshop + lunch", None, 38.7027, -9.1786, 90),
                        (
                            "Time Out Market Lisboa",
                            "dinner, last night",
                            Some("walk · 10 min"),
                            38.7071,
                            -9.1459,
                            90,
                        ),
                    ],
                ),
            ],
            bookings: vec![
                booking("Flight — LHR → LIS (TAP)", true),
                booking("Hotel — Alfama, 4 nights", false),
                booking("Sintra train tickets", false),
                booking("Fado night — reservation", false),
            ],
        },
    ]
}

/// Format minutes-from-midnight as "H:MM" (24h).
pub fn fmt_time(min: u32) -> String {
    format!("{}:{:02}", (min / 60) % 24, min % 60)
}

/// Percent-encode a string for use in a URL query value.
fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// A geocodable location string ("<stop>, <country>") for the browser link.
fn loc(stop: &Stop, trip: &Trip) -> String {
    format!("{}, {}", stop.name, trip.country)
}

/// Google Static Maps `style=` rules rendering the map in the Modernist palette.
fn map_styles(dark: bool) -> &'static [&'static str] {
    if dark {
        &[
            "feature:all|element:geometry|saturation:-100",
            "feature:all|element:labels.text.fill|color:0x8b8681",
            "feature:all|element:labels.text.stroke|color:0x1a1918",
            "feature:all|element:labels.icon|visibility:off",
            "feature:landscape|element:geometry|color:0x201e1d",
            "feature:water|element:geometry|color:0x141312",
            "feature:road|element:geometry.fill|color:0x37332f",
            "feature:road|element:geometry.stroke|color:0x262421",
            "feature:poi|element:geometry|color:0x262320",
            "feature:poi.park|element:geometry|color:0x232a23",
            "feature:transit|element:geometry|color:0x2a2724",
        ]
    } else {
        &[
            "feature:all|element:geometry|saturation:-100",
            "feature:all|element:labels.text.fill|color:0x8a8580",
            "feature:all|element:labels.text.stroke|color:0xf7f6f5",
            "feature:all|element:labels.icon|visibility:off",
            "feature:landscape|element:geometry|color:0xf1efee",
            "feature:water|element:geometry|color:0xdedbda",
            "feature:road|element:geometry.fill|color:0xffffff",
            "feature:road|element:geometry.stroke|color:0xe4e1e1",
            "feature:poi|element:geometry|color:0xeceae8",
            "feature:poi.park|element:geometry|color:0xe6eae1",
            "feature:transit|element:geometry|color:0xe8e5e3",
        ]
    }
}

/// Build a Modernist-styled Google Static Maps URL for a day: auto-framed to the
/// stops, numbered accent markers, accent route line. `None` if no stops.
pub fn static_map_url(stops: &[Stop], dark: bool, key: &str) -> Option<String> {
    if stops.is_empty() {
        return None;
    }
    let accent = if dark { "0xe08a70" } else { "0xec3013" };
    let mut url = String::from(
        "https://maps.googleapis.com/maps/api/staticmap?size=560x340&scale=2&maptype=roadmap&language=en",
    );
    for rule in map_styles(dark) {
        url.push_str("&style=");
        url.push_str(&rule.replace('|', "%7C"));
    }
    for (i, s) in stops.iter().enumerate() {
        url.push_str(&format!(
            "&markers=color:{}%7Clabel:{}%7C{:.5},{:.5}",
            accent,
            i + 1,
            s.lat,
            s.lng
        ));
    }
    if stops.len() > 1 {
        url.push_str(&format!("&path=color:{accent}c8%7Cweight:4"));
        for s in stops {
            url.push_str(&format!("%7C{:.5},{:.5}", s.lat, s.lng));
        }
    }
    url.push_str("&key=");
    url.push_str(&pct(key));
    Some(url)
}

/// Build a Google Maps directions URL (opens in the browser) routing through the
/// day's stops, in transit mode.
pub fn directions_url(trip: &Trip, day: &Day) -> Option<String> {
    let stops = day.timed();
    let (first, last) = (stops.first()?, stops.last()?);
    let mut url = format!(
        "https://www.google.com/maps/dir/?api=1&travelmode=transit&origin={}&destination={}",
        pct(&loc(first, trip)),
        pct(&loc(last, trip)),
    );
    if stops.len() > 2 {
        let mid: Vec<String> = stops[1..stops.len() - 1]
            .iter()
            .map(|s| pct(&loc(s, trip)))
            .collect();
        url.push_str(&format!("&waypoints={}", mid.join("%7C")));
    }
    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_trips_have_days_stops_and_unique_ids() {
        let trips = initial_trips();
        assert_eq!(trips.len(), 2);
        let mut ids = std::collections::HashSet::new();
        for t in &trips {
            assert!(!t.days.is_empty());
            assert!(!t.bookings.is_empty());
            for d in &t.days {
                assert!(!d.stops.is_empty());
                for s in &d.stops {
                    assert!(ids.insert(s.id), "duplicate event id {}", s.id);
                    assert!(s.dur_min > 0);
                }
            }
        }
    }

    #[test]
    fn day_timed_is_sorted_by_start() {
        let trips = initial_trips();
        let d = &trips[0].days[1];
        let times: Vec<u32> = d.timed().iter().map(|s| s.start_min).collect();
        let mut sorted = times.clone();
        sorted.sort_unstable();
        assert_eq!(times, sorted);
    }

    #[test]
    fn static_map_url_has_style_coords_and_key() {
        let d = &initial_trips()[0].days[1];
        let url = static_map_url(&d.stops, false, "TESTKEY").unwrap();
        assert!(url.contains("&style="));
        assert!(url.contains("markers=color:0xec3013"));
        assert!(url.contains("34.99490,135.78500")); // Kiyomizu-dera
        assert!(url.contains("key=TESTKEY"));
    }

    #[test]
    fn fmt_time_formats_hh_mm() {
        assert_eq!(fmt_time(9 * 60), "9:00");
        assert_eq!(fmt_time(13 * 60 + 5), "13:05");
    }
}
