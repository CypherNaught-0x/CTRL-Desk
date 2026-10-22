//! Time spent sitting and standing, and when to suggest switching.
//!
//! The desk's height decides the posture. Time is only counted while the desk
//! is connected and stationary; a long gap (desk offline, computer asleep)
//! starts a new streak.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::config::{Config, FavIcon, Favourite, Reminders};

/// Longer gaps between ticks (sleep, a stalled event loop) aren't counted.
const MAX_STEP: Duration = Duration::from_secs(10);
/// Being away this long starts a new streak.
const BREAK: Duration = Duration::from_secs(5 * 60);
const KEEP_DAYS: usize = 90;
const SAVE_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posture {
    Sitting,
    Standing,
}

impl Posture {
    pub fn other(self) -> Self {
        match self {
            Posture::Sitting => Posture::Standing,
            Posture::Standing => Posture::Sitting,
        }
    }
}

/// Seconds spent in each posture on one day.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Day {
    pub sitting_s: f64,
    pub standing_s: f64,
}

impl Day {
    fn add(&mut self, p: Posture, secs: f64) {
        match p {
            Posture::Sitting => self.sitting_s += secs,
            Posture::Standing => self.standing_s += secs,
        }
    }
}

/// Daily totals, stored as JSON next to the config.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    pub days: BTreeMap<NaiveDate, Day>,
}

impl History {
    fn path() -> Option<PathBuf> {
        Config::sibling("activity")
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn day(&self, date: NaiveDate) -> Day {
        self.days.get(&date).copied().unwrap_or_default()
    }

    fn add(&mut self, date: NaiveDate, p: Posture, secs: f64) {
        self.days.entry(date).or_default().add(p, secs);
        while self.days.len() > KEEP_DAYS {
            self.days.pop_first();
        }
    }

    /// A plausible past week for demo mode and screenshots.
    pub fn sample(today: NaiveDate) -> Self {
        const MINUTES: [(f64, f64); 7] = [
            (318.0, 96.0),
            (284.0, 131.0),
            (342.0, 64.0),
            (251.0, 148.0),
            (296.0, 112.0),
            (305.0, 122.0),
            (142.0, 58.0),
        ];
        let days = MINUTES
            .iter()
            .enumerate()
            .map(|(i, &(sit, stand))| {
                (
                    today - chrono::Days::new(6 - i as u64),
                    Day {
                        sitting_s: sit * 60.0,
                        standing_s: stand * 60.0,
                    },
                )
            })
            .collect();
        Self { days }
    }
}

struct Streak {
    posture: Posture,
    length: Duration,
    last_seen: Instant,
    /// A reminder for this streak was answered.
    reminded: bool,
}

pub struct Tracker {
    pub history: History,
    streak: Option<Streak>,
    last_tick: Option<Instant>,
    snoozed_until: Option<Instant>,
    /// Whether to write the history to disk.
    persist: bool,
    dirty: bool,
    last_save: Option<Instant>,
}

impl Tracker {
    pub fn new(history: History, persist: bool) -> Self {
        Self {
            history,
            streak: None,
            last_tick: None,
            snoozed_until: None,
            persist,
            dirty: false,
            last_save: None,
        }
    }

    /// Call regularly. `posture` is `None` while the desk is offline or moving.
    pub fn tick(&mut self, now: Instant, today: NaiveDate, posture: Option<Posture>) {
        let step = self
            .last_tick
            .replace(now)
            .map(|t| now.saturating_duration_since(t))
            .filter(|d| *d <= MAX_STEP);
        let Some(p) = posture else { return };
        match &mut self.streak {
            Some(s) if s.posture == p && now.saturating_duration_since(s.last_seen) < BREAK => {
                if let Some(step) = step {
                    s.length += step;
                    self.history.add(today, p, step.as_secs_f64());
                    self.dirty = true;
                }
                s.last_seen = now;
            }
            _ => {
                self.streak = Some(Streak {
                    posture: p,
                    length: Duration::ZERO,
                    last_seen: now,
                    reminded: false,
                });
                self.snoozed_until = None;
            }
        }
        if self.persist
            && self.dirty
            && self
                .last_save
                .is_none_or(|t| now.saturating_duration_since(t) >= SAVE_EVERY)
        {
            self.save(now);
        }
    }

    pub fn save(&mut self, now: Instant) {
        if self.persist && self.dirty {
            self.history.save();
            self.dirty = false;
        }
        self.last_save = Some(now);
    }

    fn live_streak(&self, now: Instant) -> Option<&Streak> {
        self.streak
            .as_ref()
            .filter(|s| now.saturating_duration_since(s.last_seen) <= MAX_STEP)
    }

    /// The current posture and how long it has lasted, while the desk is online.
    pub fn current(&self, now: Instant) -> Option<(Posture, Duration)> {
        self.live_streak(now).map(|s| (s.posture, s.length))
    }

    /// The posture held for too long, if a reminder should be shown now.
    pub fn due(&self, now: Instant, r: &Reminders) -> Option<Posture> {
        let s = self.live_streak(now)?;
        if s.reminded || self.snoozed_until.is_some_and(|t| now < t) {
            return None;
        }
        let (on, minutes) = match s.posture {
            Posture::Sitting => (r.stand_up, r.after_sitting_min),
            Posture::Standing => (r.sit_down, r.after_standing_min),
        };
        (on && s.length >= Duration::from_secs(u64::from(minutes) * 60)).then_some(s.posture)
    }

    pub fn snooze(&mut self, now: Instant, minutes: u32) {
        self.snoozed_until = Some(now + Duration::from_secs(u64::from(minutes.max(1)) * 60));
    }

    /// No more reminders until the posture changes.
    pub fn dismiss(&mut self) {
        if let Some(s) = &mut self.streak {
            s.reminded = true;
        }
    }
}

/// The height at which the desk counts as standing: halfway between the
/// sitting and standing favourites, or the middle of the range.
pub fn standing_from_mm(cfg: &Config, base_mm: f32, max_mm: f32) -> f32 {
    cfg.standing_from_mm.unwrap_or_else(|| {
        let by_icon = |icon| {
            cfg.favourites
                .iter()
                .find(|f| f.icon == icon)
                .map(|f| f.height_mm)
        };
        match (by_icon(FavIcon::Sitting), by_icon(FavIcon::Standing)) {
            (Some(sit), Some(stand)) if stand > sit => (sit + stand) / 2.0,
            _ => (base_mm + max_mm) / 2.0,
        }
    })
}

pub fn classify(height_mm: f32, standing_from_mm: f32) -> Posture {
    if height_mm >= standing_from_mm {
        Posture::Standing
    } else {
        Posture::Sitting
    }
}

/// The favourite to move to for `posture`: one with the matching icon, else
/// the first one on the right side of the threshold.
pub fn target(cfg: &Config, posture: Posture, standing_from_mm: f32) -> Option<&Favourite> {
    let icon = match posture {
        Posture::Sitting => FavIcon::Sitting,
        Posture::Standing => FavIcon::Standing,
    };
    cfg.favourites
        .iter()
        .find(|f| f.icon == icon && classify(f.height_mm, standing_from_mm) == posture)
        .or_else(|| {
            cfg.favourites
                .iter()
                .find(|f| classify(f.height_mm, standing_from_mm) == posture)
        })
}

/// "2 h 05 min", "32 min" or "0 min".
pub fn format_duration(secs: f64) -> String {
    let minutes = (secs / 60.0).floor() as u64;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, m) => format!("{h} h {m:02} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 22).unwrap()
    }

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    /// Ticks once a second for `n` seconds.
    fn run(t: &mut Tracker, start: Instant, n: u64, p: Option<Posture>) -> Instant {
        for i in 0..=n {
            t.tick(start + secs(i), date(), p);
        }
        start + secs(n)
    }

    #[test]
    fn counts_time_per_posture() {
        let mut t = Tracker::new(History::default(), false);
        let now = run(&mut t, Instant::now(), 120, Some(Posture::Sitting));
        let now = run(&mut t, now, 30, Some(Posture::Standing));
        let day = t.history.day(date());
        assert_eq!(day.sitting_s, 120.0);
        assert_eq!(day.standing_s, 30.0);
        assert_eq!(t.current(now), Some((Posture::Standing, secs(30))));
    }

    #[test]
    fn ignores_gaps_and_motion() {
        let mut t = Tracker::new(History::default(), false);
        let now = run(&mut t, Instant::now(), 60, Some(Posture::Sitting));
        // Moving for a bit, then a stall longer than a step.
        let now = run(&mut t, now, 5, None);
        let now = now + secs(60);
        let now = run(&mut t, now, 60, Some(Posture::Sitting));
        assert_eq!(t.history.day(date()).sitting_s, 120.0);
        assert_eq!(t.current(now), Some((Posture::Sitting, secs(120))));
    }

    #[test]
    fn long_break_starts_a_new_streak() {
        let mut t = Tracker::new(History::default(), false);
        let now = run(&mut t, Instant::now(), 60, Some(Posture::Sitting));
        assert_eq!(t.current(now + secs(30)), None);
        let now = run(&mut t, now + BREAK, 10, Some(Posture::Sitting));
        assert_eq!(t.current(now), Some((Posture::Sitting, secs(10))));
        assert_eq!(t.history.day(date()).sitting_s, 70.0);
    }

    #[test]
    fn reminders_snooze_and_dismiss() {
        let r = Reminders {
            stand_up: true,
            after_sitting_min: 1,
            ..Reminders::default()
        };
        let mut t = Tracker::new(History::default(), false);
        let now = run(&mut t, Instant::now(), 59, Some(Posture::Sitting));
        assert_eq!(t.due(now, &r), None);
        let now = run(&mut t, now, 1, Some(Posture::Sitting));
        assert_eq!(t.due(now, &r), Some(Posture::Sitting));

        t.snooze(now, 1);
        let now = run(&mut t, now, 59, Some(Posture::Sitting));
        assert_eq!(t.due(now, &r), None);
        let now = run(&mut t, now, 1, Some(Posture::Sitting));
        assert_eq!(t.due(now, &r), Some(Posture::Sitting));

        t.dismiss();
        let now = run(&mut t, now, 600, Some(Posture::Sitting));
        assert_eq!(t.due(now, &r), None);

        // Standing reminders are off; standing up resets the streak.
        let now = run(&mut t, now, 600, Some(Posture::Standing));
        assert_eq!(t.due(now, &r), None);
        let now = run(&mut t, now, 60, Some(Posture::Sitting));
        assert_eq!(t.due(now, &r), Some(Posture::Sitting));
    }

    #[test]
    fn threshold_and_targets() {
        let mut cfg = Config::default();
        // Default favourites: sitting 72 cm, standing 110 cm, focus 98 cm.
        assert_eq!(standing_from_mm(&cfg, 620.0, 1270.0), 910.0);
        assert_eq!(classify(980.0, 910.0), Posture::Standing);
        assert_eq!(
            target(&cfg, Posture::Standing, 910.0).unwrap().name,
            "Standing"
        );
        assert_eq!(
            target(&cfg, Posture::Sitting, 910.0).unwrap().name,
            "Sitting"
        );

        cfg.favourites.retain(|f| f.icon == FavIcon::Laptop);
        assert_eq!(standing_from_mm(&cfg, 620.0, 1270.0), 945.0);
        assert_eq!(
            target(&cfg, Posture::Standing, 945.0).unwrap().name,
            "Focus"
        );
        assert!(target(&cfg, Posture::Sitting, 945.0).is_none());

        cfg.standing_from_mm = Some(1000.0);
        assert_eq!(standing_from_mm(&cfg, 620.0, 1270.0), 1000.0);
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(59.0), "0 min");
        assert_eq!(format_duration(32.0 * 60.0), "32 min");
        assert_eq!(format_duration(125.0 * 60.0 + 30.0), "2 h 05 min");
    }

    #[test]
    fn history_round_trips() {
        let h = History::sample(date());
        let json = serde_json::to_string(&h).unwrap();
        assert!(json.contains("\"2026-10-22\""));
        let back: History = serde_json::from_str(&json).unwrap();
        assert_eq!(back.days, h.days);
    }
}
