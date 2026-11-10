//! What each class does in a fight, against the game as a [`Game`] so it's
//! tested without one.

use std::time::Duration;

use crate::{Config, Mob, State};

mod caster;
mod magician;
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
    /// Stops the bot's cast, for a heal.
    StopCast,
}

/// A group member in the zone and alive, as a class sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub id: u32,
    pub hp_percent: i32,
    /// Holds the `MainTank` role, or is the Main Assist if no one does.
    pub tank: bool,
    /// Auto attack is on.
    pub fighting: bool,
    /// The game's class ID (1 for warrior...).
    pub class: u8,
    /// Its pet's spawn ID, if it has one in the zone.
    pub pet: Option<u32>,
}

/// Whether a class ID is one that fights in melee: WAR, PAL, RNG, SHD, MNK,
/// BRD, ROG, BST, BER.
pub fn melee_class(class: u8) -> bool {
    matches!(class, 1 | 3 | 4 | 5 | 7 | 8 | 9 | 15 | 16)
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
    /// A spell's kind, as the game's spell data says (its subcategory: Slow,
    /// Poison...); `None` if it isn't in your book or has none. Two spells
    /// of one kind are ranks of one line: two slows don't both go on a mob.
    fn spell_kind(&self, name: &str) -> Option<i32>;
    /// The spell gems you have (8 to 12 in RoF2, with level and AAs).
    fn gem_count(&self) -> usize;
    /// The gem a spell is memorized in, numbered from 1.
    fn spell_gem(&self, name: &str) -> Option<usize>;
    /// A memorized spell the game would cast now: its gem ready, and the
    /// mana for it.
    fn spell_ready(&self, name: &str) -> bool;
    /// A cast or memorizing under way, the bot's or one by hand.
    fn casting(&self) -> bool;
    /// The spell of the bot's cast under way, by the INI's name.
    fn bot_casting(&self) -> Option<String>;
    /// The group's members in the zone and alive, you included; just you
    /// out of a group.
    fn members(&self) -> Vec<Member>;
    /// Time left on a spell, by the INI's name, on a member or a member's
    /// pet, by spawn ID; `None` if it isn't on it. One whose buffs aren't
    /// seen has every one, so nothing is cast on it over and over.
    fn buff_left(&self, spawn: u32, spell: &str) -> Option<Duration>;
    /// A spawn (member, pet or mob) close enough for a memorized spell, by
    /// the INI's name, to reach (its range, or a group spell's AE range),
    /// and in line of sight.
    fn in_range(&self, spell: &str, spawn: u32) -> bool;
    /// Your mana as a percent of your most.
    fn mana_percent(&self) -> i32;
    /// The mob being fought, its HP as the MA sees it when it can.
    fn mob(&self) -> Option<Mob>;
    /// How soon the mob being fought dies at the rate its HP is going down;
    /// `None` until that's known, or if it isn't going down.
    fn mob_dies_in(&self) -> Option<Duration>;
    /// Whether the mob being fought has a spell, by the INI's name, from
    /// you; `None` while its buffs aren't seen (not targeted, or the
    /// server hasn't sent them yet).
    fn on_mob(&self, spell: &str) -> Option<bool>;
    /// Times in a row the mob being fought resisted a spell, by the INI's
    /// name (`u8::MAX` if it's immune).
    fn resists(&self, spell: &str) -> u8;
}

pub trait Class {
    /// The INI sections it reads, for saying which others it doesn't.
    fn archetypes(&self) -> &'static [&'static str];

    /// The archetypes it casts spells from, most important first, to keep
    /// memorized (see [`plan_gems`]).
    fn spells(&self) -> &'static [Spells] {
        &[]
    }

    /// Whether it sticks to the mob it engages and turns on attack.
    fn melee(&self) -> bool {
        true
    }

    /// What comes before fighting or resting, in a fight or out (heals),
    /// if anything.
    fn urgent(&mut self, _config: &Config, _state: &State, _game: &dyn Game) -> Option<Action> {
        None
    }

    /// The next thing to do against [`State::engaged`], if anything.
    fn fight(&mut self, config: &Config, state: &State, game: &dyn Game) -> Option<Action>;
    /// The next thing to do out of combat (buffs, auras), if anything.
    /// While [`State::medding`], only what can't wait (a pet and its gear,
    /// long buffs), not short buffs a fight would cast anyway.
    fn rest(&mut self, config: &Config, state: &State, game: &dyn Game) -> Option<Action>;
}

/// The class's module, by the game's name for it ("Monk"); `None` for a
/// class without one yet.
pub fn for_class(name: &str) -> Option<Box<dyn Class>> {
    match name.to_ascii_lowercase().as_str() {
        "monk" => Some(Box::new(monk::Monk)),
        "shaman" => Some(Box::new(shaman::Shaman)),
        "magician" => Some(Box::new(magician::Magician)),
        _ => None,
    }
}

/// An archetype a class casts from, and how many of its spells it uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spells {
    /// The best one you know (a heal).
    Best(&'static str),
    /// The best one you know of each kind ([`Game::spell_kind`]), in list
    /// order: one slow, one malo, one cripple...
    EachKind(&'static str),
}

impl Spells {
    pub fn archetype(self) -> &'static str {
        match self {
            Spells::Best(archetype) | Spells::EachKind(archetype) => archetype,
        }
    }
}

/// The spells of `spells` the class uses, by the INI's names, in list order:
/// known, not in `skip` (given up on, or used by another archetype), and for
/// [`Spells::EachKind`], not of a kind an earlier one is.
pub fn spells_used<'a>(config: &'a Config, spells: Spells, game: &dyn Game, skip: &[&str]) -> Vec<&'a str> {
    let mut used: Vec<&str> = Vec::new();
    let mut kinds = Vec::new();
    for name in config.archetype(spells.archetype()) {
        if skip.contains(&name.as_str()) || used.contains(&name.as_str()) || !game.spell_known(name) {
            continue;
        }
        match (spells, game.spell_kind(name)) {
            (Spells::Best(_), _) => return vec![name],
            (Spells::EachKind(_), Some(kind)) if kinds.contains(&kind) => continue,
            (Spells::EachKind(_), kind) => kinds.extend(kind),
        }
        used.push(name);
    }
    used
}

/// The best spell of an archetype that's memorized: the one [`plan_gems`]
/// keeps, or the next best if that one wouldn't memorize.
pub fn best_memorized<'a>(config: &'a Config, archetype: &str, game: &dyn Game) -> Option<&'a str> {
    config.archetype(archetype).iter().find(|name| game.spell_gem(name).is_some()).map(String::as_str)
}

/// The best memorized spell of each kind ([`Game::spell_kind`]) in an
/// archetype, in list order: the ones [`Spells::EachKind`] keeps, or the next
/// best of a kind if one wouldn't memorize.
pub fn memorized_each_kind<'a>(config: &'a Config, archetype: &str, game: &dyn Game) -> Vec<&'a str> {
    let mut used = Vec::new();
    let mut kinds = Vec::new();
    for name in config.archetype(archetype).iter().filter(|name| game.spell_gem(name).is_some()) {
        match game.spell_kind(name) {
            Some(kind) if kinds.contains(&kind) => continue,
            kind => kinds.extend(kind),
        }
        used.push(name.as_str());
    }
    used
}

/// Which spells to keep memorized, and the next one to memorize.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GemPlan {
    /// Memorize this next, if anything.
    pub next: Option<Action>,
    /// The spells the gems have no room for, most important first, as
    /// "spell (archetype)".
    pub left_out: Vec<String>,
    /// How many spells the class uses, room or not.
    pub wanted: usize,
}

/// Keeps the spells a class uses ([`spells_used`] of each of `spells`)
/// memorized, the most important first when there are more than gems. A
/// spell already in a gem stays there; the others go into the gems holding
/// nothing wanted, lowest first. Spells in `given_up` (that wouldn't
/// memorize) are taken as unknown.
pub fn plan_gems(config: &Config, spells: &[Spells], game: &dyn Game, given_up: &[String]) -> GemPlan {
    let mut wanted: Vec<(&str, &str)> = Vec::new();
    for &archetype in spells {
        let skip: Vec<&str> = given_up.iter().map(String::as_str).chain(wanted.iter().map(|(_, spell)| *spell)).collect();
        let used = spells_used(config, archetype, game, &skip);
        wanted.extend(used.into_iter().map(|spell| (archetype.archetype(), spell)));
    }
    let room = game.gem_count().min(wanted.len());
    let kept = &wanted[..room];
    let taken: Vec<usize> = kept.iter().filter_map(|(_, spell)| game.spell_gem(spell)).collect();
    let mut free = (1..=game.gem_count()).filter(|gem| !taken.contains(gem));
    let missing = kept.iter().find(|(_, spell)| game.spell_gem(spell).is_none());
    GemPlan {
        next: missing.zip(free.next()).map(|((_, spell), gem)| Action::Memorize { gem, spell: (*spell).to_owned() }),
        left_out: wanted[room..].iter().map(|(archetype, spell)| format!("{spell} ({archetype})")).collect(),
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
    /// Spells' kinds; others have none.
    pub kinds: Vec<(&'static str, i32)>,
    /// Memorized spells by gem.
    pub gems: Vec<(usize, &'static str)>,
    pub gem_count: usize,
    pub casting: bool,
    pub bot_casting: Option<&'static str>,
    pub members: Vec<Member>,
    /// Spells on members and pets, by spawn ID, with 12 seconds left.
    pub member_buffs: Vec<(u32, &'static str)>,
    /// Spells on members and pets with this little left.
    pub fading: Vec<(u32, &'static str)>,
    /// Members out of every spell's range.
    pub out_of_range: Vec<u32>,
    pub mana_percent: i32,
    pub mob: Option<Mob>,
    pub mob_dies_in: Option<Duration>,
    /// Spells from you on the mob; `None` while its buffs aren't seen.
    pub on_mob: Option<Vec<&'static str>>,
    /// Resists in a row on the mob.
    pub resists: Vec<(&'static str, u8)>,
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
    fn spell_kind(&self, name: &str) -> Option<i32> {
        self.kinds.iter().find(|(spell, _)| *spell == name).map(|(_, kind)| *kind)
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
        self.casting || self.bot_casting.is_some()
    }
    fn bot_casting(&self) -> Option<String> {
        self.bot_casting.map(str::to_owned)
    }
    fn members(&self) -> Vec<Member> {
        self.members.clone()
    }
    fn buff_left(&self, spawn: u32, spell: &str) -> Option<Duration> {
        if self.fading.contains(&(spawn, spell)) {
            return Some(Duration::from_secs(3));
        }
        self.member_buffs.contains(&(spawn, spell)).then_some(Duration::from_secs(12))
    }
    fn in_range(&self, _spell: &str, spawn: u32) -> bool {
        !self.out_of_range.contains(&spawn)
    }
    fn mana_percent(&self) -> i32 {
        self.mana_percent
    }
    fn mob(&self) -> Option<Mob> {
        self.mob
    }
    fn mob_dies_in(&self) -> Option<Duration> {
        self.mob_dies_in
    }
    fn on_mob(&self, spell: &str) -> Option<bool> {
        self.on_mob.as_ref().map(|spells| spells.contains(&spell))
    }
    fn resists(&self, spell: &str) -> u8 {
        self.resists.iter().find(|(name, _)| *name == spell).map_or(0, |(_, count)| *count)
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
        assert!(for_class("Magician").is_some_and(|magician| !magician.melee()));
        assert!(for_class("Paladin").is_none());
    }

    const SLOW: i32 = 88;
    const MALO: i32 = 81;
    const POISON: i32 = 1;
    const DISEASE: i32 = 2;

    fn spells_config() -> Config {
        let numbered = |names: &[&str]| names.iter().enumerate().map(|(i, name)| ((i + 1).to_string(), name.to_string())).collect();
        let sections = vec![
            ("heal".to_owned(), numbered(&["Superior Healing", "Greater Healing", "Healing"])),
            ("debuff".to_owned(), numbered(&["Turgur's Insects", "Togor's Insects", "Malosi", "Malaise", "Cripple"])),
            ("dot".to_owned(), numbered(&["Envenomed Bolt", "Envenomed Breath", "Tainted Breath", "Sicken"])),
            ("nuke".to_owned(), numbered(&["Frost Strike"])),
        ];
        Config::read(&crate::IniFile { name: "Shaman.ini".into(), sections }).0
    }

    const SPELLS: &[Spells] = &[Spells::Best("heal"), Spells::EachKind("debuff"), Spells::EachKind("dot"), Spells::Best("nuke")];

    /// A level 40 shaman, without Turgur's Insects, Envenomed Bolt or Cripple.
    fn shaman() -> FakeGame {
        FakeGame {
            book: vec!["Healing", "Greater Healing", "Togor's Insects", "Malosi", "Malaise", "Envenomed Breath", "Tainted Breath", "Sicken", "Frost Strike"],
            kinds: vec![("Togor's Insects", SLOW), ("Malosi", MALO), ("Malaise", MALO), ("Envenomed Breath", POISON), ("Tainted Breath", POISON), ("Sicken", DISEASE)],
            gem_count: 8,
            ..FakeGame::default()
        }
    }

    fn memorize(gem: usize, spell: &str) -> Option<Action> {
        Some(Action::Memorize { gem, spell: spell.into() })
    }

    #[test]
    fn uses_the_best_known_spell_or_the_best_of_each_kind() {
        let config = spells_config();
        let game = shaman();
        assert_eq!(spells_used(&config, Spells::Best("heal"), &game, &[]), ["Greater Healing"]);
        assert_eq!(spells_used(&config, Spells::EachKind("debuff"), &game, &[]), ["Togor's Insects", "Malosi"], "one slow, one malo");
        assert_eq!(spells_used(&config, Spells::EachKind("dot"), &game, &[]), ["Envenomed Breath", "Sicken"], "one poison, one disease");
        assert_eq!(spells_used(&config, Spells::EachKind("dot"), &game, &["Envenomed Breath"]), ["Tainted Breath", "Sicken"]);
        let game = FakeGame { book: [game.book.as_slice(), &["Cripple"]].concat(), ..game };
        assert_eq!(spells_used(&config, Spells::EachKind("debuff"), &game, &[]), ["Togor's Insects", "Malosi", "Cripple"], "a spell with no kind is its own");
        assert!(spells_used(&config, Spells::Best("buff.melee"), &game, &[]).is_empty());
    }

    #[test]
    fn memorizes_what_the_class_uses_in_priority_order() {
        let config = spells_config();
        let mut game = shaman();
        let plan = |game: &FakeGame| plan_gems(&config, SPELLS, game, &[]);
        assert_eq!(plan(&game), GemPlan { next: memorize(1, "Greater Healing"), left_out: vec![], wanted: 6 });
        game.gems = vec![(1, "Greater Healing")];
        assert_eq!(plan(&game).next, memorize(2, "Togor's Insects"));
        game.gems = vec![(1, "Greater Healing"), (2, "Togor's Insects"), (3, "Malosi"), (4, "Envenomed Breath"), (5, "Sicken")];
        assert_eq!(plan(&game).next, memorize(6, "Frost Strike"));
        game.gems.push((6, "Frost Strike"));
        assert_eq!(plan(&game).next, None, "all in");
    }

    #[test]
    fn casts_the_best_memorized_spell_of_each_kind() {
        let config = spells_config();
        let game = FakeGame { gems: vec![(1, "Malaise"), (2, "Togor's Insects"), (3, "Malosi"), (4, "Healing")], ..shaman() };
        assert_eq!(memorized_each_kind(&config, "debuff", &game), ["Togor's Insects", "Malosi"], "one malo, the better one");
        let game = FakeGame { gems: vec![(1, "Malaise"), (2, "Sicken")], ..shaman() };
        assert_eq!(memorized_each_kind(&config, "debuff", &game), ["Malaise"], "the next best of a kind when the best isn't memorized");
    }

    #[test]
    fn leaves_memorized_spells_where_they_are() {
        let config = spells_config();
        let game = FakeGame { gems: vec![(1, "Gate"), (2, "Frost Strike"), (3, "Healing"), (4, "Greater Healing")], ..shaman() };
        assert_eq!(plan_gems(&config, SPELLS, &game, &[]).next, memorize(1, "Togor's Insects"), "gem 1 holds nothing wanted; 2 and 4 stay");
        let game = FakeGame { book: [game.book.as_slice(), &["Superior Healing"]].concat(), gems: vec![(1, "Greater Healing"), (2, "Togor's Insects")], ..game };
        assert_eq!(plan_gems(&config, SPELLS, &game, &[]).next, memorize(1, "Superior Healing"), "a better spell replaces the one it outranks");
    }

    #[test]
    fn says_which_spells_the_gems_have_no_room_for() {
        let config = spells_config();
        let game = FakeGame { gem_count: 4, gems: vec![(1, "Greater Healing"), (2, "Togor's Insects"), (3, "Malosi"), (4, "Envenomed Breath")], ..shaman() };
        assert_eq!(plan_gems(&config, SPELLS, &game, &[]), GemPlan { next: None, left_out: vec!["Sicken (dot)".into(), "Frost Strike (nuke)".into()], wanted: 6 });
    }

    #[test]
    fn a_spell_that_wouldnt_memorize_gives_way_to_the_next_best() {
        let config = spells_config();
        assert_eq!(plan_gems(&config, SPELLS, &shaman(), &["Greater Healing".into()]).next, memorize(1, "Healing"));
    }

    #[test]
    fn one_spell_in_two_archetypes_takes_one_gem() {
        let config = Config::read(&crate::IniFile { name: "Shaman.ini".into(), sections: vec![("heal".into(), vec![("1".into(), "Healing".into())]), ("heal.hot".into(), vec![("1".into(), "Healing".into())])] }).0;
        let game = FakeGame { book: vec!["Healing"], gem_count: 8, gems: vec![(1, "Healing")], ..FakeGame::default() };
        assert_eq!(plan_gems(&config, &[Spells::Best("heal"), Spells::Best("heal.hot")], &game, &[]), GemPlan { next: None, left_out: vec![], wanted: 1 });
    }
}
