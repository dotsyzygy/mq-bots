//! The bot's own pet, for any class that has one: taunt as `PetTaunt` says,
//! and sent in on the mob the bot fights.

use crate::Config;

/// Your pet as the bot sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pet {
    /// The spawn it's attacking, if any.
    pub target: Option<u32>,
    /// Whether its taunt is on; `None` while the pet window isn't there.
    pub taunt: Option<bool>,
}

/// A command for your pet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetOrder {
    /// Attack your target, which is the mob.
    Attack,
    /// Stop attacking and come back.
    BackOff,
    Taunt(bool),
}

/// What to tell the pet now, if anything: its taunt set as `PetTaunt` says
/// first, then, against `mob` (the one the bot fights, and its target), to
/// attack it.
pub fn order(config: &Config, pet: &Pet, mob: Option<u32>) -> Option<PetOrder> {
    if pet.taunt.is_some_and(|taunt| taunt != config.pet_taunt) {
        return Some(PetOrder::Taunt(config.pet_taunt));
    }
    mob.filter(|mob| pet.target != Some(*mob)).map(|_| PetOrder::Attack)
}

/// What to tell the pet when the bot stops fighting (the mob died, or
/// `/bots backoff`): to come back, if it's still on something.
pub fn on_disengage(pet: &Pet) -> Option<PetOrder> {
    pet.target.map(|_| PetOrder::BackOff)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOB: u32 = 50;

    #[test]
    fn turns_taunt_off_unless_pet_taunt_is_on() {
        let config = Config::default();
        let taunting = Pet { target: None, taunt: Some(true) };
        assert_eq!(order(&config, &taunting, Some(MOB)), Some(PetOrder::Taunt(false)), "the paladin tanks; taunt goes first");
        assert_eq!(order(&Config { pet_taunt: true, ..config.clone() }, &taunting, None), None);
        let not_taunting = Pet { taunt: Some(false), ..taunting };
        assert_eq!(order(&Config { pet_taunt: true, ..config.clone() }, &not_taunting, None), Some(PetOrder::Taunt(true)));
        assert_eq!(order(&config, &Pet { taunt: None, ..taunting }, None), None, "no pet window, nothing known");
    }

    #[test]
    fn sends_the_pet_at_the_mob_until_its_on_it() {
        let config = Config::default();
        let idle = Pet { target: None, taunt: Some(false) };
        assert_eq!(order(&config, &idle, Some(MOB)), Some(PetOrder::Attack));
        assert_eq!(order(&config, &Pet { target: Some(7), ..idle }, Some(MOB)), Some(PetOrder::Attack), "on something else");
        assert_eq!(order(&config, &Pet { target: Some(MOB), ..idle }, Some(MOB)), None, "on it");
        assert_eq!(order(&config, &Pet { target: Some(7), ..idle }, None), None, "out of a fight, it's left alone");
    }

    #[test]
    fn backs_off_when_the_bot_stops_if_its_on_something() {
        assert_eq!(on_disengage(&Pet { target: Some(MOB), taunt: None }), Some(PetOrder::BackOff));
        assert_eq!(on_disengage(&Pet::default()), None);
    }
}
