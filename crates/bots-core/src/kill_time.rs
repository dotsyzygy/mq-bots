//! How soon the mob being fought dies at the rate its HP is going down, so a
//! DoT isn't started on a mob that dies before it pays off.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How far back HP readings count: long enough to smooth out bursts, short
/// enough to follow the group's damage as it changes.
const WINDOW: Duration = Duration::from_secs(8);
/// How much of the window it takes to trust the rate.
const MIN_SPAN: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Default)]
pub struct KillTime {
    /// The mob read, by spawn ID.
    mob: Option<u32>,
    /// Its HP%, and when.
    readings: VecDeque<(Instant, i32)>,
}

impl KillTime {
    /// Takes a reading of a mob's HP%; one of another mob starts over.
    pub fn see(&mut self, mob: u32, hp_percent: i32, at: Instant) {
        if self.mob != Some(mob) {
            self.mob = Some(mob);
            self.readings.clear();
        }
        self.readings.push_back((at, hp_percent));
        while self.readings.front().is_some_and(|(then, _)| at.duration_since(*then) > WINDOW) {
            self.readings.pop_front();
        }
    }

    /// How soon `mob` dies at the rate its HP went down over the last few
    /// seconds; `None` without enough readings of it, or if its HP isn't
    /// going down.
    pub fn dies_in(&self, mob: u32) -> Option<Duration> {
        if self.mob != Some(mob) {
            return None;
        }
        let (first, from) = *self.readings.front()?;
        let (last, to) = *self.readings.back()?;
        let span = last.duration_since(first);
        if span < MIN_SPAN || to >= from {
            return None;
        }
        let per_second = f64::from(from - to) / span.as_secs_f64();
        Some(Duration::from_secs_f64(f64::from(to.max(0)) / per_second))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(start: Instant, seconds: f64) -> Instant {
        start + Duration::from_secs_f64(seconds)
    }

    #[test]
    fn dies_at_the_rate_hp_goes_down() {
        let start = Instant::now();
        let mut kill = KillTime::default();
        kill.see(7, 100, start);
        kill.see(7, 90, seconds(start, 2.0));
        assert_eq!(kill.dies_in(7), None, "too soon to tell");
        kill.see(7, 80, seconds(start, 4.0));
        assert_eq!(kill.dies_in(7), Some(Duration::from_secs(16)), "5% a second from 80%");
        assert_eq!(kill.dies_in(8), None, "another mob");
    }

    #[test]
    fn follows_the_last_few_seconds() {
        let start = Instant::now();
        let mut kill = KillTime::default();
        kill.see(7, 100, start);
        kill.see(7, 99, seconds(start, 10.0));
        kill.see(7, 90, seconds(start, 14.0));
        // The reading at 0s is gone: 9% in 4s.
        let left = kill.dies_in(7).unwrap();
        assert!((39.0..=41.0).contains(&left.as_secs_f64()), "{left:?}");
    }

    #[test]
    fn no_time_without_hp_going_down() {
        let start = Instant::now();
        let mut kill = KillTime::default();
        kill.see(7, 90, start);
        kill.see(7, 90, seconds(start, 5.0));
        assert_eq!(kill.dies_in(7), None, "holding");
        kill.see(7, 95, seconds(start, 6.0));
        assert_eq!(kill.dies_in(7), None, "healing");
    }

    #[test]
    fn another_mob_starts_over() {
        let start = Instant::now();
        let mut kill = KillTime::default();
        kill.see(7, 100, start);
        kill.see(7, 50, seconds(start, 5.0));
        kill.see(8, 100, seconds(start, 6.0));
        assert_eq!(kill.dies_in(7), None);
        assert_eq!(kill.dies_in(8), None, "one reading");
    }
}
