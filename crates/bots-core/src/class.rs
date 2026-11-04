//! What each class does in a fight, against the game as a [`Game`] so it's
//! tested without one.

use crate::{Config, State};

mod monk;

/// What a class asks the plugin to do, by the name the INI gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// An ability on the abilities page (`/doability`).
    Ability(String),
    /// A combat ability (discipline).
    Disc(String),
}

/// The game as a class sees it, with names as the INI gives them.
pub trait Game {
    /// An ability you have that's ready now.
    fn ability_ready(&self, name: &str) -> bool;
    /// A combat ability you have that the game would take now: ready, with
    /// the endurance for it, and no discipline running if it's one.
    fn disc_ready(&self, name: &str) -> bool;
    /// Close enough to the engaged mob to hit it.
    fn in_melee_range(&self) -> bool;
}

pub trait Class {
    /// The next thing to do against [`State::engaged`], if anything.
    fn fight(&mut self, config: &Config, state: &State, game: &dyn Game) -> Option<Action>;
}

/// The class's module, by the game's name for it ("Monk"); `None` for a
/// class without one yet.
pub fn for_class(name: &str) -> Option<Box<dyn Class>> {
    match name.to_ascii_lowercase().as_str() {
        "monk" => Some(Box::new(monk::Monk)),
        _ => None,
    }
}

/// Whether an INI entry names the game's ability or spell `game`: the same,
/// ignoring case, or the same but for the game's rank ("Rk. II").
pub fn same_name(entry: &str, game: &str) -> bool {
    let unranked = game.rsplit_once(" Rk. ").map_or(game, |(name, _)| name);
    entry.eq_ignore_ascii_case(game) || entry.eq_ignore_ascii_case(unranked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_with_or_without_the_rank() {
        assert!(same_name("flying kick", "Flying Kick"));
        assert!(same_name("Ashenhand Discipline", "Ashenhand Discipline Rk. II"));
        assert!(same_name("Ashenhand Discipline Rk. II", "Ashenhand Discipline Rk. II"));
        assert!(!same_name("Ashenhand Discipline Rk. III", "Ashenhand Discipline Rk. II"));
        assert!(!same_name("Ashenhand", "Ashenhand Discipline"));
    }

    #[test]
    fn finds_classes_by_name() {
        assert!(for_class("MONK").is_some());
        assert!(for_class("Paladin").is_none());
    }
}
