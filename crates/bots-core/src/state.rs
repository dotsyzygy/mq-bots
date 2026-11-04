//! What commands have set on a bot since its character came in.

use crate::{Config, Mode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub mode: Mode,
    /// `/bots backoff`: don't engage until `/bots resume`.
    pub backed_off: bool,
    /// In a burn phase.
    pub burning: bool,
    pub following: bool,
    /// `/bots attack`, until the next pulse decides on it.
    pub attack_requested: bool,
    /// The mob being fought, by spawn ID.
    pub engaged: Option<u32>,
    /// `[Gems]` is to be memorized out of combat: set on load and on
    /// `/bots reload`, and cleared once every gem holds its spell.
    pub memorize: bool,
}

impl State {
    /// How a character starts, from its config.
    pub fn new(config: &Config) -> State {
        State { mode: config.mode, backed_off: false, burning: false, following: config.follow, attack_requested: false, engaged: None, memorize: true }
    }

    /// For `/bots` with nothing after it: "mode assist, following, burning".
    pub fn summary(&self) -> String {
        let mut parts = vec![format!("mode {}", self.mode)];
        let flags = [(self.backed_off, "backed off"), (self.burning, "burning"), (self.following, "following")];
        parts.extend(flags.iter().filter(|(on, _)| *on).map(|(_, name)| name.to_string()));
        parts.extend(self.engaged.map(|id| format!("fighting {id}")));
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_from_the_config_and_sums_up() {
        let config = Config { mode: Mode::Assist, follow: true, ..Config::default() };
        let mut state = State::new(&config);
        assert_eq!(state.summary(), "mode assist, following");
        state.following = false;
        assert_eq!(state.summary(), "mode assist");
        (state.backed_off, state.burning, state.engaged) = (true, true, Some(42));
        assert_eq!(state.summary(), "mode assist, backed off, burning, fighting 42");
    }
}
