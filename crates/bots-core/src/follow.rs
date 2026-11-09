//! Whom to follow, from what the plugin saw this pulse.

use crate::{Mode, State};

/// The spawn ID to follow: the Main Assist (`ma`, `None` when there isn't
/// one in the zone, or it's this bot), while following is on, the bot is
/// on, and it isn't fighting or `busy` (casting, memorizing or feigning:
/// moving would break those off).
pub fn follow_target(state: &State, ma: Option<u32>, busy: bool) -> Option<u32> {
    let free = state.following && state.mode != Mode::Off && state.engaged.is_none() && !busy;
    ma.filter(|_| free)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;

    fn following() -> State {
        State { following: true, ..State::new(&Config { mode: Mode::Assist, ..Config::default() }) }
    }

    #[test]
    fn follows_the_ma_when_free() {
        assert_eq!(follow_target(&following(), Some(7), false), Some(7));
        assert_eq!(follow_target(&State { mode: Mode::Manual, ..following() }, Some(7), false), Some(7));
        assert_eq!(follow_target(&State { backed_off: true, ..following() }, Some(7), false), Some(7), "backing off comes back to the MA");
    }

    #[test]
    fn doesnt_follow_when_off_fighting_busy_or_without_an_ma() {
        assert_eq!(follow_target(&State { following: false, ..following() }, Some(7), false), None);
        assert_eq!(follow_target(&State { mode: Mode::Off, ..following() }, Some(7), false), None);
        assert_eq!(follow_target(&State { engaged: Some(9), ..following() }, Some(7), false), None);
        assert_eq!(follow_target(&following(), Some(7), true), None);
        assert_eq!(follow_target(&following(), None, false), None);
    }
}
