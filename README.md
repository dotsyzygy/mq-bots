# MQ2Bots

A MacroQuest plugin that runs your boxed EverQuest characters as bots: they assist your main character, use their abilities and follow you. One plugin covers every class; spells and abilities are mapped per server in an INI file.

> **Status: early.** A monk assists: it engages the Main Assist's target, sticks behind it, attacks, and uses its abilities and regular discs. Burns, Mend, Feign Death and following aren't built yet. Other classes come after the monk. See [docs/plan.md](docs/plan.md).

It's built on [mq-emu](https://github.com/dotsyzygy/mq-emu-rs), so it runs only on emulator servers with the RoF2 client (MacroQuest's `emu` branch, 32-bit). It uses [MQ2Posse](https://github.com/dotsyzygy/mq-posse) for shared status between clients on one PC.

## Building

It needs [mq2-posse-rs](https://github.com/dotsyzygy/mq-posse) checked out next to this repo, as `../mq2-posse-rs`, for Posse's crates.

```
cargo test -p bots-core
cargo mq deploy
```

Then `/plugin MQ2Bots` in game, with MQ2Posse, and MQ2MoveUtils for sticking.

## Commands

| Command | Effect |
|---|---|
| `/bots` | Show this bot's mode, flags and archetypes |
| `/bots help` | Show the usage |
| `/bots mode <off\|manual\|assist>` | Set this bot's mode |
| `/bots attack` | Engage the MA's target now, ignoring assist-at and the camp radius; it also resumes |
| `/bots backoff` / `/bots resume` | Disengage and stop auto-engaging / start again |
| `/bots burn [off]` | Start or end a burn phase |
| `/bots follow [on\|off]` | Follow the driver; alone, it toggles |
| `/bots reload` | Reread the INI; what commands set is kept |

Send them to every bot with `/pc` through Posse.

## Assisting

The Main Assist is whoever has the group's Main Assist role. Each bot reads the MA's target from Posse, so the MA's client must be in the same posse. In `assist` mode, a bot engages that target when it's an NPC at or below `AssistAt`% HP and within `CampRadius` of the MA, and switches when the MA moves on to another that qualifies. Engaging targets the mob, runs `/stick behind` and turns auto attack on. When the mob dies or leaves, or on `/bots backoff` or `mode off`, the bot turns attack off, stops sticking and clears its target.

In melee range, the monk uses the first ready ability in `ability.offense`, or else the first ready disc in `disc.regular` that it has the endurance for (and, if the disc has a duration, only while no other is running). Names match with or without the game's rank (`Rk. II`).

## Config

The plugin reads two files from MacroQuest's config folder when a character enters the game: `MQ2Bots_<server>_<class>.ini` (the class as `${Me.Class}` names it, such as `Monk`) and then, if it's there, `MQ2Bots_<server>_<character>.ini`. In the character's file, an archetype section replaces the class's whole list, and a setting replaces just that setting. Problems are reported in chat and skipped.

```ini
[Settings]
; off, manual (the default) or assist
Mode=assist
; Engage the MA's target at or below this HP% (default 95)
AssistAt=95
; Engage only within this distance of the camp (default 100)
CampRadius=100
; Start following the driver (default off)
Follow=off

; Every other section is an archetype: numbered entries, best first.
; The bot uses the first one the character has and can use.
[ability.offense]
1=Flying Kick
2=Tiger Claw

[disc.regular]
1=Thunderkick
```

Monk archetypes: `ability.offense`, `disc.regular`, `disc.burn`, `aa.burn`, `mend`, `fd` (see the plan).

## License

Copyright (C) 2026 Syzygy.

MQ2Bots is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 2 of the License, or (at your option) any later version. See [LICENSE](LICENSE). It's distributed in the hope that it will be useful, but without any warranty.

Anyone who distributes MQ2Bots, changed or not, must give the full source under the same license. Whoever receives it, paid or not, gets the same rights, including sharing it for free.
