//! Each pulse: what the bot sees, what `bots-core` decides, and doing it.

use std::time::{Duration, Instant};

use bots_core::{Action, Decision, Game, Mob, Seen, decide, same_name};
use mq_emu::{GroupRole, Mq, Spawn, SpawnId, yellow};

use crate::Bots;

/// Time between looks at the fight.
const LOOK_EVERY: Duration = Duration::from_millis(100);
/// Time after an action before the next, so the game has taken the last
/// (its timers) before readiness is checked again.
const ACTION_GAP: Duration = Duration::from_millis(500);
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
            Some(mob(&target, ma))
        });
        let engaged = self.state.engaged.and_then(|id| mq.spawn(SpawnId(id))).map(|spawn| match &ma {
            Some(ma) => mob(&spawn, ma),
            None => Mob { distance_to_ma: 0.0, ..mob(&spawn, &spawn) },
        });
        match decide(&mut self.state, &self.config, &Seen { ma_target, engaged }) {
            Decision::Idle => {}
            Decision::Engage(id) => self.engage(mq, id),
            Decision::Fight => self.attack(mq, &me),
            Decision::Disengage => self.disengage(mq),
        }
    }

    fn engage(&mut self, mq: &Mq<Self>, id: u32) {
        let Some(target) = mq.spawn(SpawnId(id)) else { return };
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
        match &action {
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

fn mob(spawn: &Spawn<'_>, ma: &Spawn<'_>) -> Mob {
    Mob { id: spawn.id().0, is_npc: spawn.is_npc(), hp_percent: spawn.hp_percent(), distance_to_ma: spawn.distance_to(ma) }
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
        // Only one discipline with a duration runs at a time.
        let blocked = spell.duration_cap() > 0 && self.mq.active_disc().is_some();
        ability.is_ready() && affordable && !blocked
    }

    fn in_melee_range(&self) -> bool {
        self.mq.target().is_some_and(|target| self.me.distance_to(&target) <= self.mq.melee_range(self.me, &target))
    }
}
