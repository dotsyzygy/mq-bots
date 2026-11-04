#![allow(non_snake_case)] // the crate is named after the DLL

use std::path::{Path, PathBuf};

use bots_core::{Command, Config, Layer, State, USAGE, parse};
use mq_emu::{GameState, Ini, IniError, Mq, Plugin, green, red, yellow};

const COMMAND: &str = "/bots";

struct Bots {
    /// The character the config was loaded for; empty until one is in game.
    loaded_for: String,
    config: Config,
    state: State,
}

impl Plugin for Bots {
    fn new(mq: &Mq<Self>) -> Self {
        if let Err(e) = mq.add_command(COMMAND, Bots::command) {
            mq.chat(red!("[Bots] couldn't add {COMMAND}: {e}"));
        }
        let config = Config::default();
        Bots { loaded_for: String::new(), state: State::new(&config), config }
    }

    fn on_pulse(&mut self, mq: &Mq<Self>) {
        if mq.game_state() == GameState::InGame
            && let Some(name) = mq.me().map(|me| me.name())
            && name != self.loaded_for
        {
            self.load(mq);
            self.state = State::new(&self.config);
            self.loaded_for = name;
        }
    }

    fn on_set_game_state(&mut self, _mq: &Mq<Self>, state: GameState) {
        // The next character in may not be this one.
        if matches!(state, GameState::CharacterSelect | GameState::CharacterCreate | GameState::LoggingIn) {
            self.loaded_for.clear();
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
            Command::Attack => mq.chat(yellow!("[Bots] attack isn't built yet")),
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
        let archetypes: Vec<String> = self.config.archetypes().map(|(name, entries)| format!("{name} ({})", entries.len())).collect();
        if archetypes.is_empty() {
            mq.chat(yellow!("[Bots] no archetypes"));
        } else {
            mq.chat(green!("[Bots] archetypes: {}", archetypes.join(", ")));
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
                Err(e) => mq.chat(red!("[Bots] couldn't read {}: {e}", path.display())),
            }
        }
        let (config, problems) = Config::from_layers(&layers);
        for problem in problems {
            mq.chat(yellow!("[Bots] {problem}"));
        }
        if layers.is_empty() {
            mq.chat(yellow!("[Bots] no config: create {} with the {class} archetypes", paths[0].display()));
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
    let name = path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    Ok(Some(Layer { name, sections: sections.collect() }))
}

mq_emu::plugin!(Bots);
