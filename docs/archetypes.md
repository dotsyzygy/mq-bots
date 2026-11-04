# Archetypes

An archetype is a kind of thing a class does, such as a heal or a kick. The code never names spells or abilities; it asks for an archetype, and your INI lists what fills it. Each archetype is an INI section of numbered entries, best first:

```ini
[heal]
1=Superior Healing
2=Greater Healing
3=Healing
```

- The bot uses the first entry the character has and can use at their level, so one list covers every level. List everything from best to worst, including spells you don't have yet.
- Names match without the rank: `Ashenhand Discipline` matches `Ashenhand Discipline Rk. II`. Case doesn't matter.
- Archetypes written with `#` are numbered lines: `dot.1`, `dot.2`, `dot.3`... Each is its own section with its own list, used in number order.
- A section the class doesn't read (usually a typo) is reported in chat on load and on `/bots reload`.
- `[Settings]` isn't an archetype; see the README.

## Monk

| Archetype | What goes in it | Used yet? |
|---|---|---|
| `ability.offense` | Kicks and strikes from the abilities page (Flying Kick, Tiger Claw...), used whenever ready and in range | Yes |
| `disc.regular` | Short discs for normal fighting, used when no ability is ready | Yes |
| `aura` | Auras, best first; out of combat the best one you have is kept up | Yes |
| `disc.burn` | Big discs, only during `/bots burn` | Milestone 5 |
| `aa.burn` | AAs, only during `/bots burn` | Milestone 5 |
| `mend` | Self-heal below an HP% | Milestone 5 |
| `fd` | Feign Death when you have aggro and low HP | Milestone 5 |

## Shaman

Listed most important first. The shaman keeps the best spell it knows of each one memorized, one gem each; if there are more than gems, the ones lower down are left out, and chat says which.

| Archetype | What goes in it | Used yet? |
|---|---|---|
| `heal` | Single-target heal | Memorized; cast from S3 |
| `heal.hot` | Single-target heal over time, kept on the tank | Memorized; cast from S3 |
| `heal.group` | Group heal over time | Memorized; cast from S3 |
| `buff.melee` | Short melee buffs (Spirit of the Panther), on melee and pets in combat | Memorized; cast from S4 |
| `debuff.#` | One debuff line each (`debuff.1` slow, `debuff.2` malo, `debuff.3` cripple...), kept on the mob in number order | Memorized; cast from S5 |
| `dot.#` | One DoT line each, the same way | Memorized; cast from S5 |
| `nuke` | Direct damage | Memorized; cast from S5 |

Example: `MQ2Bots_<server>_Shaman.ini`

```ini
[heal]
1=Superior Healing
2=Greater Healing

[debuff.1]
1=Turgur's Insects
2=Togor's Insects

[debuff.2]
1=Malosini
2=Malo

[nuke]
1=Ice Strike
```
