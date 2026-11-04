//! The shaman: stays back and casts. Only keeps its spells memorized so far;
//! heals, buffs, debuffs, DoTs and nukes come in milestones S3 to S5.

use super::{Action, Class, Game};
use crate::{Config, State};

pub struct Shaman;

/// Most important first, for when the gems can't hold them all.
const SPELL_LINES: &[&str] = &["heal", "heal.hot", "heal.group", "buff.melee", "debuff.#", "dot.#", "nuke"];

impl Class for Shaman {
    fn archetypes(&self) -> &'static [&'static str] {
        SPELL_LINES
    }

    fn spell_lines(&self) -> &'static [&'static str] {
        SPELL_LINES
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
