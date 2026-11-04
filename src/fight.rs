//! Each pulse: what the bot sees, what `bots-core` decides, and doing it.

use std::time::{Duration, Instant};

use bots_core::{Action, Decision, Game, Mob, Seen, decide, same_name};
use bots_core::Mode;
use mq_emu::{CombatState, GroupRole, Mq, Spawn, SpawnId, SpellId, green, yellow};

use crate::Bots;

/// Time between looks at the fight.
const LOOK_EVERY: Duration = Duration::from_millis(100);
/// Time after an action before the next, so the game has taken the last
/// (its timers) before readiness is checked again.
const ACTION_GAP: Duration = Duration::from_millis(500);
/// The same, out of combat: an aura takes a moment to show in the aura
/// window, and nothing there is in a hurry.
const REST_GAP: Duration = Duration::from_secs(3);
const MOVEUTILS: &str = "MQ2MoveUtils";

#[derive(Default)]
pub struct Fight {
    looked: Option<Instant>,
    next_action: Option<Instant>,
    /// Said once that sticking needs MQ2MoveUtils.
    warned_stick: bool,
}

impl Bots {
    pub fn fight(&mut self, mq: &Mq<Self>) {
        if self.fight.looked.is_some_and(|at| at.elapsed() < LOOK_EVERY) {
            return;
        }
        self.fight.looked = Some(Instant::now());
        let Some(me) = mq.me() else { return };
        let ma = main_assist(mq).filter(|ma| ma.id() != me.id());
        let ma_target = ma.as_ref().and_then(|ma| {
            let status = self.posse.as_mut()?.status(&ma.name())?;
            let target = mq.spawn(SpawnId(status.target_id)).filter(|_| status.target_id != 0)?;
            // The MA has it targeted, so its client has the HP from the
            // server; this one's copy of a mob it hasn't targeted lags.
            Some(Mob { hp_percent: status.target_hp_percent, ..mob(&target, &me) })
        });
        let engaged = self.state.engaged.and_then(|id| mq.spawn(SpawnId(id))).map(|spawn| mob(&spawn, &me));
        match decide(&mut self.state, &self.config, &Seen { ma_target, engaged }) {
            Decision::Idle => self.rest(mq, &me),
            Decision::Engage(id) => self.engage(mq, &me, id, ma_target.filter(|mob| mob.id == id).map_or(0, |mob| mob.hp_percent)),
            Decision::Fight => self.attack(mq, &me),
            Decision::Disengage => self.disengage(mq),
        }
    }

    /// `hp_percent` as the MA sees it, to say in chat.
    fn engage(&mut self, mq: &Mq<Self>, me: &Spawn<'_>, id: u32, hp_percent: i32) {
        let Some(target) = mq.spawn(SpawnId(id)) else { return };
        mq.chat(green!("[Bots] engaging {} ({hp_percent}% HP to the MA, {}% here, {:.0} away)", target.name(), target.hp_percent(), me.distance_to(&target)));
        self.state.engaged = Some(id);
        mq.set_target(Some(&target));
        if mq.is_plugin_loaded(MOVEUTILS) {
            mq.command("/stick behind");
        } else if !std::mem::replace(&mut self.fight.warned_stick, true) {
            mq.chat(yellow!("[Bots] load {MOVEUTILS} to stick to mobs"));
        }
        mq.set_auto_attack(true);
        self.fight.next_action = None;
    }

    fn attack(&mut self, mq: &Mq<Self>, me: &Spawn<'_>) {
        let Some(id) = self.state.engaged else { return };
        // Feigning, by hand for now: anything done would stand the monk up.
        if me.is_feigning() {
            return;
        }
        if mq.target().is_none_or(|target| target.id().0 != id) {
            let Some(target) = mq.spawn(SpawnId(id)) else { return };
            mq.set_target(Some(&target));
        }
        if self.fight.next_action.is_some_and(|at| Instant::now() < at) {
            return;
        }
        if !mq.is_auto_attacking() {
            mq.set_auto_attack(true);
            self.fight.next_action = Some(Instant::now() + ACTION_GAP);
            return;
        }
        let Some(class) = self.class.as_mut() else { return };
        let Some(action) = class.fight(&self.config, &self.state, &Seeing { mq, me }) else { return };
        self.act(mq, &action);
    }

    /// Out of combat: buffs and auras, when nothing is in the way.
    fn rest(&mut self, mq: &Mq<Self>, me: &Spawn<'_>) {
        if self.state.mode == Mode::Off || me.is_feigning() || me.is_stunned() || me.casting().is_some() {
            return;
        }
        if mq.is_auto_attacking() || mq.combat_state() == Some(CombatState::Combat) {
            return;
        }
        if self.fight.next_action.is_some_and(|at| Instant::now() < at) {
            return;
        }
        let Some(class) = self.class.as_mut() else { return };
        let Some(action) = class.rest(&self.config, &self.state, &Seeing { mq, me }) else { return };
        self.act(mq, &action);
        self.fight.next_action = Some(Instant::now() + REST_GAP);
    }

    fn act(&mut self, mq: &Mq<Self>, action: &Action) {
        match action {
            Action::Ability(name) => {
                if let Some(skill) = mq.abilities().find(|skill| same_name(name, &skill.name())) {
                    mq.use_ability(&skill);
                }
            }
            Action::Disc(name) => {
                if let Some(ability) = mq.combat_abilities().find(|ability| same_name(name, &ability.name())) {
                    mq.use_combat_ability(&ability);
                }
            }
        }
        self.fight.next_action = Some(Instant::now() + ACTION_GAP);
    }

    pub fn disengage(&mut self, mq: &Mq<Self>) {
        let Some(id) = self.state.engaged.take() else { return };
        if mq.is_auto_attacking() {
            mq.set_auto_attack(false);
        }
        if mq.is_plugin_loaded(MOVEUTILS) {
            mq.command("/stick off");
        }
        if mq.target().is_some_and(|target| target.id().0 == id) {
            mq.set_target(None);
        }
    }
}

/// The group's Main Assist, if it's in the zone.
fn main_assist<'a>(mq: &'a Mq<Bots>) -> Option<Spawn<'a>> {
    mq.group()?.with_role(GroupRole::MainAssist)?.spawn()
}

fn mob(spawn: &Spawn<'_>, me: &Spawn<'_>) -> Mob {
    Mob { id: spawn.id().0, is_npc: spawn.is_npc(), hp_percent: spawn.hp_percent(), distance: spawn.distance_to(me) }
}

/// The game as the class sees it, against the target.
struct Seeing<'a> {
    mq: &'a Mq<Bots>,
    me: &'a Spawn<'a>,
}

impl Game for Seeing<'_> {
    fn ability_ready(&self, name: &str) -> bool {
        self.mq.abilities().find(|skill| same_name(name, &skill.name())).is_some_and(|skill| skill.is_ready())
    }

    fn disc_ready(&self, name: &str) -> bool {
        let Some(ability) = self.mq.combat_abilities().find(|ability| same_name(name, &ability.name())) else { return false };
        let spell = ability.spell();
        let affordable = self.mq.endurance_current().is_some_and(|endurance| endurance >= spell.endurance_cost());
        // One discipline runs at a time; abilities whose buff goes in the
        // song window (Fists of Wu) don't take its place.
        let blocked = spell.duration_cap() > 0 && !spell.is_short_duration() && self.mq.active_disc().is_some();
        ability.is_ready() && affordable && !blocked
    }

    fn disc_known(&self, name: &str) -> bool {
        self.mq.combat_abilities().any(|ability| same_name(name, &ability.name()))
    }

    fn aura_active(&self, name: &str) -> bool {
        let auras = self.mq.auras();
        if auras.is_empty() {
            return false;
        }
        // The aura's name may be the INI's, the ability's spell's, or the
        // next spell's (often the aura's own effect).
        let mut names = vec![name.to_owned()];
        if let Some(ability) = self.mq.combat_abilities().find(|ability| same_name(name, &ability.name())) {
            names.push(ability.name());
            names.extend(self.mq.spell(SpellId(ability.id().0 + 1)).map(|spell| spell.name()));
        }
        auras.iter().any(|aura| names.iter().any(|name| same_name(name, &aura.name) || same_name(&aura.name, name)))
    }

    fn disc_buff_left(&self, name: &str) -> Option<Duration> {
        let ability = self.mq.combat_abilities().find(|ability| same_name(name, &ability.name()))?;
        // Its own spell, or else a buff by the INI's name.
        let buff = self.mq.buff(ability.id()).or_else(|| self.mq.buffs().chain(self.mq.songs()).find(|buff| same_name(name, &buff.name())));
        buff?.remaining()
    }

    fn in_melee_range(&self) -> bool {
        self.mq.target().is_some_and(|target| self.me.distance_to(&target) <= self.mq.melee_range(self.me, &target))
    }
}
