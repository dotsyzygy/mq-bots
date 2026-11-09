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

- Files: one per character (`MQ2Bots_<server>_<name>.ini`), holding its archetypes and `[Settings]`. No per-class file: two characters of one class may want different spells (only one magician debuffs), and copying a file is easy.
- Each archetype is a section with numbered entries, so priority order is explicit.
- Later: `/bots genconfig` drafts an INI from the spellbook and ability list using spell categories, target types and effects. The INI stays the source of truth; the generator only saves typing.

Monk archetypes still to use: `disc.burn` and `aa.burn` (only during a burn), `mend` (self-heal below an HP%), `fd` (Feign Death when the monk has aggro and is low on HP).

## Combat flow

Engaging is automatic off the MA, with commands that override it.

- **Regular vs burn:** `/bots burn` starts a burn phase: `disc.burn` and `aa.burn` are used in priority order. The burn lasts until the current combat is over (no group member is attacking and the extended target list has no haters) and then turns itself off; `/bots burn off` ends it early. Auto-burn conditions (named mob, target count) come later.
- **Camp:** for now the camp is wherever the MA is. Later, `deploy` and `packup` commands set and clear a static spot to camp at, and the camp radius is measured from there while deployed.
- **Feigning:** while feigned, a bot does nothing else. It still checks that it's feigning whenever FD still applies (aggro, low HP), and feigns again if something knocked it out, since AoE spells and the like sometimes do.
- **Out of combat:** follow the Main Assist (MQ2AdvPath `/afollow`), with an on/off switch. Following stops while engaged, casting, memorizing or feigning, and in mode off.

Modes `tank` and `puller` come later.

## Posse

- Live status fields are added only when a bot needs them, and each needs an mq-emu getter first.
- Bots compute a "ready" flag (not medding, resources above threshold, not dead) and publish it through Posse for later pulling logic.

## Out of scope for the MVP

- A `${Bots}` TLO, until something needs it.
- A HUD or ImGui panels; a driver HUD comes later.
- Casters other than the shaman and the magician, raids, pulling, MQ2Nav.
- A static camp (`deploy` / `packup`); the camp is the MA until then.

## Shaman

Heals, melee buffs, memorizing, debuffs, DoTs and nukes are done. Long-duration buffs are left out for now: telling whether someone else's better buff is already on a target is its own problem.

Known problem, melee buffs: a buff that lands but is then overwritten by another buff on the target is cast again every time it's ready, forever. Seen with Champion, a group spell with a long cooldown. Its cast lands, so neither the 6-second hold nor the `DidNotTakeHold` hold applies, and once the hold ends Posse shows it missing. Fix idea: a buff that landed but isn't seen on the target after Posse has caught up was overwritten, so stop casting it on that target for a while (or until `/bots reload`) and say so in chat once. Related: a group buff like Champion should be cast once for the group (no target), when it's missing on enough melee members, not once per member.

Later: long-duration buffs (memorized for the cast in a spare gem, then swapped back), the shaman's own pet, an INI switch for melee.

Cannibalize, to come: the shaman's own way of trading health for mana (the Cannibalize spells, and the Cannibalization AA). Cast at about 50% mana (a `CanniAt` setting), from a `canni` archetype that can list the AA and the spells, best first. It has to keep enough HP not to need healing itself, so it waits while HP is low and comes after heals.

## Magician

Planned 2026-11-09. The module, memorizing, `[debuff]`, `[nuke]` and line of sight are done. A caster with a pet: it summons and gears its own pet, gears every group member's pet, buffs its own pet, malos and nukes. It follows the caster rules the shaman does: no melee, stay out of melee range, keep line of sight.

Pets are a shared part in core, not magician code, so any class with a pet (necro, beastlord, shaman later) gets summoning, sending the pet in, and pet buffs from the same archetypes.

### Pet

- `[pet]`: pet summons, best first. Out of combat, with no pet, the bot summons the best one it has. A summon that fails for a missing reagent is said once in chat, not retried every pulse.
- In a fight the pet attacks the MA's target when the bot engages, and backs off on `/bots backoff`. Pet taunt is turned off, since the paladin tanks; a `PetTaunt` setting (saved by `/bots set`) turns it on for when a pet tanks.

### Pet gear

What a client sees of another spawn's gear is only its look: a weapon's model number in each hand and armor appearance, never item IDs, and nothing at all for rings, belts or masks. There's no pet inventory in the client. So the bot doesn't check what a pet wears; it remembers which pets it has geared, by spawn ID. A pet that dies and is summoned again has a new ID and gets geared again. The memory is lost on plugin reload, so after a reload every pet is geared once more; the extra items are harmless.

- `[pet.weapon]`: weapon summons, best first. The best one the bot has is summoned twice, one for each hand, since almost every pet dual wields.
- `[pet.gear]`: other gear summons (armor, mask, belt...), every entry the bot has, each once.
- Giving: summon onto the cursor, target the pet, left-click it to open the give window, click Give. The give window holds 4 items, so a pet's gear can go over in a few gives.
- Who: its own pet first, then each group member's pet, out of combat, not while moving or medding, and only pets in range.
- Spells that summon a folded pack or bag of gear are left out for now; to come if your gear spells turn out to give one.

### Pet buffs

- `[buff.pet]`: buffs only for the bot's own pet (Burnout, the magician's pet aura...), kept up in a fight and between fights, when missing or under `RebuffAt` seconds left. The pet's buffs are already read from the pet window.

### Damage

- `[debuff]`: the malo line, *each kind*, the shaman's way (`DpsManaMin`, `DotStopAt`, `MaxResists`).
- `[nuke]`: direct damage, once debuffs are on, above `NukeManaMin` mana.

## Milestones

Monk:

1. Burn phase, Mend, Feign Death (including feigning again when knocked out of it while FD still applies).

Magician:

1. Pet in core: `[pet]` summon out of combat, send the pet in on the MA's target, back off with `/bots backoff`, `PetTaunt` (off by default).
2. Pet gear on its own pet: `[pet.weapon]` best summoned twice, `[pet.gear]` each once, geared pets remembered by spawn ID.
3. Pet gear on group members' pets, in range, out of combat.
4. `[buff.pet]` on its own pet.
