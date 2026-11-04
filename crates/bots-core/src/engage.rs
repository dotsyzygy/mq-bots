//! Which mob to fight, from what the plugin saw this pulse.

use crate::{Config, Mode, State};

/// A spawn as engaging looks at it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mob {
    pub id: u32,
    pub is_npc: bool,
    pub hp_percent: i32,
    /// From the Main Assist, who holds the camp.
    pub distance_to_ma: f32,
}

/// What the plugin saw this pulse.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Seen {
    /// The Main Assist's target, from Posse; `None` without an MA, a fresh
    /// status from it, or a target in the zone.
    pub ma_target: Option<Mob>,
    /// [`State::engaged`] in the zone; `None` once it's gone.
    pub engaged: Option<Mob>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Nothing to fight.
    Idle,
    /// Target this mob, stick to it and attack.
    Engage(u32),
    /// Keep fighting [`State::engaged`].
    Fight,
    /// Stop fighting [`State::engaged`]: attack off, stop sticking.
    Disengage,
}

/// Also takes back a pending [`State::attack_requested`].
pub fn decide(state: &mut State, config: &Config, seen: &Seen) -> Decision {
    let requested = std::mem::take(&mut state.attack_requested);
    let fighting = state.engaged.is_some();
    if state.mode == Mode::Off || state.backed_off {
        return if fighting { Decision::Disengage } else { Decision::Idle };
    }
    let assist = state.mode == Mode::Assist;
    let switch_to = seen.ma_target.filter(|mob| {
        let wanted = if requested { mob.is_npc } else { assist && qualifies(mob, config) };
        wanted && Some(mob.id) != state.engaged
    });
    match (switch_to, seen.engaged) {
        (Some(mob), _) => Decision::Engage(mob.id),
        (None, Some(mob)) if mob.is_npc && mob.hp_percent > 0 => Decision::Fight,
        (None, _) if fighting => Decision::Disengage,
        (None, _) => Decision::Idle,
    }
}

/// An NPC worn down to assist-at, within the camp.
fn qualifies(mob: &Mob, config: &Config) -> bool {
    mob.is_npc && mob.hp_percent > 0 && mob.hp_percent <= i32::from(config.assist_at) && mob.distance_to_ma <= config.camp_radius
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mob(id: u32, hp_percent: i32, distance_to_ma: f32) -> Mob {
        Mob { id, is_npc: true, hp_percent, distance_to_ma }
    }

    fn assisting() -> State {
        State::new(&Config { mode: Mode::Assist, ..Config::default() })
    }

    #[test]
    fn assist_engages_the_mas_target_at_assist_at_within_the_camp() {
        let config = Config { assist_at: 90, camp_radius: 50.0, ..Config::default() };
        let cases = [
            (mob(7, 90, 50.0), Decision::Engage(7)),
            (mob(7, 91, 10.0), Decision::Idle),
            (mob(7, 50, 50.5), Decision::Idle),
            (mob(7, 0, 10.0), Decision::Idle),
            (Mob { is_npc: false, ..mob(7, 50, 10.0) }, Decision::Idle),
        ];
        for (target, expected) in cases {
            let seen = Seen { ma_target: Some(target), engaged: None };
            assert_eq!(decide(&mut assisting(), &config, &seen), expected, "{target:?}");
        }
        assert_eq!(decide(&mut assisting(), &config, &Seen::default()), Decision::Idle);
    }

    #[test]
    fn fights_until_the_mob_dies_or_goes_then_follows_the_ma_to_the_next() {
        let config = Config::default();
        let mut state = State { engaged: Some(7), ..assisting() };
        let seen = |ma_target, engaged| Seen { ma_target, engaged };
        assert_eq!(decide(&mut state, &config, &seen(Some(mob(7, 40, 5.0)), Some(mob(7, 40, 5.0)))), Decision::Fight);
        // The MA looked away at something not yet worn down.
        assert_eq!(decide(&mut state, &config, &seen(Some(mob(8, 100, 5.0)), Some(mob(7, 40, 5.0)))), Decision::Fight);
        assert_eq!(decide(&mut state, &config, &seen(Some(mob(8, 80, 5.0)), Some(mob(7, 40, 5.0)))), Decision::Engage(8));
        assert_eq!(decide(&mut state, &config, &seen(None, Some(mob(7, 0, 5.0)))), Decision::Disengage);
        assert_eq!(decide(&mut state, &config, &seen(None, Some(Mob { is_npc: false, ..mob(7, 40, 5.0) }))), Decision::Disengage);
        assert_eq!(decide(&mut state, &config, &seen(None, None)), Decision::Disengage);
    }

    #[test]
    fn attack_engages_any_npc_the_ma_has_once_even_in_manual() {
        let config = Config { camp_radius: 10.0, ..Config::default() };
        let mut state = State { attack_requested: true, ..State::new(&config) };
        let far_and_fresh = Seen { ma_target: Some(mob(7, 100, 200.0)), engaged: None };
        assert_eq!(decide(&mut state, &config, &far_and_fresh), Decision::Engage(7));
        assert!(!state.attack_requested);
        assert_eq!(decide(&mut state, &config, &far_and_fresh), Decision::Idle, "manual doesn't engage on its own");

        state.attack_requested = true;
        let player = Seen { ma_target: Some(Mob { is_npc: false, ..mob(7, 100, 0.0) }), engaged: None };
        assert_eq!(decide(&mut state, &config, &player), Decision::Idle);
        assert!(!state.attack_requested, "a request that couldn't be met is dropped");
    }

    #[test]
    fn off_and_backed_off_stop_fighting_and_dont_engage() {
        let config = Config::default();
        let seen = Seen { ma_target: Some(mob(7, 50, 5.0)), engaged: Some(mob(7, 50, 5.0)) };
        for state in [State { mode: Mode::Off, ..assisting() }, State { backed_off: true, ..assisting() }] {
            assert_eq!(decide(&mut State { engaged: Some(7), ..state.clone() }, &config, &seen), Decision::Disengage);
            assert_eq!(decide(&mut State { attack_requested: true, ..state }, &config, &seen), Decision::Idle);
        }
    }
}
