//! The shaman: stays back and casts. Only keeps its spells memorized so far;
//! heals, buffs, debuffs, DoTs and nukes come in milestones S3 to S5.

use super::{Action, Class, Game, Spells};
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

    fn rest(&mut self, _config: &Config, _state: &State, _game: &dyn Game) -> Option<Action> {
        None
    }

    fn fight(&mut self, _config: &Config, _state: &State, _game: &dyn Game) -> Option<Action> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_memorized_every_archetype_it_reads() {
        let spells: Vec<&str> = SPELLS.iter().map(|spells| spells.archetype()).collect();
        assert_eq!(spells, Shaman.archetypes());
    }
}
