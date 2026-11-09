//! Sitting to get mana back.

use crate::{Config, Mode, State};

/// Whether to be medding, from what it was: from below `MedAt` mana until
/// full. `mana_percent` is `None` for a character without mana.
pub fn medding(config: &Config, was: bool, mana_percent: Option<i32>) -> bool {
    let Some(mana) = mana_percent else { return false };
    config.med_at > 0 && mana < 100 && (was || mana < i32::from(config.med_at))
}

/// Whether to sit now, while medding and with nothing to do this pulse
/// (`idle`: not moving, casting, memorizing, feigning or stunned, and
/// nothing to cast). A melee class doesn't sit in a fight.
pub fn sit(state: &State, idle: bool, fighting: bool, melee: bool) -> bool {
    state.medding && idle && state.mode != Mode::Off && !(fighting && melee)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meds_from_below_med_at_until_full() {
        let config = Config::default();
        assert!(!medding(&config, false, Some(80)), "80 isn't below MedAt=80");
        assert!(medding(&config, false, Some(79)));
        assert!(medding(&config, true, Some(95)), "once started, until full");
        assert!(!medding(&config, true, Some(100)));
        assert!(!medding(&config, true, None), "no mana, no medding");
        assert!(!medding(&Config { med_at: 0, ..config }, true, Some(10)), "MedAt=0 never meds");
    }

    #[test]
    fn sits_when_idle_but_not_to_melee() {
        let medding = State { medding: true, ..State::new(&Config { mode: Mode::Assist, ..Config::default() }) };
        assert!(sit(&medding, true, false, true), "melee sit out of a fight");
        assert!(sit(&medding, true, true, false), "casters sit in one too");
        assert!(!sit(&medding, true, true, true));
        assert!(!sit(&medding, false, false, false), "not with something to do");
        assert!(!sit(&State { medding: false, ..medding.clone() }, true, false, false));
        assert!(!sit(&State { mode: Mode::Off, ..medding }, true, false, false));
    }
}
