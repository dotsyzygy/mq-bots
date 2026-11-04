//! The monk: kicks and strikes whenever they're ready, then regular discs.

use super::{Action, Class, Game, disc_usable};
use crate::{Config, State};

pub struct Monk;

impl Class for Monk {
    fn archetypes(&self) -> &'static [&'static str] {
        // Burns, Mend and Feign Death come later, but their lists can be written now.
        &["ability.offense", "disc.regular", "disc.burn", "aa.burn", "mend", "fd"]
    }

    fn fight(&mut self, config: &Config, _state: &State, game: &dyn Game) -> Option<Action> {
        if !game.in_melee_range() {
            return None;
        }
        let ability = config.archetype("ability.offense").iter().find(|name| game.ability_ready(name));
        let disc = || config.archetype("disc.regular").iter().find(|name| disc_usable(game, name));
        ability.map(|name| Action::Ability(name.clone())).or_else(|| disc().map(|name| Action::Disc(name.clone())))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::Layer;

    struct FakeGame {
        ready: Vec<&'static str>,
        in_range: bool,
        buffs: Vec<(&'static str, Duration)>,
    }

    impl Game for FakeGame {
        fn ability_ready(&self, name: &str) -> bool {
            self.ready.contains(&name)
        }
        fn disc_ready(&self, name: &str) -> bool {
            self.ready.contains(&name)
        }
        fn disc_buff_left(&self, name: &str) -> Option<Duration> {
            self.buffs.iter().find(|(buff, _)| *buff == name).map(|(_, left)| *left)
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
        fight_buffed(ready, in_range, &[])
    }

    fn fight_buffed(ready: &[&'static str], in_range: bool, buffs: &[(&'static str, Duration)]) -> Option<Action> {
        let config = config();
        Monk.fight(&config, &State::new(&config), &FakeGame { ready: ready.to_vec(), in_range, buffs: buffs.to_vec() })
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
    fn a_disc_whose_buff_is_on_waits_until_it_is_nearly_gone() {
        let ready = ["Thunderkick", "Ashenhand Discipline"];
        let thunderkick = |left| fight_buffed(&ready, true, &[("Thunderkick", Duration::from_secs(left))]);
        assert_eq!(thunderkick(6), Some(Action::Disc("Ashenhand Discipline".into())));
        assert_eq!(thunderkick(5), Some(Action::Disc("Thunderkick".into())));
        assert_eq!(fight_buffed(&["Thunderkick"], true, &[("Thunderkick", Duration::from_secs(20))]), None);
    }

    #[test]
    fn waits_until_in_melee_range() {
        assert_eq!(fight(&["Flying Kick", "Thunderkick"], false), None);
    }
}
