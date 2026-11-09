//! Each pulse: what the bot sees, what `bots-core` decides, and doing it.

use std::time::{Duration, Instant};

use bots_core::{Action, Decision, Game, Member, Mob, Mode, Outcome, Pet, PetOrder, Resists, Seen, aura_named, decide, follow_target, medding, on_disengage, pet_order, plan_gems, same_name, sit};
use mq_emu::{Cast, CastResult, CombatState, GroupRole, MemorizeResult, Mq, PetCommand, Spawn, SpawnId, Spell, SpellId, Task, TaskId, XTARGET_AUTO_HATER, XTargetStatus, green, yellow};
use posse_core::{BuffEntry, flags};
use posse_shm::now_ms;

use crate::Bots;

/// Time between looks at the fight.
const LOOK_EVERY: Duration = Duration::from_millis(100);
/// Time after an action before the next, so the game has taken the last
/// (its timers) before readiness is checked again.
const ACTION_GAP: Duration = Duration::from_millis(500);
/// The same, out of combat: an aura takes a moment to show in the aura
/// window, and nothing there is in a hurry.
const REST_GAP: Duration = Duration::from_secs(3);
/// An aura used again this soon, with auras up, isn't being seen.
const AURA_RETRY: Duration = Duration::from_secs(30);
/// A spell that just landed on someone is taken as on them this long, until
/// Posse shows it: no casting it twice.
const LANDED_HOLD: Duration = Duration::from_secs(6);
/// A spell that didn't take hold on someone (a stronger one is on them) is
/// taken as on them this long.
const BLOCKED_HOLD: Duration = Duration::from_secs(60);
/// The same order isn't given the pet again sooner than this, while the
/// server answers the first.
const PET_ORDER_GAP: Duration = Duration::from_secs(2);
/// A caster in a fight turns to face the mob when it's further off than this.
const FACE_WITHIN_DEGREES: f32 = 5.0;
/// How far into the game's melee range a melee bot sticks, in percent. The
/// game's range adds the height difference between the two spawns, so a tall
/// mob gets more than the server lets you hit from ("Your target is too far
/// away"); half of it is in reach for the server too.
const STICK_PERCENT: f32 = 50.0;
/// Abilities wait until the bot is this far into the game's melee range, in
/// percent: where it sticks, with some slack for the stick wobbling.
const REACH_PERCENT: f32 = 60.0;
const MOVEUTILS: &str = "MQ2MoveUtils";
const ADVPATH: &str = "MQ2AdvPath";

#[derive(Default)]
pub struct Fight {
    looked: Option<Instant>,
    next_action: Option<Instant>,
    /// Said once that sticking needs MQ2MoveUtils.
    warned_stick: bool,
    /// Said once that following needs MQ2AdvPath.
    warned_follow: bool,
    /// The last order given the pet, and when.
    pet_order: Option<(PetOrder, Instant)>,
    /// The spawn `/afollow` was last sent for, until `/afollow off`.
    pub followed: Option<u32>,
    /// Sat down to med, so it's this plugin's to stand up again; one sat
    /// by hand is left sitting.
    sat: bool,
    /// The aura last used, and when.
    last_aura: Option<(String, Instant)>,
    pub given_up_auras: Vec<String>,
    /// The bot's cast or memorizing, while it runs.
    pub task: Option<TaskId>,
    /// The spell, by the INI's name, of the bot's cast while it runs.
    task_spell: Option<String>,
    /// Spells that landed lately, by the INI's name, on a spawn (or the
    /// group, for `None`), and when.
    landed: Vec<(String, Option<u32>, Instant)>,
    /// The same for spells that didn't take hold (a stronger one is on).
    blocked: Vec<(String, Option<u32>, Instant)>,
    /// When the last action was taken.
    last_action: Option<Instant>,
    /// Spells, by the INI's name, that wouldn't memorize; the next best is
    /// used until /bots reload.
    pub given_up_spells: Vec<String>,
    /// Said which spell lines the gems have no room for, since the load.
    pub said_left_out: bool,
    /// The last cast that didn't go off, and how, so it's said once.
    last_failure: Option<(String, CastResult)>,
    /// Debuff and DoT resists in a row on the mob being fought.
    pub resists: Resists,
}

impl Bots {
    pub fn fight(&mut self, mq: &Mq<Self>) {
        if self.fight.looked.is_some_and(|at| at.elapsed() < LOOK_EVERY) {
            return;
        }
        self.fight.looked = Some(Instant::now());
        let Some(me) = mq.me() else { return };
        self.state.medding = medding(&self.config, self.state.medding, mana_percent(mq));
        let ma = main_assist(mq).filter(|ma| ma.id() != me.id());
        let ma_target = ma.as_ref().and_then(|ma| {
            let status = self.posse.as_mut()?.status(&ma.name())?;
            let target = mq.spawn(SpawnId(status.target_id)).filter(|_| status.target_id != 0)?;
            // The MA has it targeted, so its client has the HP from the
            // server; this one's copy of a mob it hasn't targeted lags.
            Some(Mob { hp_percent: status.target_hp_percent, ..mob(&target, &me) })
        });
        let engaged = self.state.engaged.and_then(|id| mq.spawn(SpawnId(id))).map(|spawn| mob(&spawn, &me));
        let decision = decide(&mut self.state, &self.config, &Seen { ma_target, engaged });
        // The engaged mob with the MA's HP for it, when it's the MA's target.
        let fought = engaged.map(|engaged| ma_target.filter(|mob| mob.id == engaged.id).unwrap_or(engaged));
        let busy = me.is_feigning() || me.casting().is_some() || self.fight.task.is_some();
        let decided_engage = matches!(decision, Decision::Engage(_));
        self.follow(mq, if decided_engage { None } else { follow_target(&self.state, ma.as_ref().map(|ma| ma.id().0), busy) });
        // About to be moved off by following.
        let leaving = self.fight.followed.is_some() && ma.as_ref().is_some_and(|ma| ma.is_moving());
        // Anything hating the group makes it a fight for sitting, engaged
        // or not: melee stay up, and casters sit only out of spell mana.
        let in_fight = self.state.engaged.is_some() || haters(mq);
        let mana = mana_percent(mq).unwrap_or(0);
        if self.fight.sat && (!sit(&self.config, &self.state, true, in_fight, self.melee(), mana) || leaving) {
            self.stand(mq);
        }
        if matches!(decision, Decision::Idle | Decision::Fight) && self.urgent(mq, &me, fought) {
            return;
        }
        let acted = self.fight.last_action;
        match decision {
            Decision::Idle => self.rest(mq, &me),
            Decision::Engage(id) => self.engage(mq, &me, id, ma_target.filter(|mob| mob.id == id).map_or(0, |mob| mob.hp_percent)),
            Decision::Fight => self.attack(mq, &me, fought),
            Decision::Disengage => self.disengage(mq),
        }
        let idle = self.fight.last_action == acted && self.fight.task.is_none() && !leaving && !me.is_moving() && me.casting().is_none() && !me.is_feigning() && !me.is_stunned();
        if me.is_standing() && sit(&self.config, &self.state, idle, in_fight, self.melee(), mana) {
            mq.set_sitting(true);
            self.fight.sat = true;
        }
    }

    /// Stands up from medding, or from sitting by hand for something that
    /// needs it.
    fn stand(&mut self, mq: &Mq<Self>) {
        self.fight.sat = false;
        if mq.me().is_some_and(|me| me.is_sitting()) {
            mq.set_sitting(false);
        }
    }

    /// `hp_percent` as the MA sees it, to say in chat.
    fn engage(&mut self, mq: &Mq<Self>, me: &Spawn<'_>, id: u32, hp_percent: i32) {
        let Some(target) = mq.spawn(SpawnId(id)) else { return };
        mq.chat(green!("[Bots] engaging {} ({hp_percent}% HP to the MA, {}% here, {:.0} away)", target.name(), target.hp_percent(), me.distance_to(&target)));
        self.state.engaged = Some(id);
        self.stand(mq);
        self.fight.resists.keep_only(id);
        self.fight.next_action = None;
        // A cast on someone else goes back to its target when it ends.
        if self.fight.task.is_none() {
            mq.set_target(Some(&target));
        }
        if !self.melee() {
            return;
        }
        if mq.is_plugin_loaded(MOVEUTILS) {
            mq.command(&format!("/stick behind {STICK_PERCENT}%"));
        } else if !std::mem::replace(&mut self.fight.warned_stick, true) {
            mq.chat(yellow!("[Bots] load {MOVEUTILS} to stick to mobs"));
        }
        mq.set_auto_attack(true);
    }

    /// Heals, in a fight or out; whether something was done.
    fn urgent(&mut self, mq: &Mq<Self>, me: &Spawn<'_>, fought: Option<Mob>) -> bool {
        if self.state.mode != Mode::Assist || me.is_feigning() || me.is_stunned() || self.fight.last_action.is_some_and(|at| at.elapsed() < ACTION_GAP) {
            return false;
        }
        let group = self.group(mq, me);
        let Some(class) = self.class.as_mut() else { return false };
        let Some(action) = class.urgent(&self.config, &self.state, &Seeing::new(mq, me, &self.fight, group, fought)) else { return false };
        self.act(mq, &action);
        true
    }

    /// The group as the class sees it: members from the group window, their
    /// HP and buffs from Posse when they're in it.
    fn group(&mut self, mq: &Mq<Self>, me: &Spawn<'_>) -> Vec<Mate> {
        let group = mq.group();
        let tank = group.and_then(|group| group.with_role(GroupRole::MainTank).or_else(|| group.with_role(GroupRole::MainAssist))).map(|member| member.name());
        let spawns: Vec<(String, Spawn<'_>)> = match group {
            Some(group) => group.members().filter_map(|member| Some((member.name(), member.spawn()?))).collect(),
            None => mq.spawn(me.id()).map(|spawn| (me.name(), spawn)).into_iter().collect(),
        };
        let mut mates = Vec::new();
        for (name, spawn) in spawns {
            if spawn.is_corpse() {
                continue;
            }
            let tank = tank.as_ref().is_some_and(|tank| tank.eq_ignore_ascii_case(&name));
            let id = spawn.id().0;
            let pet = mq.pet_of(&spawn).filter(|pet| !pet.is_corpse()).map(|pet| pet.id().0);
            let (hp_percent, fighting, buffs, pet_buffs) = if id == me.id().0 {
                let mine = mq.buffs().chain(mq.songs()).filter_map(|buff| Some((buff.spell()?.id().0, buff.remaining().unwrap_or(Duration::MAX))));
                let pets = mq.pet_buffs().filter_map(|buff| Some((buff.spell()?.id().0, buff.remaining().unwrap_or(Duration::MAX))));
                (me.hp_percent(), mq.is_auto_attacking(), Some(mine.collect()), Some(pets.collect()))
            } else if let Some(status) = self.posse.as_mut().and_then(|posse| posse.status(&name)).filter(|status| status.id == id) {
                let age = status.age_ms(now_ms());
                let left = |ms: Option<i64>| ms.map_or(Duration::MAX, |ms| Duration::from_millis(ms.max(0) as u64));
                let seen = |buffs: &mut dyn Iterator<Item = &BuffEntry>| buffs.filter(|buff| !buff.is_empty()).map(|buff| (buff.spell_id, left(buff.ms_left_after(age)))).filter(|(_, left)| !left.is_zero()).collect();
                // Its pet's buffs, if Posse has the same pet.
                let pet_buffs = pet.filter(|pet| *pet == status.pet_id).map(|_| seen(&mut status.pet_buffs.iter()));
                (status.hp_percent, status.has_flag(flags::COMBAT), Some(seen(&mut status.buffs.iter().chain(&status.songs))), pet_buffs)
            } else {
                (spawn.hp_percent(), false, None, None)
            };
            if hp_percent > 0 {
                mates.push(Mate { member: Member { id, hp_percent, tank, fighting, class: spawn.class(), pet }, buffs, pet_buffs });
            }
        }
        mates
    }

    fn melee(&self) -> bool {
        self.class.as_ref().is_some_and(|class| class.melee())
    }

    fn attack(&mut self, mq: &Mq<Self>, me: &Spawn<'_>, fought: Option<Mob>) {
        let Some(id) = self.state.engaged else { return };
        // Feigning, by hand for now: anything done would stand the monk up.
        if me.is_feigning() {
            return;
        }
        // A cast on someone else goes back to the mob when it ends.
        if self.fight.task.is_none() && mq.target().is_none_or(|target| target.id().0 != id) {
            let Some(target) = mq.spawn(SpawnId(id)) else { return };
            mq.set_target(Some(&target));
        }
        // A caster faces the mob: a bolt cast off to one side can miss it
        // and hit something else.
        if !self.melee()
            && let Some(mob) = mq.spawn(SpawnId(id))
        {
            let toward = me.heading_to(&mob);
            if me.heading().difference(toward) > FACE_WITHIN_DEGREES {
                let _ = mq.face_now(toward);
            }
        }
        // The pet goes for the mob only while it's targeted (`/pet attack`
        // takes your target), not while a cast on someone else has it.
        let mob_targeted = self.fight.task.is_none() && mq.target().is_some_and(|target| target.id().0 == id);
        self.order_pet(mq, |config, pet| pet_order(config, pet, mob_targeted.then_some(id)));
        if self.fight.next_action.is_some_and(|at| Instant::now() < at) {
            return;
        }
        if self.melee() && !mq.is_auto_attacking() {
            mq.set_auto_attack(true);
            self.fight.next_action = Some(Instant::now() + ACTION_GAP);
            return;
        }
        let group = self.group(mq, me);
        let Some(class) = self.class.as_mut() else { return };
        let Some(action) = class.fight(&self.config, &self.state, &Seeing::new(mq, me, &self.fight, group, fought)) else { return };
        self.act(mq, &action);
    }

    /// Out of combat: gems, buffs and auras, when nothing is in the way
    /// (moving included: following, or by hand).
    fn rest(&mut self, mq: &Mq<Self>, me: &Spawn<'_>) {
        if self.state.mode == Mode::Off || me.is_feigning() || me.is_stunned() || me.is_moving() || me.casting().is_some() || self.fight.task.is_some() {
            return;
        }
        if mq.is_auto_attacking() || mq.combat_state() == Some(CombatState::Combat) {
            return;
        }
        self.order_pet(mq, |config, pet| pet_order(config, pet, None));
        if self.fight.next_action.is_some_and(|at| Instant::now() < at) {
            return;
        }
        let group = self.group(mq, me);
        let seeing = Seeing::new(mq, me, &self.fight, group, None);
        let mut action = None;
        let mut left_out = None;
        if self.state.memorize
            && let Some(class) = &self.class
        {
            let plan = plan_gems(&self.config, class.spells(), &seeing, &self.fight.given_up_spells);
            if !plan.left_out.is_empty() && !self.fight.said_left_out {
                left_out = Some((plan.wanted, seeing.gem_count(), plan.left_out.join(", ")));
            }
            self.state.memorize = plan.next.is_some();
            action = plan.next;
        }
        if action.is_none()
            && let Some(class) = self.class.as_mut()
        {
            action = class.rest(&self.config, &self.state, &seeing);
        }
        if let Some((wanted, gems, spells)) = left_out {
            mq.chat(yellow!("[Bots] {wanted} spells to keep memorized but {gems} gems, so these are left out: {spells}"));
            self.fight.said_left_out = true;
        }
        let Some(action) = action else { return };
        if self.repeats_an_aura(mq, &action) {
            return;
        }
        self.act(mq, &action);
        self.fight.next_action = Some(Instant::now() + REST_GAP);
    }

    /// Whether `action` is an aura used moments ago while auras are up: its
    /// name in the aura window can't be matched, so it would be used again
    /// and again. It's given up on (taken as up) and said in chat.
    fn repeats_an_aura(&mut self, mq: &Mq<Self>, action: &Action) -> bool {
        let Action::Disc(name) = action else { return false };
        if !self.config.archetype("aura").iter().any(|aura| aura == name) {
            return false;
        }
        let repeated = self.fight.last_aura.as_ref().is_some_and(|(last, at)| last == name && at.elapsed() < AURA_RETRY);
        let auras: Vec<String> = mq.auras().into_iter().map(|aura| aura.name).collect();
        if repeated && !auras.is_empty() {
            mq.chat(yellow!("[Bots] {name} doesn't match an aura that's up ({}), so it's taken as up until /bots reload", auras.join(", ")));
            self.fight.given_up_auras.push(name.clone());
            return true;
        }
        self.fight.last_aura = Some((name.clone(), Instant::now()));
        false
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
            Action::Cast { spell, target } => self.cast(mq, spell, *target),
            Action::Memorize { gem, spell } => self.memorize(mq, *gem, spell),
            Action::StopCast => {
                if let Some(spell) = &self.fight.task_spell {
                    mq.chat(yellow!("[Bots] stopping {spell} for a heal"));
                }
                self.stop_task(mq);
            }
        }
        self.fight.next_action = Some(Instant::now() + ACTION_GAP);
        self.fight.last_action = Some(Instant::now());
    }

    /// Gives your pet, if you have one, the order `decide` picks, unless
    /// it's the one given last, under [`PET_ORDER_GAP`] ago.
    fn order_pet(&mut self, mq: &Mq<Self>, decide: impl FnOnce(&bots_core::Config, &Pet) -> Option<PetOrder>) {
        if mq.pet().is_none() {
            return;
        }
        let pet = Pet { target: mq.pet_target().map(|target| target.id().0), taunt: mq.pet_commands().map(|commands| commands.taunt) };
        let Some(order) = decide(&self.config, &pet) else { return };
        if self.fight.pet_order.is_some_and(|(last, at)| last == order && at.elapsed() < PET_ORDER_GAP) {
            return;
        }
        let command = match order {
            PetOrder::Attack => PetCommand::Attack,
            PetOrder::BackOff => PetCommand::BackOff,
            PetOrder::Taunt(on) => PetCommand::Taunt(on),
        };
        if mq.pet_command(command) {
            self.fight.pet_order = Some((order, Instant::now()));
        }
    }

    /// Follows `spawn` with MQ2AdvPath, or stops following for `None`; sends
    /// `/afollow` only when that changes.
    pub fn follow(&mut self, mq: &Mq<Self>, spawn: Option<u32>) {
        if spawn == self.fight.followed {
            return;
        }
        if !mq.is_plugin_loaded(ADVPATH) {
            self.fight.followed = None;
            if spawn.is_some() && !std::mem::replace(&mut self.fight.warned_follow, true) {
                mq.chat(yellow!("[Bots] load {ADVPATH} to follow the MA"));
            }
            return;
        }
        let sent = match spawn {
            Some(id) => mq.command(format!("/afollow spawn {id}")),
            None => mq.command("/afollow off"),
        };
        self.fight.followed = spawn.filter(|_| sent);
    }

    /// Casts as a task; one at a time.
    fn cast(&mut self, mq: &Mq<Self>, name: &str, target: Option<u32>) {
        if self.fight.task.is_some() {
            return;
        }
        // Moving breaks a cast off; the next look follows again once it's done.
        self.follow(mq, None);
        let Some((_, spell)) = memorized(mq, name) else { return };
        let mut cast = Cast::new(spell.id());
        if let Some(target) = target {
            cast = cast.on(SpawnId(target)).restoring_target();
        }
        let name = name.to_owned();
        self.fight.task_spell = Some(name.clone());
        self.fight.task = Some(mq.spawn_task(move |task: Task<Bots>| async move {
            let result = task.cast(cast).await;
            task.with_plugin(|bots, mq| bots.cast_ended(mq, &name, target, result));
        }));
    }

    /// Says how a cast that didn't go off ended, once until another does.
    fn cast_ended(&mut self, mq: &Mq<Self>, name: &str, target: Option<u32>, result: CastResult) {
        self.fight.task = None;
        self.fight.task_spell = None;
        self.fight.landed.retain(|(_, _, at)| at.elapsed() < LANDED_HOLD);
        self.fight.blocked.retain(|(_, _, at)| at.elapsed() < BLOCKED_HOLD);
        // A resisted debuff isn't on the mob: it's cast again when ready.
        if result.is_landed() {
            self.fight.landed.push((name.to_owned(), target, Instant::now()));
        } else if result == CastResult::DidNotTakeHold {
            self.fight.blocked.push((name.to_owned(), target, Instant::now()));
        }
        if let Some(mob) = target.filter(|target| Some(*target) == self.state.engaged) {
            self.count_resists(mq, name, mob, result);
        }
        let failure = (!result.completed()).then(|| (name.to_owned(), result));
        if failure.is_some() && failure != self.fight.last_failure {
            mq.chat(yellow!("[Bots] {name}: {result}"));
        }
        self.fight.last_failure = failure;
    }

    /// Counts a debuff's or DoT's resists in a row on the mob, and says once
    /// when the mob has resisted it `MaxResists` times, or is immune, so
    /// it's given up on that mob.
    fn count_resists(&mut self, mq: &Mq<Self>, name: &str, mob: u32, result: CastResult) {
        let resistible = ["debuff", "dot"].iter().any(|archetype| self.config.archetype(archetype).iter().any(|spell| spell == name));
        let outcome = match result {
            CastResult::Landed => Outcome::Landed,
            CastResult::Resisted => Outcome::Resisted,
            CastResult::Immune => Outcome::Immune,
            _ => return,
        };
        if !resistible {
            return;
        }
        let before = self.fight.resists.count(name, mob);
        let after = self.fight.resists.record(name, mob, outcome);
        let max = self.config.max_resists;
        if before < max && after >= max {
            let mob = mq.spawn(SpawnId(mob)).map_or_else(|| "the mob".to_owned(), |spawn| spawn.name());
            let why = if outcome == Outcome::Immune { "it's immune".to_owned() } else { format!("resisted {max} times in a row") };
            mq.chat(yellow!("[Bots] giving up on {name} on {mob}: {why}"));
        }
    }

    /// Memorizes as a task, one gem (numbered from 1) at a time.
    fn memorize(&mut self, mq: &Mq<Self>, gem: usize, entry: &str) {
        if self.fight.task.is_some() {
            return;
        }
        let level = mq.me().map_or(0, |me| me.level());
        let Some(spell) = mq.spell_book().map(|(_, spell)| spell).find(|spell| same_name(entry, &spell.name()) && castable(mq, spell, level)) else { return };
        let (id, name, entry) = (spell.id(), spell.name(), entry.to_owned());
        self.fight.task = Some(mq.spawn_task(move |task: Task<Bots>| async move {
            let result = task.memorize(gem - 1, id).await;
            task.with_plugin(|bots, mq| bots.memorize_ended(mq, gem, &name, &entry, result));
        }));
    }

    /// `name` is the game's, `entry` the INI's.
    fn memorize_ended(&mut self, mq: &Mq<Self>, gem: usize, name: &str, entry: &str, result: MemorizeResult) {
        self.fight.task = None;
        let why = match result {
            MemorizeResult::Memorized => return mq.chat(green!("[Bots] memorized {name} in gem {gem}")),
            MemorizeResult::Refused => "the game wouldn't",
            MemorizeResult::TimedOut => "it took too long",
        };
        mq.chat(yellow!("[Bots] couldn't memorize {name} in gem {gem} ({why}), so the next best is used until /bots reload"));
        self.fight.given_up_spells.push(entry.to_owned());
    }

    /// Stops the bot's cast or memorizing, for a character leaving.
    pub fn stop_task(&mut self, mq: &Mq<Self>) {
        self.fight.task_spell = None;
        if let Some(task) = self.fight.task.take() {
            mq.cancel_task(task);
        }
    }

    pub fn disengage(&mut self, mq: &Mq<Self>) {
        let Some(id) = self.state.engaged.take() else { return };
        self.order_pet(mq, |_, pet| on_disengage(pet));
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

/// Whether an auto-hater slot of the extended target window holds a living
/// spawn in the zone: something hates you or the group.
fn haters(mq: &Mq<Bots>) -> bool {
    let alive = |id| mq.spawn(id).is_some_and(|spawn| !spawn.is_corpse());
    mq.xtargets().any(|slot| slot.role() == XTARGET_AUTO_HATER && slot.status() == XTargetStatus::InZone && slot.spawn_id().is_some_and(alive))
}

/// The group's Main Assist, if it's in the zone.
fn main_assist<'a>(mq: &'a Mq<Bots>) -> Option<Spawn<'a>> {
    mq.group()?.with_role(GroupRole::MainAssist)?.spawn()
}

/// Your mana as a percent of your most; `None` without mana.
fn mana_percent(mq: &Mq<Bots>) -> Option<i32> {
    match (mq.mana_current(), mq.mana_max()) {
        (Some(current), Some(max)) if max > 0 => Some((i64::from(current) * 100 / i64::from(max)) as i32),
        _ => None,
    }
}

fn mob(spawn: &Spawn<'_>, me: &Spawn<'_>) -> Mob {
    Mob { id: spawn.id().0, is_npc: spawn.is_npc(), hp_percent: spawn.hp_percent(), distance: spawn.distance_to(me) }
}

/// The spell by the INI's name in one of your gems, with its gem (0-based).
fn memorized<'a>(mq: &'a Mq<Bots>, name: &str) -> Option<(usize, Spell<'a>)> {
    let count = mq.gem_count();
    mq.gems().find(|(gem, spell)| *gem < count && same_name(name, &spell.name()))
}

/// Whether your class can cast `spell` at `level`.
fn castable(mq: &Mq<Bots>, spell: &Spell<'_>, level: u8) -> bool {
    mq.spell_level(spell).is_some_and(|needed| needed <= level)
}

/// A group member as the class sees it.
struct Mate {
    member: Member,
    /// Buffs and songs by spell ID, with time left (`Duration::MAX` for one
    /// that doesn't run out); `None` if they aren't seen (not in Posse).
    buffs: Option<Vec<(i32, Duration)>>,
    /// Its pet's, the same way.
    pet_buffs: Option<Vec<(i32, Duration)>>,
}

/// The game as the class sees it, against the target.
struct Seeing<'a> {
    mq: &'a Mq<Bots>,
    me: &'a Spawn<'a>,
    /// Auras taken as up, the bot's cast, and spells landed lately.
    fight: &'a Fight,
    group: Vec<Mate>,
    /// The mob being fought, if any.
    mob: Option<Mob>,
}

impl<'a> Seeing<'a> {
    fn new(mq: &'a Mq<Bots>, me: &'a Spawn<'a>, fight: &'a Fight, group: Vec<Mate>, mob: Option<Mob>) -> Seeing<'a> {
        Seeing { mq, me, fight, group, mob }
    }

    /// Whether `spell` just landed on `spawn`, or wouldn't take hold on it,
    /// so it's taken as on it until it's seen.
    fn held(&self, spell: &str, spawn: u32) -> bool {
        let held = |casts: &[(String, Option<u32>, Instant)], hold: Duration| {
            casts.iter().any(|(name, target, at)| name == spell && target.is_none_or(|target| target == spawn) && at.elapsed() < hold)
        };
        held(&self.fight.landed, LANDED_HOLD) || held(&self.fight.blocked, BLOCKED_HOLD)
    }
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
        auras.iter().any(|aura| names.iter().any(|name| aura_named(name, &aura.name))) || self.fight.given_up_auras.iter().any(|given_up| same_name(name, given_up))
    }

    fn disc_buff_left(&self, name: &str) -> Option<Duration> {
        let ability = self.mq.combat_abilities().find(|ability| same_name(name, &ability.name()))?;
        // Its own spell, or else a buff by the INI's name.
        let buff = self.mq.buff(ability.id()).or_else(|| self.mq.buffs().chain(self.mq.songs()).find(|buff| same_name(name, &buff.name())));
        buff?.remaining()
    }

    fn in_melee_range(&self) -> bool {
        self.mq.target().is_some_and(|target| self.me.distance_to(&target) <= self.mq.melee_range(self.me, &target) * REACH_PERCENT / 100.0)
    }

    fn spell_known(&self, name: &str) -> bool {
        self.mq.spell_book().any(|(_, spell)| same_name(name, &spell.name()) && castable(self.mq, &spell, self.me.level()))
    }

    fn spell_kind(&self, name: &str) -> Option<i32> {
        let spell = self.mq.spell_book().map(|(_, spell)| spell).find(|spell| same_name(name, &spell.name()))?;
        Some(spell.subcategory()).filter(|kind| *kind != 0)
    }

    fn gem_count(&self) -> usize {
        self.mq.gem_count()
    }

    fn spell_gem(&self, name: &str) -> Option<usize> {
        memorized(self.mq, name).map(|(gem, _)| gem + 1)
    }

    fn spell_ready(&self, name: &str) -> bool {
        let Some((gem, spell)) = memorized(self.mq, name) else { return false };
        self.mq.gem_ready(gem) && self.mq.mana_current().is_some_and(|mana| mana >= spell.mana_cost())
    }

    fn casting(&self) -> bool {
        self.fight.task.is_some() || self.me.casting().is_some()
    }

    fn bot_casting(&self) -> Option<String> {
        self.fight.task_spell.clone()
    }

    fn members(&self) -> Vec<Member> {
        self.group.iter().map(|mate| mate.member.clone()).collect()
    }

    fn buff_left(&self, spawn: u32, spell: &str) -> Option<Duration> {
        // Just landed, or wouldn't take hold: taken as on, with all its time.
        if self.held(spell, spawn) {
            return Some(Duration::MAX);
        }
        let buffs = self.group.iter().find_map(|mate| match mate.member.pet {
            _ if mate.member.id == spawn => Some(&mate.buffs),
            Some(pet) if pet == spawn => Some(&mate.pet_buffs),
            _ => None,
        });
        let Some(Some(buffs)) = buffs else { return Some(Duration::MAX) };
        let id = self.mq.spell_book().map(|(_, book)| book).find(|book| same_name(spell, &book.name()))?.id().0;
        buffs.iter().find(|(buff, _)| *buff == id).map(|(_, left)| *left)
    }

    fn in_range(&self, spell: &str, spawn: u32) -> bool {
        if spawn == self.me.id().0 {
            return true;
        }
        let Some((_, spell)) = memorized(self.mq, spell) else { return false };
        let reach = if spell.range() > 0.0 { spell.range() } else { spell.ae_range() };
        // The game won't cast on a spawn it can't see.
        self.mq.spawn(SpawnId(spawn)).is_some_and(|spawn| (reach <= 0.0 || self.me.distance_to(&spawn) <= reach) && self.mq.line_of_sight(&spawn))
    }

    fn mana_percent(&self) -> i32 {
        mana_percent(self.mq).unwrap_or(0)
    }

    fn mob(&self) -> Option<Mob> {
        self.mob
    }

    fn on_mob(&self, spell: &str) -> Option<bool> {
        let mob = self.mob?.id;
        if self.held(spell, mob) {
            return Some(true);
        }
        // Its buffs are seen only while it's targeted.
        if self.mq.target().is_none_or(|target| target.id().0 != mob) || !self.mq.target_buffs_populated() {
            return None;
        }
        let me = self.me.name();
        Some(self.mq.target_buffs().any(|buff| same_name(spell, &buff.name()) && buff.caster().eq_ignore_ascii_case(&me)))
    }

    fn resists(&self, spell: &str) -> u8 {
        self.mob.map_or(0, |mob| self.fight.resists.count(spell, mob.id))
    }
}
