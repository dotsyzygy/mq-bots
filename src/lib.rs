#![allow(non_snake_case)] // the crate is named after the DLL

mod fight;
mod posse;

use std::path::{Path, PathBuf};

use bots_core::{Class, Command, Config, Layer, Mode, SETTINGS_SECTION, State, USAGE, for_class, parse};
use mq_emu::{GameState, Ini, IniError, Mq, Plugin, green, red, yellow};

const COMMAND: &str = "/bots";

struct Bots {
    /// The character the config was loaded for; empty until one is in game.
    loaded_for: String,
    config: Config,
    state: State,
    /// What this character's class does in a fight; `None` until a class has one.
    class: Option<Box<dyn Class>>,
    /// Where the Main Assist's target comes from.
    posse: Option<posse::PosseLink>,
    fight: fight::Fight,
}

impl Plugin for Bots {
    fn new(mq: &Mq<Self>) -> Self {
        if let Err(e) = mq.add_command(COMMAND, Bots::command) {
            mq.chat(red!("[Bots] couldn't add {COMMAND}: {e}"));
        }
        let config = Config::default();
        Bots { loaded_for: String::new(), state: State::new(&config), config, class: None, posse: None, fight: fight::Fight::default() }
    }

    fn on_pulse(&mut self, mq: &Mq<Self>) {
        if mq.game_state() == GameState::InGame
            && let Some(name) = mq.me().map(|me| me.name())
            && name != self.loaded_for
        {
            self.load(mq);
            self.state = State::new(&self.config);
            self.come_in(mq, &name);
            self.loaded_for = name;
        }
        if !self.loaded_for.is_empty() && mq.game_state() == GameState::InGame {
            self.fight(mq);
        }
    }

    /// What was set by command (mode, following...) stays; what belonged to
    /// the old zone goes. The mob's spawn ID may be another's in the new zone.
    fn on_begin_zone(&mut self, mq: &Mq<Self>) {
        self.state.engaged = None;
        self.state.attack_requested = false;
        self.stop_task(mq);
        // MQ2AdvPath takes no commands while zoning; the first look in the
        // new zone follows again.
        self.fight.followed = None;
    }

    /// Doesn't leave the character following with no one to stop it.
    fn shutdown(&mut self, mq: &Mq<Self>) {
        self.follow(mq, None);
    }

    fn on_set_game_state(&mut self, mq: &Mq<Self>, state: GameState) {
        // The next character in may not be this one. Not on LoggingIn:
        // zoning passes through it, and changing characters always goes
        // through character select.
        if matches!(state, GameState::CharacterSelect | GameState::CharacterCreate) {
            self.loaded_for.clear();
            self.posse = None;
            self.stop_task(mq);
            self.fight.followed = None;
        }
    }
}

impl Bots {
    fn command(&mut self, mq: &Mq<Self>, args: &str) {
        let command = match parse(args) {
            Ok(command) => command,
            Err(e) => {
                mq.chat(red!("[Bots] {e}"));
                mq.chat(green!("{USAGE}"));
                return;
            }
        };
        match command {
            Command::Status => self.status(mq),
            Command::Help => mq.chat(green!("{USAGE}")),
            Command::Mode(mode) => self.set_setting(mq, "Mode", &mode.to_string()),
            Command::Set(None) => {
                let settings: Vec<String> = self.config.settings().into_iter().map(|(key, value)| format!("{key}={value}")).collect();
                mq.chat(green!("[Bots] {}", settings.join(", ")));
            }
            Command::Set(Some((key, value))) => self.set_setting(mq, &key, &value),
            Command::Attack if self.state.mode == Mode::Off => mq.chat(yellow!("[Bots] mode is off")),
            Command::Attack => {
                self.state.attack_requested = true;
                self.state.backed_off = false;
            }
            Command::Backoff => {
                self.state.backed_off = true;
                mq.chat(green!("[Bots] backed off until /bots resume"));
            }
            Command::Resume => {
                self.state.backed_off = false;
                mq.chat(green!("[Bots] resumed"));
            }
            Command::Burn(on) => {
                self.state.burning = on;
                mq.chat(green!("[Bots] burn {}", if on { "on" } else { "off" }));
            }
            Command::Follow(on) => {
                let on = on.unwrap_or(!self.state.following);
                self.set_setting(mq, "Follow", if on { "on" } else { "off" });
            }
            // Commands keep what they set; only the INI's values change.
            Command::Reload if self.loaded_for.is_empty() => mq.chat(yellow!("[Bots] nothing to reload until a character is in game")),
            Command::Reload => {
                self.load(mq);
                self.fight.given_up_auras.clear();
                self.fight.given_up_spells.clear();
                self.fight.resists.clear();
                self.fight.said_left_out = false;
                self.state.memorize = true;
                if let Some(class) = &self.class {
                    self.check_archetypes(mq, class.archetypes());
                }
            }
        }
    }

    fn status(&self, mq: &Mq<Self>) {
        mq.chat(green!("[Bots] {}", self.state.summary()));
        match &self.posse {
            Some(posse) => mq.chat(green!("[Bots] the MA's target comes from posse {}", posse.name)),
            None => mq.chat(yellow!("[Bots] not in a posse, so not assisting")),
        }
        let archetypes: Vec<String> = self.config.archetypes().map(|(name, entries)| format!("{name} ({})", entries.len())).collect();
        if archetypes.is_empty() {
            mq.chat(yellow!("[Bots] no archetypes"));
        } else {
            mq.chat(green!("[Bots] archetypes: {}", archetypes.join(", ")));
        }
    }

    /// Picks the class's module and the posse, for a character just in.
    fn come_in(&mut self, mq: &Mq<Self>, character: &str) {
        let class = mq.me().and_then(|me| mq.class_name(me.class())).unwrap_or_default();
        self.class = for_class(&class);
        self.stop_task(mq);
        self.fight = fight::Fight::default();
        match &self.class {
            None => mq.chat(yellow!("[Bots] {class} isn't implemented yet")),
            Some(module) => self.check_archetypes(mq, module.archetypes()),
        }
        self.posse = match posse::PosseLink::open(mq, character) {
            Ok(Some(link)) => Some(link),
            Ok(None) => {
                mq.chat(yellow!("[Bots] not in a posse (AutoJoin=0), so not assisting"));
                None
            }
            Err(e) => {
                mq.chat(red!("[Bots] {e}, so not assisting"));
                None
            }
        };
    }

    /// Says which INI sections the class doesn't read: most likely misspelled.
    fn check_archetypes(&self, mq: &Mq<Self>, known: &[&str]) {
        for name in self.config.unknown_archetypes(known) {
            mq.chat(yellow!("[Bots] [{name}] isn't an archetype this class reads: {}", known.join(", ")));
        }
    }

    /// Sets a `[Settings]` key now and writes it to the character's INI, so
    /// it's kept through zoning, camping and `/bots reload`; the
    /// character's INI overrides the class's.
    fn set_setting(&mut self, mq: &Mq<Self>, key: &str, value: &str) {
        let key = match self.config.set(key, value) {
            Ok(key) => key,
            Err(e) => return mq.chat(red!("[Bots] {e}")),
        };
        self.state.mode = self.config.mode;
        self.state.following = self.config.follow;
        let value = self.config.setting(key).unwrap_or_default();
        mq.chat(green!("[Bots] {key}={value}"));
        if self.loaded_for.is_empty() {
            return mq.chat(yellow!("[Bots] not saved until a character is in game"));
        }
        let path = config_path(mq, &mq.server_name().unwrap_or_default(), &self.loaded_for);
        if let Err(e) = Ini::new(&path).and_then(|ini| ini.set(SETTINGS_SECTION, key, &value)) {
            mq.chat(red!("[Bots] couldn't save {key} to config/{}: {e}", file_name(&path)));
        }
    }

    /// Reads the class's INI and then the character's, saying in chat what
    /// was read and what was wrong with it.
    fn load(&mut self, mq: &Mq<Self>) {
        let Some(me) = mq.me() else { return };
        let server = mq.server_name().unwrap_or_default();
        let class = mq.class_name(me.class()).unwrap_or_else(|| me.class().to_string());
        let paths = [config_path(mq, &server, &class), config_path(mq, &server, &me.name())];
        let mut layers = Vec::new();
        for path in &paths {
            match read_layer(path) {
                Ok(Some(layer)) => layers.push(layer),
                Ok(None) => {}
                Err(e) => mq.chat(red!("[Bots] couldn't read config/{}: {e}", file_name(path))),
            }
        }
        let (config, problems) = Config::from_layers(&layers);
        for problem in problems {
            mq.chat(yellow!("[Bots] {problem}"));
        }
        if layers.is_empty() {
            mq.chat(yellow!("[Bots] no config: create config/{} with the {class} archetypes", file_name(&paths[0])));
        } else {
            let names: Vec<&str> = layers.iter().map(|layer| layer.name.as_str()).collect();
            mq.chat(green!("[Bots] read {}: {} archetypes", names.join(" and "), config.archetypes().count()));
        }
        self.config = config;
    }
}

/// `MQ2Bots_<server>_<class or character>.ini` in MacroQuest's config folder.
fn config_path(mq: &Mq<Bots>, server: &str, name: &str) -> PathBuf {
    mq.config_dir().join(format!("MQ2Bots_{server}_{name}.ini"))
}

/// `None` if there's no such file.
fn read_layer(path: &Path) -> Result<Option<Layer>, IniError> {
    if !path.exists() {
        return Ok(None);
    }
    let ini = Ini::new(path)?;
    let sections = ini.sections().into_iter().map(|section| {
        let entries = ini.section(&section);
        (section, entries)
    });
    Ok(Some(Layer { name: file_name(path), sections: sections.collect() }))
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

mq_emu::plugin!(Bots);
