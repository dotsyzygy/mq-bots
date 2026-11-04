//! MQ2Bots' logic that doesn't touch the game, so it's tested without one.

mod class;
mod commands;
mod config;
mod engage;
mod state;

use std::fmt;
use std::str::FromStr;

pub use class::{Action, Class, Game, REFRESH_BUFF_AT, aura_named, disc_usable, for_class, same_name};
pub use commands::{Command, USAGE, parse};
pub use config::{Config, Layer, SETTINGS_SECTION};
pub use engage::{Decision, Mob, Seen, decide};
pub use state::State;

/// What a bot does on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Nothing, not even commands that act.
    Off,
    /// Only what commands tell it.
    #[default]
    Manual,
    /// Engages the Main Assist's target.
    Assist,
}

impl FromStr for Mode {
    type Err = ();

    fn from_str(text: &str) -> Result<Mode, ()> {
        match text.to_ascii_lowercase().as_str() {
            "off" => Ok(Mode::Off),
            "manual" => Ok(Mode::Manual),
            "assist" => Ok(Mode::Assist),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Mode::Off => "off",
            Mode::Manual => "manual",
            Mode::Assist => "assist",
        })
    }
}
