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
| `disc.burn` | Big discs used only during a burn |
| `aa.burn` | AAs used only during a burn |
| `mend` | Self-heal below an HP% |
| `fd` | Feign Death when the monk has aggro and is low on HP |

## Combat flow

Engaging is automatic off the MA, with commands that override it.

- **Assist:** a bot in `assist` mode engages the MA's target when it is an NPC, at or below the assist-at HP%, and within the camp radius. It sticks behind the target (MQ2MoveUtils `/stick behind`) and turns on attack.
- **Regular vs burn:** in regular phase the bot uses `ability.offense` and `disc.regular`. `/bots burn` starts a burn phase: `disc.burn` and `aa.burn` are used in priority order. The burn lasts until the current combat is over (no group member is attacking and the extended target list has no haters) and then turns itself off; `/bots burn off` ends it early. Auto-burn conditions (named mob, target count) come later.
- **Backoff:** `/bots backoff` drops the target, turns attack off and stops auto-engaging until `/bots resume`.
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
- Live status fields are added only when a bot needs them, and each needs an mq-emu getter first. Monk MVP needs from the MA: target ID, auto attack (the `COMBAT` flag), position. From each member: HP%, target ID. All are already published.
- Bots compute a "ready" flag (not medding, resources above threshold, not dead) and publish it through Posse for later pulling logic.

## Out of scope for the MVP

- A `${Bots}` TLO, until something needs it.
- A HUD or ImGui panels; a driver HUD comes later.
- Casters, cast-result tracking in the bot, raids, pulling, MQ2Nav.

## MVP milestones

1. ~~Fill the mq-emu gaps for monk (see API gaps) and release mq-emu.~~ Done in mq-emu 0.1.6.
2. ~~Confirm Posse's `COMBAT` flag meaning; no new Posse fields expected for the monk.~~ Done: it's auto attack (`${Me.Combat}`), which is what "attacking" means in the combat flow.
3. ~~Scaffold the workspace, `/bots` commands and INI loading.~~ Done. `bots-core` holds what doesn't touch the game (command parsing, config layering, the bot's state) and is tested off-game, like `posse-core`; the plugin crate does the game access. Commands set state only; acting on it starts in milestone 4.
4. Monk: assist, stick, attack, abilities and regular discs.
5. Burn phase, Mend, Feign Death.
6. Follow and backoff. Test in a group of 4.

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

Later, for casters: cast-result tracking (mq-emu has chat events and `Spawn::casting` polling, but no API that reports a cast's outcome).

Posse needs nothing new for the monk. Its live status already has `target_id`, `hp_percent`, position, and `COMBAT` and `FEIGNING` flags; the bot reads the MA's target's HP% from the spawn locally. The ready flag comes later. The bots read it with `posse_shm::Mapping::open` and `posse_core::Roster` (`refresh`, `read` by name), linked as path dependencies, since both crates are `publish = false`. The `COMBAT` flag is auto attack (`${Me.Combat}`), published from mq-emu's `in_combat`, not the combat state; if a bot needs a member's combat state, that's a new Posse field.
