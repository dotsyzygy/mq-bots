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

use crate::Mode;

pub const SETTINGS_SECTION: &str = "Settings";

/// One INI file's sections, each with its entries in file order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Layer {
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
    /// By lowercase name, since INI sections ignore case.
    pub(crate) archetypes: BTreeMap<String, Vec<String>>,
}

impl Default for Config {
    fn default() -> Config {
        Config { mode: Mode::Manual, follow: false, assist_at: 95, camp_radius: 100.0, archetypes: BTreeMap::new() }
    }
}

impl Config {
    /// Later layers override earlier ones: a section of archetype entries
    /// replaces the whole list, and a setting replaces just that setting.
    /// Also returns the problems found, to say in chat; each is skipped.
    pub fn from_layers(layers: &[Layer]) -> (Config, Vec<String>) {
        let mut config = Config::default();
        let mut problems = Vec::new();
        for layer in layers {
            for (section, entries) in &layer.sections {
                let mut problem = |text: String| problems.push(format!("{}: [{section}] {text}", layer.name));
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
        }
        (config, problems)
    }

    /// The archetype's entries, best first; empty if there's no such section.
    pub fn archetype(&self, name: &str) -> &[String] {
        self.archetypes.get(&name.to_ascii_lowercase()).map_or(&[], Vec::as_slice)
    }

    /// Sections that aren't among `known` (a class's archetypes, where `#`
    /// stands for a number: `dot.#`), as read.
    pub fn unknown_archetypes(&self, known: &[&str]) -> Vec<&str> {
        self.archetypes.keys().map(String::as_str).filter(|name| !known.iter().any(|known| matches(known, name).is_some())).collect()
    }

    /// The archetypes there are sections for that `pattern` names: itself,
    /// or for `dot.#`, `dot.1`, `dot.2`... in number order.
    pub fn archetypes_like(&self, pattern: &str) -> Vec<&str> {
        let mut found: Vec<(u32, &str)> = self.archetypes.keys().filter_map(|name| Some((matches(pattern, name)?, name.as_str()))).collect();
        found.sort_by_key(|&(number, _)| number);
        found.into_iter().map(|(_, name)| name).collect()
    }

    /// Every archetype with its entries, by name.
    pub fn archetypes(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.archetypes.iter().map(|(name, entries)| (name.as_str(), entries.as_slice()))
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let set = match key.to_ascii_lowercase().as_str() {
            "mode" => value.parse().map(|mode| self.mode = mode).map_err(|()| "off, manual or assist"),
            "follow" => switch(value).map(|on| self.follow = on).ok_or("on or off"),
            "assistat" => value.parse().ok().filter(|percent| (1..=100).contains(percent)).map(|percent| self.assist_at = percent).ok_or("a percent from 1 to 100"),
            "campradius" => value.parse::<f32>().ok().filter(|radius| *radius > 0.0 && radius.is_finite()).map(|radius| self.camp_radius = radius).ok_or("a distance above 0"),
            _ => return Err(format!("{key} isn't a setting")),
        };
        set.map_err(|expected| format!("{key}={value}: {expected}"))
    }
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

/// Whether archetype `name` is `pattern`, ignoring case, where a `#` ending
/// `pattern` stands for a number; that number, or 0 without a `#`.
fn matches(pattern: &str, name: &str) -> Option<u32> {
    let Some(prefix) = pattern.strip_suffix('#') else { return pattern.eq_ignore_ascii_case(name).then_some(0) };
    let number = name.get(prefix.len()..).filter(|_| name[..prefix.len()].eq_ignore_ascii_case(prefix))?;
    number.bytes().all(|b| b.is_ascii_digit()).then(|| number.parse().ok()).flatten()
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

    fn layer(name: &str, sections: &[(&str, &[(&str, &str)])]) -> Layer {
        let sections = sections
            .iter()
            .map(|(section, entries)| (section.to_string(), entries.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()))
            .collect();
        Layer { name: name.to_owned(), sections }
    }

    #[test]
    fn nothing_gives_the_defaults() {
        assert_eq!(Config::from_layers(&[]), (Config::default(), vec![]));
        assert!(Config::default().archetype("ability.offense").is_empty());
    }

    #[test]
    fn archetypes_go_by_number_not_file_order() {
        let class = layer("Monk.ini", &[("ability.offense", &[("2", "Tiger Claw"), ("1", "Flying Kick"), ("10", "Round Kick"), ("3", " ")])]);
        let (config, problems) = Config::from_layers(&[class]);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!(config.archetype("Ability.Offense"), ["Flying Kick", "Tiger Claw", "Round Kick"]);
    }

    #[test]
    fn a_later_layer_replaces_whole_archetypes_and_single_settings() {
        let class = layer("Monk.ini", &[
            ("Settings", &[("Mode", "assist"), ("AssistAt", "90")]),
            ("ability.offense", &[("1", "Flying Kick"), ("2", "Tiger Claw")]),
            ("mend", &[("1", "Mend")]),
        ]);
        let character = layer("Bob.ini", &[("settings", &[("AssistAt", "80")]), ("ABILITY.OFFENSE", &[("1", "Kick")])]);
        let (config, problems) = Config::from_layers(&[class, character]);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!((config.mode, config.assist_at), (Mode::Assist, 80));
        assert_eq!(config.archetype("ability.offense"), ["Kick"]);
        assert_eq!(config.archetype("mend"), ["Mend"]);
        assert_eq!(config.archetypes().map(|(name, _)| name).collect::<Vec<_>>(), ["ability.offense", "mend"]);
    }

    #[test]
    fn says_which_sections_a_class_doesnt_read() {
        let monk = layer("Monk.ini", &[("ability.offensive", &[("1", "Flying Kick")]), ("Disc.Regular", &[("1", "Thunderkick")])]);
        let (config, _) = Config::from_layers(&[monk]);
        assert_eq!(config.unknown_archetypes(&["ability.offense", "disc.regular"]), ["ability.offensive"]);
    }

    #[test]
    fn numbered_archetypes_match_a_hash() {
        let shaman = layer("Shaman.ini", &[("dot.2", &[("1", "Bane")]), ("DOT.10", &[("1", "Bane")]), ("dot.1", &[("1", "Bane")]), ("dot.", &[("1", "Bane")]), ("dot.x", &[("1", "Bane")]), ("dot.+1", &[("1", "Bane")]), ("nuke", &[("1", "Bolt")])]);
        let (config, _) = Config::from_layers(&[shaman]);
        assert_eq!(config.unknown_archetypes(&["dot.#", "nuke"]), ["dot.", "dot.+1", "dot.x"]);
        assert_eq!(config.archetypes_like("Dot.#"), ["dot.1", "dot.2", "dot.10"]);
        assert_eq!(config.archetypes_like("NUKE"), ["nuke"]);
        assert_eq!(config.archetypes_like("heal"), Vec::<&str>::new());
    }

    #[test]
    fn reads_every_setting() {
        let settings = layer("Monk.ini", &[("Settings", &[("mode", "OFF"), ("Follow", "on"), ("AssistAt", "100"), ("CampRadius", "60.5")])]);
        let (config, problems) = Config::from_layers(&[settings]);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!((config.mode, config.follow, config.assist_at, config.camp_radius), (Mode::Off, true, 100, 60.5));
    }

    #[test]
    fn says_what_it_skipped() {
        let bad = layer("Monk.ini", &[
            ("Settings", &[("Mode", "tank"), ("Follow", "maybe"), ("AssistAt", "0"), ("CampRadius", "-5"), ("Speed", "9")]),
            ("ability.offense", &[("first", "Flying Kick"), ("2", "Tiger Claw")]),
        ]);
        let (config, problems) = Config::from_layers(&[bad]);
        assert_eq!(problems, [
            "Monk.ini: [Settings] Mode=tank: off, manual or assist",
            "Monk.ini: [Settings] Follow=maybe: on or off",
            "Monk.ini: [Settings] AssistAt=0: a percent from 1 to 100",
            "Monk.ini: [Settings] CampRadius=-5: a distance above 0",
            "Monk.ini: [Settings] Speed isn't a setting",
            "Monk.ini: [ability.offense] first=Flying Kick: entries are numbered 1=, 2=..., best first",
        ]);
        let defaults = Config::default();
        assert_eq!((config.mode, config.follow, config.assist_at, config.camp_radius), (defaults.mode, defaults.follow, defaults.assist_at, defaults.camp_radius));
        assert_eq!(config.archetype("ability.offense"), ["Tiger Claw"]);
    }
}
