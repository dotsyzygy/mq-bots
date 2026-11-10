//! What casters share against a mob: their debuffs (and DoTs) of each kind,
//! then a nuke.

use super::{Action, Game, best_memorized, memorized_each_kind};
use crate::{Config, Mob};

/// The first spell of each kind in `archetypes` (in order) missing on the
/// mob that can be cast now, or else, once none could still be missing, the
/// best `nuke`.
pub fn damage(config: &Config, game: &dyn Game, archetypes: &[&str]) -> Option<Action> {
    let mob = game.mob()?;
    let missing = missing_on_mob(config, game, &mob, archetypes);
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

/// The spells of each kind in `archetypes` the caster would put on the mob,
/// that aren't seen on it from this caster (or its buffs aren't seen yet):
/// none unless mana is above `DpsManaMin` and the mob above `DotStopAt`, no
/// DoT on a mob that dies sooner than `DotMinFight`, and none the mob
/// resisted `MaxResists` times in a row.
fn missing_on_mob<'a>(config: &'a Config, game: &dyn Game, mob: &Mob, archetypes: &[&str]) -> Vec<&'a str> {
    if game.mana_percent() <= i32::from(config.dps_mana_min) || mob.hp_percent <= i32::from(config.dot_stop_at) {
        return Vec::new();
    }
    let dies_soon = game.mob_dies_in().is_some_and(|left| left < config.dot_min_fight);
    let worth = |archetype: &&&str| !(dies_soon && archetype.eq_ignore_ascii_case("dot"));
    let spells = archetypes.iter().filter(worth).flat_map(|archetype| memorized_each_kind(config, archetype, game));
    spells.filter(|spell| game.on_mob(spell) != Some(true) && game.resists(spell) < config.max_resists).collect()
}

/// The best `nuke`, with mana above `NukeManaMin`.
fn nuke(config: &Config, game: &dyn Game, mob: &Mob) -> Option<Action> {
    let nuke = best_memorized(config, "nuke", game)?;
    let go = game.mana_percent() > i32::from(config.nuke_mana_min) && game.spell_ready(nuke) && game.in_range(nuke, mob.id);
    go.then(|| Action::Cast { spell: nuke.to_owned(), target: Some(mob.id) })
}
