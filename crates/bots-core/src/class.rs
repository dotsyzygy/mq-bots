//! What each class does in a fight, against the game as a [`Game`] so it's
//! tested without one.

use std::time::Duration;

use crate::{Config, State};

mod monk;
mod shaman;

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
    /// A memorized spell, on the spawn with this ID (targeting it, then going
    /// back to the target before), or on the current target if `None`.
    Cast { spell: String, target: Option<u32> },
    /// A spell from your book into a gem, numbered from 1 as `/cast` does.
    Memorize { gem: usize, spell: String },
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
    /// A spell in your book that you're high enough level to cast.
    fn spell_known(&self, name: &str) -> bool;
    /// The spell gems you have (8 to 12 in RoF2, with level and AAs).
    fn gem_count(&self) -> usize;
    /// The gem a spell is memorized in, numbered from 1.
    fn spell_gem(&self, name: &str) -> Option<usize>;
    /// A memorized spell the game would cast now: its gem ready, and the
    /// mana for it.
    fn spell_ready(&self, name: &str) -> bool;
    /// A cast or memorizing under way, the bot's or one by hand.
    fn casting(&self) -> bool;
}

pub trait Class {
    /// The INI sections it reads, for saying which others it doesn't; `#`
    /// stands for a number (`dot.#` for `dot.1`, `dot.2`...).
    fn archetypes(&self) -> &'static [&'static str];

    /// The archetypes it casts spells from, most important first, to keep
    /// memorized (see [`plan_gems`]). `#` as in [`archetypes`](Class::archetypes).
    fn spell_lines(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether it sticks to the mob it engages and turns on attack.
    fn melee(&self) -> bool {
        true
    }

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
        "shaman" => Some(Box::new(shaman::Shaman)),
        _ => None,
    }
}

/// Which spells to keep memorized, and the next one to memorize.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GemPlan {
    /// Memorize this next, if anything.
    pub next: Option<Action>,
    /// The archetypes with a spell you know that the gems have no room for,
    /// most important first.
    pub left_out: Vec<String>,
    /// The archetypes with a spell you know, room or not.
    pub wanted: usize,
}

/// Keeps the best spell you know of each of `lines` (a class's
/// [`spell_lines`](Class::spell_lines)) memorized, the most important first
/// when there are more than gems. A spell already in a gem stays there; the
/// others go into the gems holding nothing wanted, lowest first. Spells in
/// `given_up` (that wouldn't memorize) are taken as unknown.
pub fn plan_gems(config: &Config, lines: &[&str], game: &dyn Game, given_up: &[String]) -> GemPlan {
    let mut wanted: Vec<(&str, &str)> = Vec::new();
    for archetype in lines.iter().flat_map(|line| config.archetypes_like(line)) {
        let usable = |name: &&String| !wanted.iter().any(|(_, spell)| spell == name) && !given_up.contains(name) && game.spell_known(name);
        let best = config.archetype(archetype).iter().find(usable);
        wanted.extend(best.map(|spell| (archetype, spell.as_str())));
    }
    let room = game.gem_count().min(wanted.len());
    let kept = &wanted[..room];
    let taken: Vec<usize> = kept.iter().filter_map(|(_, spell)| game.spell_gem(spell)).collect();
    let mut free = (1..=game.gem_count()).filter(|gem| !taken.contains(gem));
    let missing = kept.iter().find(|(_, spell)| game.spell_gem(spell).is_none());
    GemPlan {
        next: missing.zip(free.next()).map(|((_, spell), gem)| Action::Memorize { gem, spell: (*spell).to_owned() }),
        left_out: wanted[room..].iter().map(|(archetype, _)| (*archetype).to_owned()).collect(),
        wanted: wanted.len(),
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

/// A [`Game`] made up by tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct FakeGame {
    /// Abilities, discs and memorized spells that are ready.
    pub ready: Vec<&'static str>,
    /// Discs you have.
    pub known: Vec<&'static str>,
    pub auras: Vec<&'static str>,
    pub in_range: bool,
    pub buffs: Vec<(&'static str, Duration)>,
    /// Spells in your book you can cast.
    pub book: Vec<&'static str>,
    /// Memorized spells by gem.
    pub gems: Vec<(usize, &'static str)>,
    pub gem_count: usize,
    pub casting: bool,
}

#[cfg(test)]
impl Game for FakeGame {
    fn ability_ready(&self, name: &str) -> bool {
        self.ready.contains(&name)
    }
    fn disc_ready(&self, name: &str) -> bool {
        self.ready.contains(&name)
    }
    fn disc_known(&self, name: &str) -> bool {
        self.known.contains(&name)
    }
    fn aura_active(&self, name: &str) -> bool {
        self.auras.contains(&name)
    }
    fn disc_buff_left(&self, name: &str) -> Option<Duration> {
        self.buffs.iter().find(|(buff, _)| *buff == name).map(|(_, left)| *left)
    }
    fn in_melee_range(&self) -> bool {
        self.in_range
    }
    fn spell_known(&self, name: &str) -> bool {
        self.book.contains(&name)
    }
    fn gem_count(&self) -> usize {
        self.gem_count
    }
    fn spell_gem(&self, name: &str) -> Option<usize> {
        self.gems.iter().find(|(_, spell)| *spell == name).map(|(gem, _)| *gem)
    }
    fn spell_ready(&self, name: &str) -> bool {
        self.spell_gem(name).is_some() && self.ready.contains(&name)
    }
    fn casting(&self) -> bool {
        self.casting
    }
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
        assert!(for_class("MONK").is_some_and(|monk| monk.melee()));
        assert!(for_class("Shaman").is_some_and(|shaman| !shaman.melee()));
        assert!(for_class("Paladin").is_none());
    }

    fn spells_config() -> Config {
        let numbered = |names: &[&str]| names.iter().enumerate().map(|(i, name)| ((i + 1).to_string(), name.to_string())).collect();
        let sections = vec![
            ("heal".to_owned(), numbered(&["Superior Healing", "Greater Healing", "Healing"])),
            ("dot.2".to_owned(), numbered(&["Tainted Breath"])),
            ("dot.10".to_owned(), numbered(&["Envenomed Breath"])),
            ("dot.1".to_owned(), numbered(&["Sicken"])),
            ("nuke".to_owned(), numbered(&["Frost Strike"])),
        ];
        Config::from_layers(&[crate::Layer { name: "Shaman.ini".into(), sections }]).0
    }

    const LINES: &[&str] = &["heal", "dot.#", "nuke"];
    const BOOK: &[&str] = &["Healing", "Greater Healing", "Sicken", "Tainted Breath", "Envenomed Breath", "Frost Strike"];

    fn memorize(gem: usize, spell: &str) -> Option<Action> {
        Some(Action::Memorize { gem, spell: spell.into() })
    }

    #[test]
    fn memorizes_each_lines_best_known_spell_in_priority_order() {
        let config = spells_config();
        let mut game = FakeGame { book: BOOK.to_vec(), gem_count: 8, ..FakeGame::default() };
        let plan = |game: &FakeGame| plan_gems(&config, LINES, game, &[]);
        assert_eq!(plan(&game), GemPlan { next: memorize(1, "Greater Healing"), left_out: vec![], wanted: 5 });
        game.gems = vec![(1, "Greater Healing")];
        assert_eq!(plan(&game).next, memorize(2, "Sicken"), "dot.1, dot.2, dot.10 by number");
        game.gems = vec![(1, "Greater Healing"), (2, "Sicken"), (3, "Tainted Breath")];
        assert_eq!(plan(&game).next, memorize(4, "Envenomed Breath"));
        game.gems.extend([(4, "Envenomed Breath"), (5, "Frost Strike")]);
        assert_eq!(plan(&game).next, None, "all in");
    }

    #[test]
    fn leaves_memorized_spells_where_they_are() {
        let config = spells_config();
        let game = FakeGame { book: BOOK.to_vec(), gem_count: 8, gems: vec![(1, "Gate"), (2, "Frost Strike"), (3, "Healing"), (4, "Greater Healing")], ..FakeGame::default() };
        assert_eq!(plan_gems(&config, LINES, &game, &[]).next, memorize(1, "Sicken"), "gem 1 holds nothing wanted; 2 and 4 stay");
        let game = FakeGame { book: [BOOK, &["Superior Healing"]].concat(), gems: vec![(1, "Greater Healing"), (2, "Sicken")], ..game };
        assert_eq!(plan_gems(&config, LINES, &game, &[]).next, memorize(1, "Superior Healing"), "a better spell replaces the one it outranks");
    }

    #[test]
    fn says_which_lines_the_gems_have_no_room_for() {
        let config = spells_config();
        let game = FakeGame { book: BOOK.to_vec(), gem_count: 3, gems: vec![(1, "Greater Healing"), (2, "Sicken"), (3, "Tainted Breath")], ..FakeGame::default() };
        assert_eq!(plan_gems(&config, LINES, &game, &[]), GemPlan { next: None, left_out: vec!["dot.10".into(), "nuke".into()], wanted: 5 });
    }

    #[test]
    fn a_spell_that_wouldnt_memorize_gives_way_to_the_next_best() {
        let config = spells_config();
        let game = FakeGame { book: BOOK.to_vec(), gem_count: 8, ..FakeGame::default() };
        assert_eq!(plan_gems(&config, LINES, &game, &["Greater Healing".into()]).next, memorize(1, "Healing"));
    }

    #[test]
    fn one_spell_in_two_lines_takes_one_gem() {
        let config = Config::from_layers(&[crate::Layer { name: "Shaman.ini".into(), sections: vec![("heal".into(), vec![("1".into(), "Healing".into())]), ("heal.hot".into(), vec![("1".into(), "Healing".into())])] }]).0;
        let game = FakeGame { book: vec!["Healing"], gem_count: 8, gems: vec![(1, "Healing")], ..FakeGame::default() };
        assert_eq!(plan_gems(&config, &["heal", "heal.hot"], &game, &[]), GemPlan { next: None, left_out: vec![], wanted: 1 });
    }
}
