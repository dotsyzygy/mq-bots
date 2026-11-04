//! The shaman: stays back and casts. Heals (in a fight or out) and keeps its
//! spells memorized so far; buffs, debuffs, DoTs and nukes come in
//! milestones S4 and S5.

use super::{Action, Class, Game, Member, Spells, best_memorized};
use crate::{Config, State};

pub struct Shaman;

/// Most important first, for when the gems can't hold them all.
const SPELLS: &[Spells] = &[
    Spells::Best("heal"),
    Spells::Best("heal.hot"),
    Spells::Best("heal.group.hot"),
    Spells::Best("buff.melee"),
    Spells::EachKind("debuff"),
    Spells::EachKind("dot"),
    Spells::Best("nuke"),
];

/// The archetypes whose casts aren't stopped for an emergency heal.
const HEALS: &[&str] = &["heal", "heal.hot", "heal.group.hot"];

impl Class for Shaman {
    fn archetypes(&self) -> &'static [&'static str] {
        &["heal", "heal.hot", "heal.group.hot", "buff.melee", "debuff", "dot", "nuke"]
    }

    fn spells(&self) -> &'static [Spells] {
        SPELLS
    }

    fn melee(&self) -> bool {
        false
    }

    /// The emergency heal, then the group HoT, then the tank's HoT, each on
    /// the members its spell reaches.
    fn urgent(&mut self, config: &Config, _state: &State, game: &dyn Game) -> Option<Action> {
        let members = game.members();
        let reached = |spell: &str| members.iter().filter(|member| game.in_range(spell, member.id)).cloned().collect::<Vec<_>>();
        if let Some(heal) = best_memorized(config, "heal", game)
            && let Some(member) = most_hurt(config, &reached(heal))
        {
            if game.casting() {
                // A heal under way finishes; anything else is stopped.
                let other = game.bot_casting().is_some_and(|spell| !is_heal(config, &spell));
                return other.then_some(Action::StopCast);
            }
            if game.spell_ready(heal) {
                return Some(Action::Cast { spell: heal.to_owned(), target: Some(member.id) });
            }
        }
        if game.casting() {
            return None;
        }
        if let Some(hot) = best_memorized(config, "heal.group.hot", game) {
            let reached = reached(hot);
            let hurt = reached.iter().filter(|member| member.hp_percent < i32::from(config.group_hot_at) && game.buff_left(member.id, hot).is_none());
            if hurt.count() >= usize::from(config.group_hot_count) && game.spell_ready(hot) {
                return Some(Action::Cast { spell: hot.to_owned(), target: None });
            }
        }
        let hot = best_memorized(config, "heal.hot", game)?;
        let tank = members.iter().find(|member| member.tank && member.fighting && game.in_range(hot, member.id))?;
        (game.buff_left(tank.id, hot).is_none() && game.spell_ready(hot)).then(|| Action::Cast { spell: hot.to_owned(), target: Some(tank.id) })
    }

    fn rest(&mut self, _config: &Config, _state: &State, _game: &dyn Game) -> Option<Action> {
        None
    }

    fn fight(&mut self, _config: &Config, _state: &State, _game: &dyn Game) -> Option<Action> {
        None
    }
}

/// The lowest of the tank below `HealAt` and anyone else below
/// `EmergencyAt`, the tank winning ties.
fn most_hurt<'a>(config: &Config, members: &'a [Member]) -> Option<&'a Member> {
    let below = |member: &&Member| member.hp_percent < i32::from(if member.tank { config.heal_at } else { config.emergency_at });
    members.iter().filter(below).min_by_key(|member| (member.hp_percent, !member.tank))
}

fn is_heal(config: &Config, spell: &str) -> bool {
    HEALS.iter().any(|archetype| config.archetype(archetype).iter().any(|heal| heal == spell))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::class::FakeGame;

    #[test]
    fn keeps_memorized_every_archetype_it_reads() {
        let spells: Vec<&str> = SPELLS.iter().map(|spells| spells.archetype()).collect();
        assert_eq!(spells, Shaman.archetypes());
    }

    const ME: u32 = 1;
    const TANK: u32 = 2;
    const ROGUE: u32 = 3;
    const CLERIC: u32 = 4;

    fn config() -> Config {
        let section = |name: &str, spells: &[&str]| (name.to_owned(), spells.iter().enumerate().map(|(i, spell)| ((i + 1).to_string(), spell.to_string())).collect());
        let sections = vec![
            section("heal", &["Greater Healing", "Healing"]),
            section("heal.hot", &["Spiritual Light"]),
            section("heal.group.hot", &["Breath of Trushar"]),
            section("debuff", &["Togor's Insects"]),
        ];
        Config::from_layers(&[crate::Layer { name: "Shaman.ini".into(), sections }]).0
    }

    fn member(id: u32, hp_percent: i32) -> Member {
        Member { id, hp_percent, tank: id == TANK, fighting: id == TANK }
    }

    /// Everyone at full HP, the tank fighting, every heal memorized and
    /// ready, and the tank's HoT on.
    fn group() -> FakeGame {
        let heals = ["Greater Healing", "Spiritual Light", "Breath of Trushar", "Togor's Insects"];
        FakeGame {
            gems: heals.iter().enumerate().map(|(gem, spell)| (gem + 1, *spell)).collect(),
            ready: heals.to_vec(),
            members: [ME, TANK, ROGUE, CLERIC].map(|id| member(id, 100)).to_vec(),
            member_buffs: vec![(TANK, "Spiritual Light")],
            ..FakeGame::default()
        }
    }

    fn hurt(game: &mut FakeGame, id: u32, hp_percent: i32) {
        game.members.iter_mut().find(|member| member.id == id).unwrap().hp_percent = hp_percent;
    }

    fn urgent(game: &FakeGame) -> Option<Action> {
        Shaman.urgent(&config(), &State::new(&Config::default()), game)
    }

    fn cast(spell: &str, target: Option<u32>) -> Option<Action> {
        Some(Action::Cast { spell: spell.into(), target })
    }

    #[test]
    fn nothing_to_do_with_everyone_well() {
        assert_eq!(urgent(&group()), None);
    }

    #[test]
    fn heals_the_tank_sooner_than_the_others() {
        let mut game = group();
        hurt(&mut game, ROGUE, 60);
        assert_eq!(urgent(&game), None, "60% is above EmergencyAt");
        hurt(&mut game, TANK, 65);
        assert_eq!(urgent(&game), cast("Greater Healing", Some(TANK)), "the tank is below HealAt");
        hurt(&mut game, ROGUE, 45);
        assert_eq!(urgent(&game), cast("Greater Healing", Some(ROGUE)), "the lowest first");
        hurt(&mut game, TANK, 45);
        assert_eq!(urgent(&game), cast("Greater Healing", Some(TANK)), "the tank wins a tie");
        hurt(&mut game, ME, 30);
        assert_eq!(urgent(&game), cast("Greater Healing", Some(ME)), "itself too");
    }

    #[test]
    fn skips_members_out_of_range() {
        let mut game = group();
        hurt(&mut game, ROGUE, 20);
        hurt(&mut game, CLERIC, 40);
        game.out_of_range = vec![ROGUE];
        assert_eq!(urgent(&game), cast("Greater Healing", Some(CLERIC)), "the rogue is lower but out of range");
        game.out_of_range = vec![ROGUE, CLERIC];
        assert_eq!(urgent(&game), None, "two hurt, but neither reached by the group HoT");
        game.member_buffs.clear();
        game.out_of_range = vec![TANK];
        hurt(&mut game, ROGUE, 100);
        hurt(&mut game, CLERIC, 100);
        assert_eq!(urgent(&game), None, "the tank's HoT doesn't reach it");
    }

    #[test]
    fn uses_the_next_best_heal_when_the_best_isnt_memorized() {
        let mut game = group();
        game.gems[0] = (1, "Healing");
        game.ready.push("Healing");
        hurt(&mut game, CLERIC, 20);
        assert_eq!(urgent(&game), cast("Healing", Some(CLERIC)));
    }

    #[test]
    fn stops_a_cast_that_isnt_a_heal_for_one() {
        let mut game = group();
        hurt(&mut game, TANK, 30);
        game.bot_casting = Some("Togor's Insects");
        assert_eq!(urgent(&game), Some(Action::StopCast));
        game.bot_casting = Some("Breath of Trushar");
        assert_eq!(urgent(&game), None, "a heal finishes");
        game.bot_casting = None;
        game.casting = true;
        assert_eq!(urgent(&game), None, "a cast by hand, or memorizing, isn't the bot's to stop");
        game.casting = false;
        game.ready.retain(|spell| *spell != "Greater Healing");
        assert_eq!(urgent(&game), None, "a heal that isn't ready isn't cast, and the tank's HoT is on");
    }

    #[test]
    fn group_hot_when_enough_are_hurt_without_it() {
        let mut game = group();
        hurt(&mut game, ROGUE, 80);
        assert_eq!(urgent(&game), None, "one below GroupHotAt");
        hurt(&mut game, CLERIC, 84);
        assert_eq!(urgent(&game), cast("Breath of Trushar", None));
        game.member_buffs.push((CLERIC, "Breath of Trushar"));
        assert_eq!(urgent(&game), None, "it's on one of the two");
        hurt(&mut game, ME, 70);
        assert_eq!(urgent(&game), cast("Breath of Trushar", None));
        game.bot_casting = Some("Togor's Insects");
        assert_eq!(urgent(&game), None, "only an emergency heal stops a cast");
    }

    #[test]
    fn keeps_the_hot_on_the_tank_while_it_fights() {
        let mut game = group();
        game.member_buffs.clear();
        assert_eq!(urgent(&game), cast("Spiritual Light", Some(TANK)));
        game.members.iter_mut().for_each(|member| member.fighting = false);
        assert_eq!(urgent(&game), None, "the tank isn't fighting");
        game.members[2].fighting = true;
        assert_eq!(urgent(&game), None, "only the tank gets it");
    }
}
