//! The magician: stays back and casts. Keeps its debuffs (the malo line) on
//! the mob, nukes, and keeps its spells memorized. Its pet comes later.

use super::{Action, Class, Game, Spells, caster};
use crate::{Config, State};

pub struct Magician;

/// Most important first, for when the gems can't hold them all.
const SPELLS: &[Spells] = &[Spells::EachKind("debuff"), Spells::Best("nuke")];

impl Class for Magician {
    fn archetypes(&self) -> &'static [&'static str] {
        &["debuff", "nuke"]
    }

    fn spells(&self) -> &'static [Spells] {
        SPELLS
    }

    fn melee(&self) -> bool {
        false
    }

    /// Debuffs, then a nuke.
    fn fight(&mut self, config: &Config, _state: &State, game: &dyn Game) -> Option<Action> {
        if game.casting() {
            return None;
        }
        caster::damage(config, game, &["debuff"])
    }

    fn rest(&mut self, _config: &Config, _state: &State, _game: &dyn Game) -> Option<Action> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mob;
    use crate::class::FakeGame;

    #[test]
    fn keeps_memorized_every_archetype_it_reads() {
        let spells: Vec<&str> = SPELLS.iter().map(|spells| spells.archetype()).collect();
        assert_eq!(spells, Magician.archetypes());
    }

    const MOB: u32 = 50;
    const MALO: &str = "Malosi";
    const BOLT: &str = "Bolt of Flame";

    fn config() -> Config {
        let section = |name: &str, spells: &[&str]| (name.to_owned(), spells.iter().enumerate().map(|(i, spell)| ((i + 1).to_string(), spell.to_string())).collect());
        Config::from_layers(&[crate::Layer { name: "Magician.ini".into(), sections: vec![section("debuff", &[MALO]), section("nuke", &[BOLT])] }]).0
    }

    /// A fight against a mob at 90% with nothing on it and its buffs seen,
    /// at full mana, with both spells memorized and ready.
    fn fighting() -> FakeGame {
        FakeGame {
            gems: vec![(1, MALO), (2, BOLT)],
            ready: vec![MALO, BOLT],
            mana_percent: 100,
            mob: Some(Mob { id: MOB, is_npc: true, hp_percent: 90, distance: 50.0 }),
            on_mob: Some(vec![]),
            ..FakeGame::default()
        }
    }

    fn fight(game: &FakeGame) -> Option<Action> {
        Magician.fight(&config(), &State::new(&Config::default()), game)
    }

    fn cast(spell: &str) -> Option<Action> {
        Some(Action::Cast { spell: spell.into(), target: Some(MOB) })
    }

    #[test]
    fn malos_then_nukes() {
        let mut game = fighting();
        assert_eq!(fight(&game), cast(MALO));
        game.on_mob = Some(vec![MALO]);
        assert_eq!(fight(&game), cast(BOLT));
        game.bot_casting = Some(BOLT);
        assert_eq!(fight(&game), None, "a cast is under way");
    }

    #[test]
    fn nothing_on_a_mob_out_of_reach() {
        let mut game = fighting();
        game.out_of_range = vec![MOB];
        assert_eq!(fight(&game), None, "out of range, or out of sight");
    }

    #[test]
    fn mana_floors() {
        let mut game = fighting();
        game.mana_percent = 70;
        assert_eq!(fight(&game), cast(MALO), "a debuff above DpsManaMin");
        game.on_mob = Some(vec![MALO]);
        assert_eq!(fight(&game), None, "a nuke only above NukeManaMin");
    }
}
