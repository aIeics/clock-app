//! Reminders: create, edit, delete, repeat, save and fire on time.

mod schedule;
mod view;

pub use view::RemindersView;

use chrono::{DateTime, Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::storage::Store;

pub const FILE: &str = "reminders.json";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Repeat {
    #[default]
    Once,
    Daily,
    Weekly,
}

impl Repeat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Once => "Once",
            Self::Daily => "Daily",
            Self::Weekly => "Weekly",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reminder {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub note: String,
    /// Next local time this reminder is due to fire.
    pub when: DateTime<Local>,
    #[serde(default)]
    pub repeat: Repeat,
}

/// A reminder that became due. `missed` means it was more than a minute late
/// when we noticed it, usually because the app was closed at the time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fired {
    pub title: String,
    pub missed: bool,
}

/// All reminders, stored as a JSON array. `#[serde(transparent)]` keeps the file
/// a plain list that is easy to read and edit by hand.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Reminders {
    items: Vec<Reminder>,
}

impl Reminders {
    pub fn load(store: &Store) -> Self {
        store.load_or_default(FILE)
    }

    pub fn save(&self, store: &Store) {
        if let Err(err) = store.save(FILE, self) {
            eprintln!("clock-app: could not save {}: {err}", FILE);
        }
    }

    pub fn items(&self) -> &[Reminder] {
        &self.items
    }

    pub fn on(&self, date: NaiveDate) -> impl Iterator<Item = &Reminder> {
        self.items
            .iter()
            .filter(move |reminder| reminder.when.date_naive() == date)
    }

    pub fn add(
        &mut self,
        title: String,
        note: String,
        when: DateTime<Local>,
        repeat: Repeat,
    ) -> u64 {
        let id = self.next_id();
        self.items.push(Reminder {
            id,
            title,
            note,
            when,
            repeat,
        });
        id
    }

    pub fn update(
        &mut self,
        id: u64,
        title: String,
        note: String,
        when: DateTime<Local>,
        repeat: Repeat,
    ) -> bool {
        let Some(reminder) = self.items.iter_mut().find(|reminder| reminder.id == id) else {
            return false;
        };
        reminder.title = title;
        reminder.note = note;
        reminder.when = when;
        reminder.repeat = repeat;
        true
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|reminder| reminder.id != id);
        self.items.len() != before
    }

    /// The earliest time anything is due, used to schedule the next check.
    pub fn next_due(&self) -> Option<DateTime<Local>> {
        self.items.iter().map(|reminder| reminder.when).min()
    }

    /// Fires everything due at `now`. One-off reminders are removed; repeating
    /// ones jump to their next occurrence after `now`, including reminders that
    /// were missed while the app was closed.
    pub fn fire_due(&mut self, now: DateTime<Local>) -> Vec<Fired> {
        let mut fired = Vec::new();

        for reminder in &mut self.items {
            if reminder.when > now {
                continue;
            }

            fired.push(Fired {
                title: reminder.title.clone(),
                missed: now.signed_duration_since(reminder.when) > Duration::minutes(1),
            });

            if reminder.repeat != Repeat::Once {
                reminder.when = schedule::next_occurrence(reminder.when, reminder.repeat, now);
            }
        }

        self.items
            .retain(|reminder| reminder.repeat != Repeat::Once || reminder.when > now);
        fired
    }

    fn next_id(&self) -> u64 {
        self.items
            .iter()
            .map(|reminder| reminder.id + 1)
            .max()
            .unwrap_or(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{LocalResult, NaiveDate, TimeZone};

    fn at(day: u32, hour: u32, minute: u32) -> DateTime<Local> {
        let naive = NaiveDate::from_ymd_opt(2026, 11, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap();
        match Local.from_local_datetime(&naive) {
            LocalResult::Single(time) => time,
            other => panic!("test time is not unambiguous here: {other:?}"),
        }
    }

    #[test]
    fn add_update_remove_and_next_id() {
        let mut reminders = Reminders::default();
        let id = reminders.add("One".into(), String::new(), at(25, 9, 0), Repeat::Once);
        assert_eq!(id, 1);
        assert_eq!(
            reminders.add("Two".into(), String::new(), at(26, 9, 0), Repeat::Daily),
            2
        );

        assert!(reminders.update(
            id,
            "Changed".into(),
            "note".into(),
            at(27, 9, 0),
            Repeat::Weekly
        ));
        let changed = reminders.items().iter().find(|item| item.id == id).unwrap();
        assert_eq!(changed.title, "Changed");
        assert_eq!(changed.repeat, Repeat::Weekly);

        assert!(reminders.remove(id));
        assert!(!reminders.remove(id));
        assert_eq!(reminders.items().len(), 1);
    }

    #[test]
    fn fires_a_due_one_off_reminder_once_and_removes_it() {
        let mut reminders = Reminders::default();
        reminders.add("Now".into(), String::new(), at(24, 12, 0), Repeat::Once);
        reminders.add("Later".into(), String::new(), at(25, 12, 0), Repeat::Once);

        let fired = reminders.fire_due(at(24, 12, 1));
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].title, "Now");
        assert!(!fired[0].missed);
        assert_eq!(reminders.items().len(), 1);
        assert!(reminders.fire_due(at(24, 12, 2)).is_empty());
    }

    #[test]
    fn marks_reminders_that_were_missed_while_closed() {
        let mut reminders = Reminders::default();
        reminders.add("Missed".into(), String::new(), at(24, 9, 0), Repeat::Once);

        let fired = reminders.fire_due(at(24, 12, 0));
        assert_eq!(fired.len(), 1);
        assert!(fired[0].missed);
    }

    #[test]
    fn a_repeating_reminder_moves_to_its_next_slot() {
        let mut reminders = Reminders::default();
        reminders.add("Daily".into(), String::new(), at(23, 9, 0), Repeat::Daily);

        let fired = reminders.fire_due(at(24, 12, 0));
        assert_eq!(fired.len(), 1);
        assert_eq!(reminders.items().len(), 1);
        assert_eq!(
            reminders.items()[0]
                .when
                .format("%Y-%m-%d %H:%M")
                .to_string(),
            "2026-11-25 09:00"
        );
    }

    #[test]
    fn next_due_is_the_earliest_time() {
        let mut reminders = Reminders::default();
        assert_eq!(reminders.next_due(), None);
        reminders.add("Later".into(), String::new(), at(26, 9, 0), Repeat::Once);
        reminders.add("Sooner".into(), String::new(), at(25, 9, 0), Repeat::Once);
        assert_eq!(reminders.next_due(), Some(at(25, 9, 0)));
    }

    #[test]
    fn on_filters_by_local_date() {
        let mut reminders = Reminders::default();
        reminders.add("Today".into(), String::new(), at(25, 9, 0), Repeat::Once);
        reminders.add("Tomorrow".into(), String::new(), at(26, 9, 0), Repeat::Once);

        let titles: Vec<_> = reminders
            .on(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap())
            .map(|reminder| reminder.title.clone())
            .collect();
        assert_eq!(titles, ["Today"]);
    }

    #[test]
    fn saves_and_loads_from_disk() {
        let dir = std::env::temp_dir().join(format!("clock-app-reminders-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::at(dir);

        let mut reminders = Reminders::default();
        reminders.add(
            "Saved".into(),
            "a note".into(),
            at(25, 9, 0),
            Repeat::Weekly,
        );
        reminders.save(&store);

        let loaded = Reminders::load(&store);
        assert_eq!(loaded, reminders);
    }
}
