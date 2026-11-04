# MQ2Bots

A MacroQuest plugin that runs your boxed EverQuest characters as bots: they assist your main character, use their abilities and follow you. One plugin covers every class; spells and abilities are mapped per server in an INI file.

> **Status: early.** The `/bots` commands and INI loading work; bots don't act yet. The MVP is a monk. See [docs/plan.md](docs/plan.md).

It's built on [mq-emu](https://github.com/dotsyzygy/mq-emu-rs), so it runs only on emulator servers with the RoF2 client (MacroQuest's `emu` branch, 32-bit). It uses [MQ2Posse](https://github.com/dotsyzygy/mq-posse) for shared status between clients on one PC.

## Building

```
cargo test -p bots-core
cargo mq deploy
```

Then `/plugin MQ2Bots` in game.

## Commands

| Command | Effect |
|---|---|
| `/bots` | Show this bot's mode, flags and archetypes |
| `/bots help` | Show the usage |
| `/bots mode <off\|manual\|assist>` | Set this bot's mode |
| `/bots attack` | Engage the MA's target now, ignoring assist-at (not built yet) |
| `/bots backoff` / `/bots resume` | Disengage and stop auto-engaging / start again |
| `/bots burn [off]` | Start or end a burn phase |
| `/bots follow [on\|off]` | Follow the driver; alone, it toggles |
| `/bots reload` | Reread the INI; what commands set is kept |

Send them to every bot with `/pc` through Posse.

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
```

Monk archetypes: `ability.offense`, `disc.regular`, `disc.burn`, `aa.burn`, `mend`, `fd` (see the plan).

## License

MIT.
