//! The shaman: stays back and casts. Heals, keeps melee buffs up (in a fight
//! or out), keeps its debuffs and DoTs on the mob, nukes, and keeps its
//! spells memorized.

use super::{Action, Class, Game, Member, Spells, best_memorized, melee_class, memorized_each_kind};
use crate::{Config, Mob, State};

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

    /// Melee buffs, kept up between fights too, but not while medding:
    /// they're short, and a fight casts them.
    fn rest(&mut self, config: &Config, state: &State, game: &dyn Game) -> Option<Action> {
        if game.casting() || state.medding {
            return None;
        }
        melee_buff(config, game)
    }

    /// Melee buffs, then debuffs and DoTs, then a nuke.
    fn fight(&mut self, config: &Config, _state: &State, game: &dyn Game) -> Option<Action> {
        if game.casting() {
            return None;
        }
        if let Some(buff) = melee_buff(config, game) {
            return Some(buff);
        }
        let mob = game.mob()?;
        let missing = missing_on_mob(config, game, &mob);
        let castable = |spell: &&str| game.on_mob(spell) == Some(false) && game.spell_ready(spell) && game.in_range(spell, mob.id);
        if let Some(spell) = missing.iter().copied().find(castable) {
            return Some(Action::Cast { spell: spell.to_owned(), target: Some(mob.id) });
        }
        // Until the mob's buffs are seen, a debuff may be missing: it goes
        // before a nuke.
        if missing.iter().any(|spell| game.on_mob(spell).is_none()) {
            return None;
        }
        nuke(config, game, &mob)
    }
}

/// The `debuff`, then `dot`, spells of each kind the shaman would put on the
/// mob, that aren't seen on it from this shaman (or its buffs aren't seen
/// yet): none unless mana is above `DpsManaMin` and the mob above
/// `DotStopAt`, and none the mob resisted `MaxResists` times in a row.
fn missing_on_mob<'a>(config: &'a Config, game: &dyn Game, mob: &Mob) -> Vec<&'a str> {
    if game.mana_percent() <= i32::from(config.dps_mana_min) || mob.hp_percent <= i32::from(config.dot_stop_at) {
        return Vec::new();
    }
    let spells = ["debuff", "dot"].into_iter().flat_map(|archetype| memorized_each_kind(config, archetype, game));
    spells.filter(|spell| game.on_mob(spell) != Some(true) && game.resists(spell) < config.max_resists).collect()
}

/// The best `nuke`, with mana above `NukeManaMin`.
fn nuke(config: &Config, game: &dyn Game, mob: &Mob) -> Option<Action> {
    let nuke = best_memorized(config, "nuke", game)?;
    let go = game.mana_percent() > i32::from(config.nuke_mana_min) && game.spell_ready(nuke) && game.in_range(nuke, mob.id);
    go.then(|| Action::Cast { spell: nuke.to_owned(), target: Some(mob.id) })
}

/// `buff.melee` on the first melee-class member, the tank first, then on the
/// first member's pet, that it reaches and that hasn't got it with more than
/// `RebuffAt` left.
fn melee_buff(config: &Config, game: &dyn Game) -> Option<Action> {
    let buff = best_memorized(config, "buff.melee", game).filter(|buff| game.spell_ready(buff))?;
    let mut members = game.members();
    members.sort_by_key(|member| !member.tank);
    let melee = members.iter().filter(|member| melee_class(member.class)).map(|member| member.id);
    let mut spawns = melee.chain(members.iter().filter_map(|member| member.pet));
    let target = spawns.find(|&spawn| game.in_range(buff, spawn) && game.buff_left(spawn, buff).is_none_or(|left| left < config.rebuff_at))?;
    Some(Action::Cast { spell: buff.to_owned(), target: Some(target) })
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
    const WOLF: u32 = 5;
    const MONK: u8 = 7;
    const ROGUE_CLASS: u8 = 9;
    const CLERIC_CLASS: u8 = 2;
    const SHAMAN_CLASS: u8 = 10;

    fn config() -> Config {
        let section = |name: &str, spells: &[&str]| (name.to_owned(), spells.iter().enumerate().map(|(i, spell)| ((i + 1).to_string(), spell.to_string())).collect());
        let sections = vec![
            section("heal", &["Greater Healing", "Healing"]),
            section("heal.hot", &["Spiritual Light"]),
            section("heal.group.hot", &["Breath of Trushar"]),
            section("debuff", &["Togor's Insects", "Malosi"]),
            section("dot", &["Envenomed Breath"]),
            section("nuke", &["Frost Strike"]),
            section("buff.melee", &["Spirit of the Panther"]),
        ];
        Config::from_layers(&[crate::Layer { name: "Shaman.ini".into(), sections }]).0
    }

    fn member(id: u32, hp_percent: i32) -> Member {
        let class = match id {
            ME => SHAMAN_CLASS,
            TANK => MONK,
            ROGUE => ROGUE_CLASS,
            _ => CLERIC_CLASS,
        };
        Member { id, hp_percent, tank: id == TANK, fighting: id == TANK, class, pet: None }
    }

    /// Everyone at full HP, the tank fighting, every spell memorized and
    /// ready, and the tank's HoT on.
    fn group() -> FakeGame {
        let heals = ["Greater Healing", "Spiritual Light", "Breath of Trushar", "Togor's Insects", "Spirit of the Panther"];
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

    fn fight(game: &FakeGame) -> Option<Action> {
        Shaman.fight(&config(), &State::new(&Config::default()), game)
    }

    const PANTHER: &str = "Spirit of the Panther";

    #[test]
    fn melee_buffs_the_tank_then_melee_then_pets() {
        let mut game = group();
        game.members[3].pet = Some(WOLF);
        assert_eq!(fight(&game), cast(PANTHER, Some(TANK)), "the tank first");
        game.member_buffs.push((TANK, PANTHER));
        assert_eq!(fight(&game), cast(PANTHER, Some(ROGUE)), "then the rogue; not the shaman or the cleric");
        game.member_buffs.push((ROGUE, PANTHER));
        assert_eq!(fight(&game), cast(PANTHER, Some(WOLF)), "then the cleric's pet");
        game.member_buffs.push((WOLF, PANTHER));
        assert_eq!(fight(&game), None, "on everyone");
        game.fading.push((ROGUE, PANTHER));
        assert_eq!(fight(&game), cast(PANTHER, Some(ROGUE)), "under RebuffAt");
    }

    #[test]
    fn melee_buffs_wait_for_range_the_spell_and_the_cast() {
        let mut game = group();
        game.out_of_range = vec![TANK];
        assert_eq!(fight(&game), cast(PANTHER, Some(ROGUE)), "the tank is out of range");
        game.bot_casting = Some("Spiritual Light");
        assert_eq!(fight(&game), None, "a cast is under way");
        game.bot_casting = None;
        game.ready.retain(|spell| *spell != PANTHER);
        assert_eq!(fight(&game), None, "not ready");
        game.gems.retain(|(_, spell)| *spell != PANTHER);
        assert_eq!(fight(&game), None, "not memorized");
    }

    #[test]
    fn melee_buffs_between_fights_too() {
        let mut game = group();
        let rest = |game: &FakeGame| Shaman.rest(&config(), &State::new(&Config::default()), game);
        assert_eq!(rest(&game), cast(PANTHER, Some(TANK)));
        game.member_buffs.extend([(TANK, PANTHER), (ROGUE, PANTHER)]);
        assert_eq!(rest(&game), None, "on everyone");
        game.member_buffs.clear();
        game.casting = true;
        assert_eq!(rest(&game), None, "a cast is under way");
        game.casting = false;
        let medding = State { medding: true, ..State::new(&Config::default()) };
        assert_eq!(Shaman.rest(&config(), &medding, &game), None, "not while medding: a fight casts them");
        assert_eq!(fight(&game), cast(PANTHER, Some(TANK)), "in a fight, still");
    }

    const MOB: u32 = 50;
    const TOGOR: &str = "Togor's Insects";
    const MALOSI: &str = "Malosi";
    const BREATH: &str = "Envenomed Breath";
    const FROST: &str = "Frost Strike";

    /// A fight against a mob at 90% with nothing on it and its buffs seen,
    /// at full mana, with the melee buffed and every spell memorized and
    /// ready.
    fn fighting() -> FakeGame {
        let mut game = group();
        for (gem, spell) in [(6, MALOSI), (7, BREATH), (8, FROST)] {
            game.gems.push((gem, spell));
            game.ready.push(spell);
        }
        game.kinds = vec![(TOGOR, 88), (MALOSI, 81), (BREATH, 1)];
        game.member_buffs.extend([(TANK, PANTHER), (ROGUE, PANTHER)]);
        game.mana_percent = 100;
        game.mob = Some(Mob { id: MOB, is_npc: true, hp_percent: 90, distance: 50.0 });
        game.on_mob = Some(vec![]);
        game
    }

    #[test]
    fn debuffs_then_dots_then_a_nuke() {
        let mut game = fighting();
        assert_eq!(fight(&game), cast(TOGOR, Some(MOB)), "the slow first");
        game.on_mob = Some(vec![TOGOR]);
        assert_eq!(fight(&game), cast(MALOSI, Some(MOB)));
        game.on_mob = Some(vec![TOGOR, MALOSI]);
        assert_eq!(fight(&game), cast(BREATH, Some(MOB)), "then the DoT");
        game.on_mob = Some(vec![TOGOR, MALOSI, BREATH]);
        assert_eq!(fight(&game), cast(FROST, Some(MOB)), "then a nuke");
        game.member_buffs.clear();
        assert_eq!(fight(&game), cast(PANTHER, Some(TANK)), "melee buffs come before");
    }

    #[test]
    fn skips_a_debuff_that_cant_be_cast_now() {
        let mut game = fighting();
        game.ready.retain(|spell| *spell != TOGOR);
        assert_eq!(fight(&game), cast(MALOSI, Some(MOB)), "the slow isn't ready");
        game.ready.retain(|spell| *spell != MALOSI && *spell != BREATH);
        assert_eq!(fight(&game), cast(FROST, Some(MOB)), "a nuke while they recast");
        game.out_of_range = vec![MOB];
        assert_eq!(fight(&game), None, "out of range");
    }

    #[test]
    fn waits_for_the_mobs_buffs_before_debuffing_or_nuking() {
        let mut game = fighting();
        game.on_mob = None;
        assert_eq!(fight(&game), None, "a debuff may be missing");
        game.mob.as_mut().unwrap().hp_percent = 25;
        assert_eq!(fight(&game), cast(FROST, Some(MOB)), "no debuffs at DotStopAt, so nothing to wait for");
    }

    #[test]
    fn mana_floors_and_dot_stop() {
        let mut game = fighting();
        game.mana_percent = 70;
        game.on_mob = Some(vec![TOGOR, MALOSI, BREATH]);
        assert_eq!(fight(&game), None, "a nuke only above NukeManaMin");
        game.on_mob = Some(vec![]);
        assert_eq!(fight(&game), cast(TOGOR, Some(MOB)));
        game.mana_percent = 40;
        assert_eq!(fight(&game), None, "debuffs only above DpsManaMin");
        game.mana_percent = 100;
        game.mob.as_mut().unwrap().hp_percent = 25;
        assert_eq!(fight(&game), cast(FROST, Some(MOB)), "no debuffs or DoTs at DotStopAt, but nukes");
    }

    #[test]
    fn gives_up_on_a_line_after_resists_in_a_row() {
        let mut game = fighting();
        game.resists = vec![(TOGOR, 2)];
        assert_eq!(fight(&game), cast(TOGOR, Some(MOB)), "under MaxResists");
        game.resists = vec![(TOGOR, 3), (MALOSI, u8::MAX), (BREATH, 3)];
        assert_eq!(fight(&game), cast(FROST, Some(MOB)), "all given up on, so a nuke");
    }
}
