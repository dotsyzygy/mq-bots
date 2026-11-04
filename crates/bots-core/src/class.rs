//! What each class does in a fight, against the game as a [`Game`] so it's
//! tested without one.

use std::time::Duration;

use crate::{Config, State};

mod monk;

/// A buff a combat ability gives you is used again only with this little
/// left on it (Fists of Wu has no reuse timer, only its song).
pub const REFRESH_BUFF_AT: Duration = Duration::from_secs(5);

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
    /// A combat ability you have, ready or not.
    fn disc_known(&self, name: &str) -> bool;
    /// One of your auras is up from the ability or spell `name`.
    fn aura_active(&self, name: &str) -> bool;
    /// Time left on the buff or song a combat ability gave you; `None` if
    /// it isn't on you or doesn't wear off.
    fn disc_buff_left(&self, name: &str) -> Option<Duration>;
    /// Close enough to the engaged mob to hit it.
    fn in_melee_range(&self) -> bool;
}

pub trait Class {
    /// The INI sections it reads, for saying which others it doesn't.
    fn archetypes(&self) -> &'static [&'static str];

    /// The next thing to do against [`State::engaged`], if anything.
    fn fight(&mut self, config: &Config, state: &State, game: &dyn Game) -> Option<Action>;
    /// The next thing to do out of combat (buffs, auras), if anything.
    fn rest(&mut self, config: &Config, state: &State, game: &dyn Game) -> Option<Action>;
}

/// The class's module, by the game's name for it ("Monk"); `None` for a
/// class without one yet.
pub fn for_class(name: &str) -> Option<Box<dyn Class>> {
    match name.to_ascii_lowercase().as_str() {
        "monk" => Some(Box::new(monk::Monk)),
        _ => None,
    }
}

/// A combat ability to use now: ready, and not still buffing you.
pub fn disc_usable(game: &dyn Game, name: &str) -> bool {
    game.disc_ready(name) && game.disc_buff_left(name).is_none_or(|left| left <= REFRESH_BUFF_AT)
}

/// The best aura in the `aura` archetype that you have, if it isn't up and
/// can be used. Only the best: a lesser one up doesn't stop it, so a new
/// aura takes over as soon as you have it.
pub fn keep_aura(config: &Config, game: &dyn Game) -> Option<Action> {
    let best = config.archetype("aura").iter().find(|name| game.disc_known(name))?;
    (!game.aura_active(best) && disc_usable(game, best)).then(|| Action::Disc(best.clone()))
}

/// Whether the aura window's name for an aura is `name` (an INI entry or a
/// spell's name). The window drops punctuation ("Masters Aura" for
/// "Master's Aura") and may add to the name, so only letters and digits
/// count, and one may start with the other.
pub fn aura_named(name: &str, aura: &str) -> bool {
    let plain = |text: &str| text.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect::<String>();
    let (name, aura) = (plain(name), plain(aura));
    !name.is_empty() && !aura.is_empty() && (aura.starts_with(&name) || name.starts_with(&aura))
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
    fn auras_match_without_punctuation_or_a_suffix() {
        assert!(aura_named("Master's Aura", "Masters Aura"));
        assert!(aura_named("Master's Aura", "Master's Aura Effect"));
        assert!(aura_named("disciple's aura", "Disciples Aura"));
        assert!(!aura_named("Master's Aura", "Disciple's Aura"));
        assert!(!aura_named("", "Masters Aura"));
    }

    #[test]
    fn finds_classes_by_name() {
        assert!(for_class("MONK").is_some());
        assert!(for_class("Paladin").is_none());
    }
}
