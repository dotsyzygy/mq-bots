//! The INI settings and archetype lists.
//!
//! An archetype is a section of numbered entries, best first:
//!
//! ```ini
//! [ability.offense]
//! 1=Flying Kick
//! 2=Tiger Claw
//! ```
//!
//! Every section but `[Settings]` is an archetype.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::Mode;

pub const SETTINGS_SECTION: &str = "Settings";

/// One INI file's sections, each with its entries in file order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IniFile {
    /// The file's name, for the problems found in it.
    pub name: String,
    pub sections: Vec<(String, Vec<(String, String)>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// The mode a character starts in (`Mode`).
    pub mode: Mode,
    /// Whether a character starts following the driver (`Follow`).
    pub follow: bool,
    /// Engage the MA's target at or below this HP% (`AssistAt`).
    pub assist_at: u8,
    /// Engage only within this distance of the camp (`CampRadius`).
    pub camp_radius: f32,
    /// Heal the tank below this HP% (`HealAt`).
    pub heal_at: u8,
    /// Heal anyone else below this HP% (`EmergencyAt`).
    pub emergency_at: u8,
    /// Members below this HP% count toward a group HoT (`GroupHotAt`).
    pub group_hot_at: u8,
    /// How many members below `GroupHotAt` it takes (`GroupHotCount`).
    pub group_hot_count: u8,
    /// Cast a buff again with this little left on it (`RebuffAt`, seconds).
    pub rebuff_at: Duration,
    /// Debuff and DoT only with mana above this % (`DpsManaMin`).
    pub dps_mana_min: u8,
    /// Debuff and DoT only a mob above this HP% (`DotStopAt`).
    pub dot_stop_at: u8,
    /// Give up on a debuff or DoT a mob resists this many times in a row
    /// (`MaxResists`).
    pub max_resists: u8,
    /// Nuke only with mana above this % (`NukeManaMin`).
    pub nuke_mana_min: u8,
    /// Sit to med below this mana %, until full; 0 never meds (`MedAt`).
    pub med_at: u8,
    /// Your pet taunts (`PetTaunt`); off while the paladin tanks.
    pub pet_taunt: bool,
    /// By lowercase name, since INI sections ignore case.
    pub(crate) archetypes: BTreeMap<String, Vec<String>>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            mode: Mode::Manual,
            follow: false,
            assist_at: 95,
            camp_radius: 100.0,
            heal_at: 70,
            emergency_at: 50,
            group_hot_at: 85,
            group_hot_count: 2,
            rebuff_at: Duration::from_secs(6),
            dps_mana_min: 40,
            dot_stop_at: 25,
            max_resists: 3,
            nuke_mana_min: 70,
            med_at: 80,
            pet_taunt: false,
            archetypes: BTreeMap::new(),
        }
    }
}

impl Config {
    /// The character's INI over the defaults. Also returns the problems
    /// found, to say in chat; each is skipped.
    pub fn read(file: &IniFile) -> (Config, Vec<String>) {
        let mut config = Config::default();
        let mut problems = Vec::new();
        for (section, entries) in &file.sections {
            let mut problem = |text: String| problems.push(format!("{}: [{section}] {text}", file.name));
            if section.eq_ignore_ascii_case(SETTINGS_SECTION) {
                for (key, value) in entries {
                    if let Err(text) = config.set(key, value) {
                        problem(text);
                    }
                }
            } else {
                config.archetypes.insert(section.to_ascii_lowercase(), archetype(entries, &mut problem));
            }
        }
        (config, problems)
    }

    /// The archetype's entries, best first; empty if there's no such section.
    pub fn archetype(&self, name: &str) -> &[String] {
        self.archetypes.get(&name.to_ascii_lowercase()).map_or(&[], Vec::as_slice)
    }

    /// Sections that aren't among `known` (a class's archetypes), as read.
    pub fn unknown_archetypes(&self, known: &[&str]) -> Vec<&str> {
        self.archetypes.keys().map(String::as_str).filter(|name| !known.iter().any(|k| k.eq_ignore_ascii_case(name))).collect()
    }

    /// Every archetype with its entries, by name.
    pub fn archetypes(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.archetypes.iter().map(|(name, entries)| (name.as_str(), entries.as_slice()))
    }

    /// Every `[Settings]` key with its value, as the INI writes it.
    pub fn settings(&self) -> Vec<(&'static str, String)> {
        SETTINGS.iter().map(|key| (*key, self.setting(key).unwrap_or_default())).collect()
    }

    /// A setting's value as the INI writes it, by any case of its key.
    pub fn setting(&self, key: &str) -> Option<String> {
        let on_off = |on: bool| if on { "on" } else { "off" }.to_owned();
        Some(match canonical(key)? {
            "Mode" => self.mode.to_string(),
            "Follow" => on_off(self.follow),
            "AssistAt" => self.assist_at.to_string(),
            "CampRadius" => self.camp_radius.to_string(),
            "HealAt" => self.heal_at.to_string(),
            "EmergencyAt" => self.emergency_at.to_string(),
            "GroupHotAt" => self.group_hot_at.to_string(),
            "GroupHotCount" => self.group_hot_count.to_string(),
            "RebuffAt" => self.rebuff_at.as_secs().to_string(),
            "DpsManaMin" => self.dps_mana_min.to_string(),
            "DotStopAt" => self.dot_stop_at.to_string(),
            "MaxResists" => self.max_resists.to_string(),
            "NukeManaMin" => self.nuke_mana_min.to_string(),
            "MedAt" => self.med_at.to_string(),
            "PetTaunt" => on_off(self.pet_taunt),
            _ => return None,
        })
    }

    /// Sets a `[Settings]` key, by any case, as the INI would. Returns the
    /// key as the INI spells it; `Err` says what's wrong, for chat.
    pub fn set(&mut self, key: &str, value: &str) -> Result<&'static str, String> {
        let name = canonical(key).ok_or_else(|| format!("{key} isn't a setting: {}", SETTINGS.join(", ")))?;
        let percent = |field: &mut u8| value.parse().ok().filter(|percent| (1..=100).contains(percent)).map(|percent| *field = percent).ok_or("a percent from 1 to 100");
        // 0 for no floor.
        let mana = |field: &mut u8| value.parse().ok().filter(|percent| *percent <= 100).map(|percent| *field = percent).ok_or("a percent from 0 to 100");
        let set = match key.to_ascii_lowercase().as_str() {
            "mode" => value.parse().map(|mode| self.mode = mode).map_err(|()| "off, manual or assist"),
            "follow" => switch(value).map(|on| self.follow = on).ok_or("on or off"),
            "assistat" => percent(&mut self.assist_at),
            "healat" => percent(&mut self.heal_at),
            "emergencyat" => percent(&mut self.emergency_at),
            "grouphotat" => percent(&mut self.group_hot_at),
            "grouphotcount" => value.parse().ok().filter(|count| (1..=6).contains(count)).map(|count| self.group_hot_count = count).ok_or("a count from 1 to 6"),
            "rebuffat" => value.parse().ok().filter(|seconds| *seconds <= 600).map(|seconds| self.rebuff_at = Duration::from_secs(seconds)).ok_or("seconds from 0 to 600"),
            "dpsmanamin" => mana(&mut self.dps_mana_min),
            "dotstopat" => mana(&mut self.dot_stop_at),
            "maxresists" => value.parse().ok().filter(|count| (1..=10).contains(count)).map(|count| self.max_resists = count).ok_or("a count from 1 to 10"),
            "nukemanamin" => mana(&mut self.nuke_mana_min),
            "medat" => mana(&mut self.med_at),
            "pettaunt" => switch(value).map(|on| self.pet_taunt = on).ok_or("on or off"),
            "campradius" => value.parse::<f32>().ok().filter(|radius| *radius > 0.0 && radius.is_finite()).map(|radius| self.camp_radius = radius).ok_or("a distance above 0"),
            _ => unreachable!("{name} is in SETTINGS but not set"),
        };
        set.map(|()| name).map_err(|expected| format!("{name}={value}: {expected}"))
    }
}

/// The `[Settings]` keys, as the INI spells them.
pub const SETTINGS: &[&str] = &["Mode", "Follow", "AssistAt", "CampRadius", "HealAt", "EmergencyAt", "GroupHotAt", "GroupHotCount", "RebuffAt", "DpsManaMin", "DotStopAt", "MaxResists", "NukeManaMin", "MedAt", "PetTaunt"];

fn canonical(key: &str) -> Option<&'static str> {
    SETTINGS.iter().copied().find(|name| name.eq_ignore_ascii_case(key))
}

/// Entries in their numbered order. Empty values are left out.
fn archetype(entries: &[(String, String)], problem: &mut impl FnMut(String)) -> Vec<String> {
    let mut numbered: Vec<(u32, &str)> = Vec::new();
    for (key, value) in entries {
        match key.parse() {
            Ok(number) => numbered.push((number, value.trim())),
            Err(_) => problem(format!("{key}={value}: entries are numbered 1=, 2=..., best first")),
        }
    }
    numbered.sort_by_key(|&(number, _)| number);
    numbered.into_iter().filter(|(_, value)| !value.is_empty()).map(|(_, value)| value.to_owned()).collect()
}

fn switch(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "true" | "1" => Some(true),
        "off" | "false" | "0" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ini(name: &str, sections: &[(&str, &[(&str, &str)])]) -> IniFile {
        let sections = sections
            .iter()
            .map(|(section, entries)| (section.to_string(), entries.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()))
            .collect();
        IniFile { name: name.to_owned(), sections }
    }

    #[test]
    fn nothing_gives_the_defaults() {
        assert_eq!(Config::read(&IniFile::default()), (Config::default(), vec![]));
        assert!(Config::default().archetype("ability.offense").is_empty());
    }

    #[test]
    fn archetypes_go_by_number_not_file_order() {
        let class = ini("Monk.ini", &[("ability.offense", &[("2", "Tiger Claw"), ("1", "Flying Kick"), ("10", "Round Kick"), ("3", " ")])]);
        let (config, problems) = Config::read(&class);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!(config.archetype("Ability.Offense"), ["Flying Kick", "Tiger Claw", "Round Kick"]);
    }

    #[test]
    fn reads_settings_and_archetypes_from_one_file() {
        let bob = ini("Bob.ini", &[
            ("settings", &[("Mode", "assist"), ("AssistAt", "80")]),
            ("ABILITY.OFFENSE", &[("1", "Flying Kick"), ("2", "Tiger Claw")]),
            ("mend", &[("1", "Mend")]),
        ]);
        let (config, problems) = Config::read(&bob);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!((config.mode, config.assist_at), (Mode::Assist, 80));
        assert_eq!(config.archetype("ability.offense"), ["Flying Kick", "Tiger Claw"]);
        assert_eq!(config.archetype("mend"), ["Mend"]);
        assert_eq!(config.archetypes().map(|(name, _)| name).collect::<Vec<_>>(), ["ability.offense", "mend"]);
    }

    #[test]
    fn says_which_sections_a_class_doesnt_read() {
        let monk = ini("Monk.ini", &[("ability.offensive", &[("1", "Flying Kick")]), ("Disc.Regular", &[("1", "Thunderkick")])]);
        let (config, _) = Config::read(&monk);
        assert_eq!(config.unknown_archetypes(&["ability.offense", "disc.regular"]), ["ability.offensive"]);
    }

    #[test]
    fn reads_every_setting() {
        let settings = ini("Monk.ini", &[("Settings", &[("mode", "OFF"), ("Follow", "on"), ("AssistAt", "100"), ("CampRadius", "60.5")])]);
        let (config, problems) = Config::read(&settings);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!((config.mode, config.follow, config.assist_at, config.camp_radius), (Mode::Off, true, 100, 60.5));
        let heals = ini("Shaman.ini", &[("Settings", &[("HealAt", "75"), ("EmergencyAt", "40"), ("GroupHotAt", "90"), ("GroupHotCount", "3"), ("RebuffAt", "20")])]);
        let (config, problems) = Config::read(&heals);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!((config.heal_at, config.emergency_at, config.group_hot_at, config.group_hot_count), (75, 40, 90, 3));
        assert_eq!(config.rebuff_at, Duration::from_secs(20));
        let (_, problems) = Config::read(&ini("Shaman.ini", &[("Settings", &[("HealAt", "101"), ("GroupHotCount", "0"), ("RebuffAt", "-1")])]));
        assert_eq!(problems, ["Shaman.ini: [Settings] HealAt=101: a percent from 1 to 100", "Shaman.ini: [Settings] GroupHotCount=0: a count from 1 to 6", "Shaman.ini: [Settings] RebuffAt=-1: seconds from 0 to 600"]);
        let dps = ini("Shaman.ini", &[("Settings", &[("DpsManaMin", "0"), ("DotStopAt", "10"), ("MaxResists", "2"), ("NukeManaMin", "100")])]);
        let (config, problems) = Config::read(&dps);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!((config.dps_mana_min, config.dot_stop_at, config.max_resists, config.nuke_mana_min), (0, 10, 2, 100));
        let (_, problems) = Config::read(&ini("Shaman.ini", &[("Settings", &[("NukeManaMin", "101"), ("MaxResists", "0")])]));
        assert_eq!(problems, ["Shaman.ini: [Settings] NukeManaMin=101: a percent from 0 to 100", "Shaman.ini: [Settings] MaxResists=0: a count from 1 to 10"]);
        assert_eq!(Config::default().med_at, 80);
        let (config, problems) = Config::read(&ini("Shaman.ini", &[("Settings", &[("MedAt", "0")])]));
        assert_eq!((config.med_at, problems), (0, vec![]), "0 never meds");
        let (_, problems) = Config::read(&ini("Shaman.ini", &[("Settings", &[("MedAt", "101")])]));
        assert_eq!(problems, ["Shaman.ini: [Settings] MedAt=101: a percent from 0 to 100"]);
        assert!(!Config::default().pet_taunt, "the paladin tanks");
        let (config, problems) = Config::read(&ini("Magician.ini", &[("Settings", &[("PetTaunt", "on")])]));
        assert_eq!((config.pet_taunt, config.setting("pettaunt"), problems), (true, Some("on".into()), vec![]));
    }

    #[test]
    fn says_what_it_skipped() {
        let bad = ini("Monk.ini", &[
            ("Settings", &[("Mode", "tank"), ("Follow", "maybe"), ("AssistAt", "0"), ("CampRadius", "-5"), ("Speed", "9")]),
            ("ability.offense", &[("first", "Flying Kick"), ("2", "Tiger Claw")]),
        ]);
        let (config, problems) = Config::read(&bad);
        assert_eq!(problems, [
            "Monk.ini: [Settings] Mode=tank: off, manual or assist",
            "Monk.ini: [Settings] Follow=maybe: on or off",
            "Monk.ini: [Settings] AssistAt=0: a percent from 1 to 100",
            "Monk.ini: [Settings] CampRadius=-5: a distance above 0",
            "Monk.ini: [Settings] Speed isn't a setting: Mode, Follow, AssistAt, CampRadius, HealAt, EmergencyAt, GroupHotAt, GroupHotCount, RebuffAt, DpsManaMin, DotStopAt, MaxResists, NukeManaMin, MedAt, PetTaunt",
            "Monk.ini: [ability.offense] first=Flying Kick: entries are numbered 1=, 2=..., best first",
        ]);
        let defaults = Config::default();
        assert_eq!((config.mode, config.follow, config.assist_at, config.camp_radius), (defaults.mode, defaults.follow, defaults.assist_at, defaults.camp_radius));
        assert_eq!(config.archetype("ability.offense"), ["Tiger Claw"]);
    }

    #[test]
    fn writes_back_every_setting_as_it_reads_it() {
        let mut config = Config::default();
        assert_eq!(config.set("healat", "60"), Ok("HealAt"));
        assert_eq!(config.set("Follow", "ON"), Ok("Follow"));
        assert_eq!(config.set("HealAt", "0"), Err("HealAt=0: a percent from 1 to 100".to_owned()));
        assert_eq!(config.heal_at, 60, "a bad value changes nothing");
        assert_eq!(config.setting("HEALAT"), Some("60".to_owned()));
        assert_eq!(config.setting("Speed"), None);
        let settings = config.settings();
        assert_eq!(settings.iter().map(|(key, _)| *key).collect::<Vec<_>>(), SETTINGS);
        let mut again = Config::default();
        for (key, value) in &settings {
            assert_eq!(again.set(key, value), Ok(*key), "{key}={value} reads back");
        }
        assert_eq!(again, config);
    }
}
