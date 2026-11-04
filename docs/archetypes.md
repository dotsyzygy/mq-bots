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
- A few archetypes (marked *each kind* below) use more than one entry: the best one you have of each kind of spell, in list order. The kind is the game's own (the spell's subcategory), so `[debuff]` keeps one slow, one malo and one cripple, and `[dot]` one poison and one disease DoT, and you still list every rank. A spell the game gives no kind counts as its own.
- A section the class doesn't read (usually a typo) is reported in chat on load and on `/bots reload`.
- `[Settings]` isn't an archetype; see the README.

## Monk

| Archetype | What goes in it | Used yet? |
|---|---|---|
| `ability.offense` | Kicks and strikes from the abilities page (Flying Kick, Tiger Claw...), used whenever ready and in range | Yes |
| `disc.regular` | Short discs for normal fighting, used when no ability is ready | Yes |
| `aura` | Auras, best first; out of combat the best one you have is kept up | Yes |
| `disc.burn` | Big discs, only during `/bots burn` | Milestone 1 |
| `aa.burn` | AAs, only during `/bots burn` | Milestone 1 |
| `mend` | Self-heal below an HP% | Milestone 1 |
| `fd` | Feign Death when you have aggro and low HP | Milestone 1 |

## Shaman

Listed most important first. The shaman keeps memorized what it uses of each, one gem per spell; if there are more spells than gems, the ones lower down are left out, and chat says which.

| Archetype | What goes in it | Used yet? |
|---|---|---|
| `heal` | Single-target heal | Cast |
| `heal.hot` | Single-target heal over time, kept on the tank | Cast |
| `heal.group.hot` | Group heal over time | Cast |
| `buff.melee` | Short melee buffs (Spirit of the Panther), in a fight, on melee-class members (the tank first), then their pets, when missing or under `RebuffAt` seconds left | Cast |
| `debuff` | Slows, malos, cripples..., *each kind*, kept on the mob in list order | Memorized; cast from S1 |
| `dot` | DoTs, *each kind* (poison, disease), the same way | Memorized; cast from S1 |
| `nuke` | Direct damage | Memorized; cast from S1 |

`heal.group` (a direct group heal, no HoT) is a different archetype, for classes that have one; the shaman doesn't.

Example: `MQ2Bots_<server>_Shaman.ini`

```ini
[heal]
1=Superior Healing
2=Greater Healing

; The slow comes first, so it goes on before the malo.
[debuff]
1=Turgur's Insects
2=Togor's Insects
3=Malo
4=Malosini

[dot]
1=Bane of Nife
2=Envenomed Bolt
3=Pox of Bertoxxulous
4=Plague

[nuke]
1=Ice Strike
```
