# MQ2Bots plan

Design decisions for the bot plugin, as agreed on 2026-11-02, and what's left to build. Done work is taken out of this file, and the milestones left are numbered again from 1.

## Setup this plan assumes

- You play one character by hand: a paladin who tanks and is the Main Assist (MA). Some of the paladin's actions (taunt, bash) will be automated later so you can swap to another character.
- Groups of 4, maybe up to 6. Raids come later.
- Every client runs on one PC, with MQ2Posse for fast shared status.

## Structure

One plugin, `MQ2Bots.dll`, built from a cargo workspace:

- `crates/bots-core`: what doesn't touch the game (commands, config, state, what to engage, each class's decisions against a `Game` trait), tested off-game.
- One module per class, each implementing the `Class` trait. The plugin picks the module from `Me.Class` at load.
- Behaviors several classes share (heals, slows, mezzes, pets) live in core as reusable parts, not copied into each class.

Why one plugin: a character is only ever one class, so separate DLLs would mean loading the right one on each character, registering the shared commands many times, and redeploying them all on every core change.

## Rule: game access goes through mq-emu

Everything the bot reads or does in game goes through mq-emu's safe API. A missing getter or action is added to mq-emu and released first, never worked around in this plugin. mq-emu does how to cast; the bots decide what and when (thresholds, priorities, resist counting).

## Config: archetypes in INI

The code never names spells, discs or abilities. It asks for an archetype, and the INI maps each archetype to an ordered list, best first. The bot uses the first entry that is known and usable at the character's level, so one list covers every level.

- Files: one per server and class (`MQ2Bots_<server>_<class>.ini`), plus an optional per-character override (`MQ2Bots_<server>_<name>.ini`).
- Each archetype is a section with numbered entries, so priority order is explicit.
- Later: `/bots genconfig` drafts an INI from the spellbook and ability list using spell categories, target types and effects. The INI stays the source of truth; the generator only saves typing.

Monk archetypes still to use: `disc.burn` and `aa.burn` (only during a burn), `mend` (self-heal below an HP%), `fd` (Feign Death when the monk has aggro and is low on HP).

## Combat flow

Engaging is automatic off the MA, with commands that override it.

- **Regular vs burn:** `/bots burn` starts a burn phase: `disc.burn` and `aa.burn` are used in priority order. The burn lasts until the current combat is over (no group member is attacking and the extended target list has no haters) and then turns itself off; `/bots burn off` ends it early. Auto-burn conditions (named mob, target count) come later.
- **Camp:** for now the camp is wherever the MA is. Later, `deploy` and `packup` commands set and clear a static spot to camp at, and the camp radius is measured from there while deployed.
- **Feigning:** while feigned, a bot does nothing else. It still checks that it's feigning whenever FD still applies (aggro, low HP), and feigns again if something knocked it out, since AoE spells and the like sometimes do.
- **Out of combat:** follow the driver (MQ2AdvPath `/afollow`), with an on/off switch.

Modes `tank` and `puller` come later.

## Posse

- Live status fields are added only when a bot needs them, and each needs an mq-emu getter first.
- Bots compute a "ready" flag (not medding, resources above threshold, not dead) and publish it through Posse for later pulling logic.

## Out of scope for the MVP

- A `${Bots}` TLO, until something needs it.
- A HUD or ImGui panels; a driver HUD comes later.
- Casters other than the shaman, raids, pulling, MQ2Nav.
- A static camp (`deploy` / `packup`); the camp is the MA until then.

## Shaman

Heals, melee buffs and memorizing are done. Long-duration buffs are left out for now: telling whether someone else's better buff is already on a target is its own problem.

Still to come, in the shaman's `fight`, after melee buffs:

1. **Debuffs, then DoTs:** on the MA's target while engaged, each line that isn't on it from this shaman, while mana is above `DpsManaMin` and the mob is above `DotStopAt` HP%. After `MaxResists` resists in a row, a line is given up on that mob (slow-immune mobs).
2. **Nuke:** while mana is above `NukeManaMin`.

The mob's debuffs and DoTs come from `target_buffs` with each buff's caster, once `target_buffs_populated` says they've arrived.

Known problem, melee buffs: a buff that lands but is then overwritten by another buff on the target is cast again every time it's ready, forever. Seen with Champion, a group spell with a long cooldown. Its cast lands, so neither the 6-second hold nor the `DidNotTakeHold` hold applies, and once the hold ends Posse shows it missing. Fix idea: a buff that landed but isn't seen on the target after Posse has caught up was overwritten, so stop casting it on that target for a while (or until `/bots reload`) and say so in chat once. Related: a group buff like Champion should be cast once for the group (no target), when it's missing on enough melee members, not once per member.

Later: long-duration buffs (memorized for the cast in a spare gem, then swapped back), medding, the shaman's own pet, an INI switch for melee.

Cannibalize, to come: the shaman's own way of trading health for mana (the Cannibalize spells, and the Cannibalization AA). Cast at about 50% mana (a `CanniAt` setting), from a `canni` archetype that can list the AA and the spells, best first. It has to keep enough HP not to need healing itself, so it waits while HP is low and comes after heals.

## Milestones

Monk:

1. Burn phase, Mend, Feign Death (including feigning again when knocked out of it while FD still applies).
2. Follow (backoff is done). Test in a group of 4.

Shaman:

- S1. Debuffs, DoTs and nukes, with mana floors and giving up after resists. Stopping a cast for a heal can be tried in game once these cast.
- S2. Try it in game with the group, melee buffs included (not tried in game yet).
