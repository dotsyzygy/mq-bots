#![allow(non_snake_case)] // the crate is named after the DLL

mod fight;
mod posse;

use std::path::{Path, PathBuf};

use bots_core::{Class, Command, Config, Layer, Mode, State, USAGE, for_class, parse};
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

    fn on_set_game_state(&mut self, _mq: &Mq<Self>, state: GameState) {
        // The next character in may not be this one.
        if matches!(state, GameState::CharacterSelect | GameState::CharacterCreate | GameState::LoggingIn) {
            self.loaded_for.clear();
            self.posse = None;
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
            Command::Mode(mode) => {
                self.state.mode = mode;
                mq.chat(green!("[Bots] mode {mode}"));
            }
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
                self.state.following = on.unwrap_or(!self.state.following);
                mq.chat(green!("[Bots] follow {}", if self.state.following { "on" } else { "off" }));
            }
            // Commands keep what they set; only the INI's values change.
            Command::Reload if self.loaded_for.is_empty() => mq.chat(yellow!("[Bots] nothing to reload until a character is in game")),
            Command::Reload => self.load(mq),
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
        if self.class.is_none() {
            mq.chat(yellow!("[Bots] {class} doesn't fight yet"));
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
