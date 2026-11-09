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
/// nothing to cast). In a fight, a melee class never sits, and a caster only
/// once its mana (`mana_percent`) is too low for any damage spell (at or
/// below both `DpsManaMin` and `NukeManaMin`): between casts it stays up.
pub fn sit(config: &Config, state: &State, idle: bool, fighting: bool, melee: bool, mana_percent: i32) -> bool {
    let spent = mana_percent <= i32::from(config.dps_mana_min.min(config.nuke_mana_min));
    state.medding && idle && state.mode != Mode::Off && !(fighting && (melee || !spent))
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
        let config = Config::default();
        let medding = State { medding: true, ..State::new(&Config { mode: Mode::Assist, ..Config::default() }) };
        let sit = |state: &State, idle, fighting, melee| sit(&config, state, idle, fighting, melee, 30);
        assert!(sit(&medding, true, false, true), "melee sit out of a fight");
        assert!(sit(&medding, true, true, false), "casters sit in one, out of mana for any spell");
        assert!(!sit(&medding, true, true, true));
        assert!(!sit(&medding, false, false, false), "not with something to do");
        assert!(!sit(&State { medding: false, ..medding.clone() }, true, false, false));
        assert!(!sit(&State { mode: Mode::Off, ..medding }, true, false, false));
    }

    #[test]
    fn casters_stay_up_in_a_fight_while_they_can_still_cast() {
        let config = Config::default();
        let medding = State { medding: true, ..State::new(&Config { mode: Mode::Assist, ..Config::default() }) };
        assert!(!super::sit(&config, &medding, true, true, false, 41), "above DpsManaMin=40: a debuff may still go on");
        assert!(super::sit(&config, &medding, true, true, false, 40));
        let nukes_low = Config { nuke_mana_min: 20, ..Config::default() };
        assert!(!super::sit(&nukes_low, &medding, true, true, false, 30), "above NukeManaMin=20: it can still nuke");
        assert!(super::sit(&config, &medding, true, false, false, 79), "out of a fight, MedAt alone decides");
    }
}
