# MQ2Bots plan

Design decisions for the bot plugin, as agreed on 2026-11-02. The MVP is a monk; other classes come after it works.

## Setup this plan assumes

- You play one character by hand: a paladin who tanks and is the Main Assist (MA). Some of the paladin's actions (taunt, bash) will be automated later so you can swap to another character.
- Groups of 4, maybe up to 6. Raids come later.
- Every client runs on one PC, with MQ2Posse for fast shared status.

## Structure

One plugin, `MQ2Bots.dll`, built from a cargo workspace:

- `crates/bots-core`: the bot loop and state machine, assist and engage logic, config loading, reading Posse, `/bots` commands, and the action layer (one queue for abilities, discs, AAs, spells and items, with ready checks and cooldowns).
- One module per class, each implementing a class trait (something like `fn tick(&mut self, ctx: &Ctx) -> Option<Action>` plus its default archetypes). The plugin picks the module from `Me.Class` at load.
- Behaviors several classes share (heals, slows, mezzes, pets) live in core as reusable parts, not copied into each class.

Why one plugin: a character is only ever one class, so separate DLLs would mean loading the right one on each character, registering the shared commands many times, and redeploying them all on every core change.

## Rule: game access goes through mq-emu

Everything the bot reads or does in game goes through mq-emu's safe API. A missing getter or action is added to mq-emu and released first, never worked around in this plugin. Expected big item: cast-result tracking (success, fizzle, interrupt, resist...) as an mq-emu API, needed before any caster class.

## Config: archetypes in INI

The code never names spells, discs or abilities. It asks for an archetype, and the INI maps each archetype to an ordered list, best first. The bot uses the first entry that is known and usable at the character's level, so one list covers every level.

- Files: one per server and class (`MQ2Bots_<server>_<class>.ini`), plus an optional per-character override (`MQ2Bots_<server>_<name>.ini`).
- Each archetype is a section with numbered entries, so priority order is explicit.
- Later: `/bots genconfig` drafts an INI from the spellbook and ability list using spell categories, target types and effects. The INI stays the source of truth; the generator only saves typing.

Monk archetypes for the MVP (names may change while building):

| Archetype | Use |
|---|---|
| `ability.offense` | Kicks and strikes used whenever ready (Flying Kick, Tiger Claw...) |
| `disc.regular` | Short discs used during normal DPS |
| `aura` | Auras, best first; out of combat the best one the monk has is kept up (mq-emu 0.1.9 reads the aura window) |
| `disc.burn` | Big discs used only during a burn |
| `aa.burn` | AAs used only during a burn |
| `mend` | Self-heal below an HP% |
| `fd` | Feign Death when the monk has aggro and is low on HP |

## Combat flow

Engaging is automatic off the MA, with commands that override it.

- **Assist:** a bot in `assist` mode engages the MA's target when it is an NPC, at or below the assist-at HP%, and within the camp radius. It sticks behind the target (MQ2MoveUtils `/stick behind`) and turns on attack.
- **Regular vs burn:** in regular phase the bot uses `ability.offense` and `disc.regular`. `/bots burn` starts a burn phase: `disc.burn` and `aa.burn` are used in priority order. The burn lasts until the current combat is over (no group member is attacking and the extended target list has no haters) and then turns itself off; `/bots burn off` ends it early. Auto-burn conditions (named mob, target count) come later.
- **Backoff:** `/bots backoff` drops the target, turns attack off and stops auto-engaging until `/bots resume`.
- **Camp:** for now the camp is wherever the MA is, so the bots just follow the MA, and the camp radius is measured from the bot itself: a mob the MA is still pulling is left alone until it's brought in. Later, `deploy` and `packup` commands set and clear a static spot to camp at, and the camp radius is measured from there while deployed.
- **Feigning:** while feigned, a bot does nothing else. It still checks that it's feigning whenever FD still applies (aggro, low HP), and feigns again if something knocked it out, since AoE spells and the like sometimes do.
- **Out of combat:** follow the driver (MQ2AdvPath `/afollow`), with an on/off switch.

Modes per bot: `off` (does nothing), `manual` (only reacts to commands), `assist` (auto-engages). `tank` and `puller` come later.

## Commands (MVP)

| Command | Effect |
|---|---|
| `/bots mode <off\|manual\|assist>` | Set this bot's mode |
| `/bots attack` | Engage the MA's target now, ignoring assist-at |
| `/bots backoff` / `/bots resume` | Disengage and stop auto-engaging / start again |
| `/bots burn [off]` | Start or end a burn phase |
| `/bots follow [on\|off]` | Toggle following the driver |
| `/bots reload` | Reread the INI |

Sent to every bot with `/pc` through Posse.

## Posse

- Bots read Posse's shared memory through `posse-core` directly (typed, no string parsing, no passthrough lag). The `${Posse}` TLO stays for macros and Lua.
- Live status fields are added only when a bot needs them, and each needs an mq-emu getter first. Monk MVP needs from the MA: target ID, target HP% (`target_hp_percent`, added for milestone 4: a bot's own copy of a mob it hasn't targeted lags behind the server's), auto attack (the `COMBAT` flag), position. From each member: HP%, target ID. All are already published.
- Bots compute a "ready" flag (not medding, resources above threshold, not dead) and publish it through Posse for later pulling logic.

## Out of scope for the MVP

- A `${Bots}` TLO, until something needs it.
- A HUD or ImGui panels; a driver HUD comes later.
- Casters other than the shaman, raids, pulling, MQ2Nav.
- A static camp (`deploy` / `packup`); the camp is the MA until then.

## Shaman

Started before monk milestone 5. Long-duration buffs are left out for now: telling whether someone else's better buff is already on a target is its own problem.

The tank is the group member with the `MainTank` role, or the MA if no one has it.

No melee by default: the shaman never sticks to a mob or turns on attack. It stays back where it is, following the driver out of combat, and only casts. An INI switch for melee can come later.

Archetypes:

| Archetype | Use |
|---|---|
| `heal` | Single-target heal |
| `heal.hot` | Single-target heal over time, kept on the tank |
| `heal.group` | Group heal over time |
| `buff.melee` | Short melee buffs (Spirit of the Panther), on melee members and pets in combat |
| `debuff.1`, `debuff.2`... | One debuff line each (slow, malo, cripple...), each best first; kept up in number order |
| `dot.1`, `dot.2`... | One DoT line each, the same way |
| `nuke` | Direct damage |

Each pulse the shaman does the first of these that applies:

1. **Emergency heal:** the tank below `HealAt`, or anyone else below `EmergencyAt`: `heal` on the lowest, the tank winning ties. A cast that isn't a heal is stopped for it.
2. **Group HoT:** `GroupHotCount` (default 2) or more members below `GroupHotAt`, and the group HoT isn't on them: `heal.group`.
3. **Tank HoT:** while the tank is fighting, keep `heal.hot` on it.
4. **Melee buffs:** in combat, `buff.melee` on each melee-class member (WAR, PAL, SHD, MNK, ROG, BER, RNG, BST, BRD) and each member's pet, the tank first, when it's missing or under `RebuffAt` seconds left.
5. **Debuffs, then DoTs:** on the MA's target while engaged, each line that isn't on it from this shaman, while mana is above `DpsManaMin` and the mob is above `DotStopAt` HP%. After `MaxResists` resists in a row, a line is given up on that mob (slow-immune mobs).
6. **Nuke:** while mana is above `NukeManaMin`.

How it sees things:

- **Members' buffs:** Posse already publishes each member's buffs and songs with time left. A buff that just landed shows up a pulse or so later, so after a cast lands the shaman holds that line on that target briefly instead of casting it twice.
- **Pets' buffs:** a new Posse field. Each owner publishes its pet's buffs from its pet window (mq-emu `pet_buffs`, already bound).
- **The mob's debuffs and DoTs:** `target_buffs` with each buff's caster, once `target_buffs_populated` says they've arrived.
- **Casting on a member:** target it, cast, then go back to the MA's target.

Gems: a `[Gems]` section maps gem numbers to archetypes (`1=heal`, `2=heal.hot`...). Out of combat, on load and on `/bots reload`, the shaman memorizes the best known spell of each archetype into its gem, so one INI works at every level.

Later: long-duration buffs, canni and medding, the shaman's own pet.

## MVP milestones

1. ~~Fill the mq-emu gaps for monk (see API gaps) and release mq-emu.~~ Done in mq-emu 0.1.6.
2. ~~Confirm Posse's `COMBAT` flag meaning; no new Posse fields expected for the monk.~~ Done: it's auto attack (`${Me.Combat}`), which is what "attacking" means in the combat flow.
3. ~~Scaffold the workspace, `/bots` commands and INI loading.~~ Done. `bots-core` holds what doesn't touch the game (command parsing, config layering, the bot's state) and is tested off-game, like `posse-core`; the plugin crate does the game access. Commands set state only; acting on it starts in milestone 4.
4. ~~Monk: assist, stick, attack, abilities and regular discs.~~ Done, and tried in game. `bots-core` decides what to engage (`engage.rs`) and what the monk uses (`class/monk.rs`, against a `Game` trait the plugin implements), so both are tested off-game. The camp radius is measured from the bot, so a mob being pulled waits until it's brought in. mq-emu 0.1.7 added what this needed: `set_auto_attack` and `melee_range` (`${Target.MaxRangeTo}`, so abilities wait until in range). posse-core now holds MQ2Posse's settings names and `auto_join_posse`, so the bots find the same posse Posse joins. Disengaging on `/bots backoff` came with it, so milestone 6 leaves following and the group test.
5. Burn phase, Mend, Feign Death (including feigning again when knocked out of it while FD still applies).
6. Follow and backoff. Test in a group of 4.

Shaman milestones, started before monk milestone 5:

- S1. ~~mq-emu: the safe cast API (see API gaps), then release. Posse: pets' buffs.~~ Done: mq-emu 0.1.10 (`Task::cast`, `Task::memorize`, `CastResult::completed`, `Task::watch_messages`, `cast_time_left`, through `eqclient.dll`, which `cargo mq deploy` ships next to the plugin) and MQ2Posse 0.1.1 (`pet_buffs`, layout 6). Tried in game: a buff cast lands exactly when `Task::cast` says, a stronger buff already on gives `DidNotTakeHold`, and a stopped cast's late messages don't count against the next one.
- S2. Casting in `bots-core` (a `Cast` action, gem and cast state in `Game`) and memorizing `[Gems]` out of combat.
- S3. Heals: emergency heals, group HoT, tank HoT, stopping a non-heal cast for a heal.
- S4. Melee buffs on members and pets.
- S5. Debuffs, DoTs and nukes, with mana floors and giving up after resists.
- S6. Try it in game with the group.

## API gaps

From an audit of mq-emu 0.1.4 and MQ2Posse on 2026-11-02. Already typed and enough for the monk: Me's HP, level, class, sitting, feigning, stunned and casting; endurance; abilities and their readiness (`Skill::is_ready`); AAs and their readiness; target spawn, HP%, type, distance, heading and position; group members and roles (`GroupRole::MainAssist`); extended targets with aggro; spell category, subcategory, target type and effects (for `genconfig`); INI; pulse, commands, events and async tasks.

Missing from mq-emu, to bind before the monk (milestone 1):

| Gap | Today | Needed |
|---|---|---|
| Combat abilities and discs | done in mq-emu 0.1.5 (`combat_abilities`, `use_combat_ability`, `active_disc`); names include the rank, so INI lists will match without it | |
| Triggering an ability | done in mq-emu 0.1.6 (`use_ability`) | |
| Activating an AA | done in mq-emu 0.1.6 (`use_alt_ability`) | |
| Aggro | done in mq-emu 0.1.6 (`aggro_percent`, `secondary_aggro_percent`, `secondary_aggro_player`, `aggro_lock`, `target_of_target`); `target_of_target` has only been seen empty on this client, and `${Target.AggroHolder}` isn't bound | |
| Setting the target | done in mq-emu 0.1.6 (`set_target`) | |
| Combat state | done in mq-emu 0.1.6 (`combat_state`; `in_combat` is deprecated for `is_auto_attacking`) | |
| Small helpers | done in mq-emu 0.1.6 (`endurance_percent`, `Spawn::is_standing`) | |
| Is a plugin loaded | done in mq-emu 0.1.6 (`is_plugin_loaded`, by any name `GetPlugin` takes) | |

For the shaman (S1): a safe cast API in mq-emu. mq-emu has `Spawn::casting` polling, but nothing that reports how a cast ended; MacroQuest itself has none either, and MQ2Cast reads outcomes from chat text, which is unreliable and not used here. Instead the new `eq-client` crate hooks the client's `StringTable::getString` from a shared `eqclient.dll`, so mq-emu sees the client's messages by eqstr_us.txt ID (fizzle 173, resist 425, interrupt 439...), whether the server or the client itself shows them. It goes into mq-emu, not a separate plugin: a plugin like MQ2Cast is driven by commands and read back through a TLO string, the lag and parsing the bots avoid by reading Posse directly, and only the bot casts for its character, so a shared queue isn't needed.

The split: mq-emu does how to cast; the bots decide what and when (thresholds, priorities, resist counting, `[Gems]`).

- `mq.cast(spell, target)` is an async task that always ends with a result: landed, resisted, interrupted, fizzled, out of range, no line of sight, not enough mana, not memorized, or timed out.
- It checks first, without touching the game: spell known and in a gem, gem ready, mana, target exists, is alive and in range; not already casting, stunned or feigning. A failed check returns its reason.
- It does the routine steps: stands if sitting, targets the spell's target (optionally going back to the previous one), casts from the gem.
- It always finishes: it tracks the cast through the casting state and the client's messages by ID (fizzle, interrupt, resist, can't see target), with a timeout so a lost message can't hang a bot.
- Cancelling the task stops the cast, which is what an emergency heal needs.
- Memorizing gets the same shape: a task that ends once the spell is in its gem and ready.

Done in mq-emu 0.1.10: `task.cast(Cast::new(spell).on(spawn))` ends with a `CastResult`, `task.memorize(gem, spell)` with a `MemorizeResult`, and `task.watch_messages(&ids)` gives any task the client's messages by ID. Everything else the shaman needs is bound: casting from a gem, gem readiness, memorizing, stopping a cast, mana, target and pet buffs with casters, `knows_spell` and `spell_level`.

Posse needed one new field for the monk, `target_hp_percent` (layout 5): a bot's client had a pulled mob at 95% while the MA's showed it higher, so the MA's view is used. Its live status already had `target_id`, `hp_percent`, position, and `COMBAT` and `FEIGNING` flags; the bot no longer reads the MA's target's HP% from the spawn locally. The ready flag comes later. The bots read it with `posse_shm::Mapping::open` and `posse_core::Roster` (`refresh`, `read` by name), linked as path dependencies, since both crates are `publish = false`. The `COMBAT` flag is auto attack (`${Me.Combat}`), published from mq-emu's `in_combat`, not the combat state; if a bot needs a member's combat state, that's a new Posse field.
