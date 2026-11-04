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

The Main Assist is whoever has the group's Main Assist role. Each bot reads the MA's target from Posse, so the MA's client must be in the same posse. In `assist` mode, a bot engages that target when it's an NPC at or below `AssistAt`% HP and within `CampRadius` of the bot (so a mob the MA is still pulling is left alone until it's brought in), and switches when the MA moves on to another that qualifies. Engaging targets the mob, runs `/stick behind` and turns auto attack on. When the mob dies or leaves, or on `/bots backoff` or `mode off`, the bot turns attack off, stops sticking and clears its target.

In melee range, the monk uses the first ready ability in `ability.offense`, or else the first ready disc in `disc.regular` that it has the endurance for. A disc that puts a buff or song on the monk (Fists of Wu) waits until 5 seconds or less are left on it, and a disc with a long duration waits while another runs. Out of combat (not attacking and not in the combat state), it keeps the best aura in `aura` that it has up. Names match with or without the game's rank (`Rk. II`).

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
; Healers: heal the tank below this HP% (default 70)
HealAt=70
; Healers: heal anyone else below this HP% (default 50)
EmergencyAt=50
; Healers: the group HoT when GroupHotCount members (default 2) are below
; GroupHotAt% HP (default 85) without it
GroupHotAt=85
GroupHotCount=2
; Buffers: cast a buff again with this many seconds left on it (default 6)
RebuffAt=6

; Every other section is an archetype: numbered entries, best first.
; The bot uses the first one the character has and can use.
[ability.offense]
1=Flying Kick
2=Tiger Claw

[disc.regular]
1=Thunderkick
```

Each class's archetypes, what goes in them and which are used yet: [docs/archetypes.md](docs/archetypes.md). The shaman doesn't melee. So far it memorizes its spells and, in mode `assist`, heals, in a fight or out, only members its spell reaches: the lowest of the tank below `HealAt` and anyone else below `EmergencyAt` (stopping its own cast for it unless that's a heal), then the group HoT, then the HoT on the tank while it's attacking. The tank is whoever has the Main Tank role, or else the MA. HP and buffs come from Posse, so members who aren't in the posse get emergency heals but no HoTs. A section the class doesn't read, such as a misspelled one, is reported in chat on load and on `/bots reload`.

A caster memorizes its spells itself, out of combat, after loading and after `/bots reload`: for each spell archetype, the best spell in the spell book that's castable at the character's level, so one INI works at every level, and for `debuff` and `dot`, the best of each kind (one slow, one malo...; see [docs/archetypes.md](docs/archetypes.md)). Each gets a gem. A spell already memorized stays in its gem; the others go into gems holding nothing the bot uses. With more spells than gems, the class's most important ones get gems, and chat says which were left out. A spell the game won't memorize is reported, and the next best is used until `/bots reload`.

## License

Copyright (C) 2026 Syzygy.

MQ2Bots is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 2 of the License, or (at your option) any later version. See [LICENSE](LICENSE). It's distributed in the hope that it will be useful, but without any warranty.

Anyone who distributes MQ2Bots, changed or not, must give the full source under the same license. Whoever receives it, paid or not, gets the same rights, including sharing it for free.
