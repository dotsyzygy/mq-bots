//! The monk: kicks and strikes whenever they're ready, then regular discs.

use super::{Action, Class, Game};
use crate::{Config, State};

pub struct Monk;

impl Class for Monk {
    fn fight(&mut self, config: &Config, _state: &State, game: &dyn Game) -> Option<Action> {
        if !game.in_melee_range() {
            return None;
        }
        let ability = config.archetype("ability.offense").iter().find(|name| game.ability_ready(name));
        let disc = || config.archetype("disc.regular").iter().find(|name| game.disc_ready(name));
        ability.map(|name| Action::Ability(name.clone())).or_else(|| disc().map(|name| Action::Disc(name.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Layer;

    struct FakeGame {
        ready: Vec<&'static str>,
        in_range: bool,
    }

    impl Game for FakeGame {
        fn ability_ready(&self, name: &str) -> bool {
            self.ready.contains(&name)
        }
        fn disc_ready(&self, name: &str) -> bool {
            self.ready.contains(&name)
        }
        fn in_melee_range(&self) -> bool {
            self.in_range
        }
    }

    fn config() -> Config {
        let numbered = |names: &[&str]| names.iter().enumerate().map(|(i, name)| ((i + 1).to_string(), name.to_string())).collect();
        let sections = vec![
            ("ability.offense".to_owned(), numbered(&["Flying Kick", "Tiger Claw"])),
            ("disc.regular".to_owned(), numbered(&["Thunderkick", "Ashenhand Discipline"])),
        ];
        Config::from_layers(&[Layer { name: "Monk.ini".into(), sections }]).0
    }

    fn fight(ready: &[&'static str], in_range: bool) -> Option<Action> {
        let config = config();
        Monk.fight(&config, &State::new(&config), &FakeGame { ready: ready.to_vec(), in_range })
    }

    #[test]
    fn uses_the_best_ready_ability_before_any_disc() {
        assert_eq!(fight(&["Tiger Claw", "Flying Kick", "Thunderkick"], true), Some(Action::Ability("Flying Kick".into())));
        assert_eq!(fight(&["Tiger Claw", "Thunderkick"], true), Some(Action::Ability("Tiger Claw".into())));
        assert_eq!(fight(&["Ashenhand Discipline", "Thunderkick"], true), Some(Action::Disc("Thunderkick".into())));
        assert_eq!(fight(&["Ashenhand Discipline"], true), Some(Action::Disc("Ashenhand Discipline".into())));
        assert_eq!(fight(&[], true), None);
    }

    #[test]
    fn waits_until_in_melee_range() {
        assert_eq!(fight(&["Flying Kick", "Thunderkick"], false), None);
    }
}
