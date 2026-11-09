//! What follows `/bots`.

use crate::Mode;

pub const USAGE: &str = "usage: /bots [help | mode off|manual|assist | attack | backoff | resume | burn [off] | follow [on|off] | set [<setting> <value>] | reload]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `set <setting> <value>` sets an INI setting and saves it; `set` alone
    /// lists them.
    Set(Option<(String, String)>),
    /// No arguments: show this bot's state.
    Status,
    Help,
    Mode(Mode),
    /// Engage the MA's target now, ignoring assist-at.
    Attack,
    Backoff,
    Resume,
    /// `burn` starts a burn phase, `burn off` ends it.
    Burn(bool),
    /// `None` toggles.
    Follow(Option<bool>),
    Reload,
}

/// Parses `/bots` arguments. `Err` says what's wrong, for chat.
pub fn parse(args: &str) -> Result<Command, String> {
    let mut words = args.split_whitespace();
    let command = words.next().unwrap_or("").to_ascii_lowercase();
    if command == "set" {
        return match (words.next(), words.next(), words.next()) {
            (None, ..) => Ok(Command::Set(None)),
            (Some(key), Some(value), None) => Ok(Command::Set(Some((key.to_owned(), value.to_owned())))),
            (_, _, Some(extra)) => Err(format!("unexpected \"{extra}\"")),
            (Some(_), None, _) => Err("usage: /bots set [<setting> <value>]".into()),
        };
    }
    let arg = words.next().map(str::to_ascii_lowercase);
    if let Some(extra) = words.next() {
        return Err(format!("unexpected \"{extra}\""));
    }
    match (command.as_str(), arg.as_deref()) {
        ("", None) => Ok(Command::Status),
        ("help", None) => Ok(Command::Help),
        ("mode", Some(mode)) => mode.parse().map(Command::Mode).map_err(|()| format!("unknown mode \"{mode}\": off, manual or assist")),
        ("mode", None) => Err("usage: /bots mode off|manual|assist".into()),
        ("attack", None) => Ok(Command::Attack),
        ("backoff", None) => Ok(Command::Backoff),
        ("resume", None) => Ok(Command::Resume),
        ("burn", None | Some("on")) => Ok(Command::Burn(true)),
        ("burn", Some("off")) => Ok(Command::Burn(false)),
        ("burn", Some(_)) => Err("usage: /bots burn [off]".into()),
        ("follow", None) => Ok(Command::Follow(None)),
        ("follow", Some("on")) => Ok(Command::Follow(Some(true))),
        ("follow", Some("off")) => Ok(Command::Follow(Some(false))),
        ("follow", Some(_)) => Err("usage: /bots follow [on|off]".into()),
        ("reload", None) => Ok(Command::Reload),
        ("help" | "attack" | "backoff" | "resume" | "reload", Some(arg)) => Err(format!("{command} takes nothing after it, not \"{arg}\"")),
        _ => Err(format!("unknown command \"{command}\"")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_command() {
        let cases = [
            ("", Command::Status),
            ("  ", Command::Status),
            ("help", Command::Help),
            ("mode assist", Command::Mode(Mode::Assist)),
            ("MODE Off", Command::Mode(Mode::Off)),
            ("mode manual", Command::Mode(Mode::Manual)),
            ("attack", Command::Attack),
            ("backoff", Command::Backoff),
            ("resume", Command::Resume),
            ("burn", Command::Burn(true)),
            ("burn on", Command::Burn(true)),
            ("burn off", Command::Burn(false)),
            ("follow", Command::Follow(None)),
            ("follow on", Command::Follow(Some(true))),
            ("follow OFF", Command::Follow(Some(false))),
            ("reload", Command::Reload),
            ("set", Command::Set(None)),
            ("SET HealAt 60", Command::Set(Some(("HealAt".into(), "60".into())))),
        ];
        for (args, expected) in cases {
            assert_eq!(parse(args), Ok(expected), "{args:?}");
        }
    }

    #[test]
    fn refuses_bad_arguments() {
        let cases = [
            ("dance", "unknown command \"dance\""),
            ("mode", "usage: /bots mode off|manual|assist"),
            ("mode tank", "unknown mode \"tank\": off, manual or assist"),
            ("burn now", "usage: /bots burn [off]"),
            ("follow me", "usage: /bots follow [on|off]"),
            ("attack now", "attack takes nothing after it, not \"now\""),
            ("burn off now", "unexpected \"now\""),
            ("set HealAt", "usage: /bots set [<setting> <value>]"),
            ("set HealAt 60 now", "unexpected \"now\""),
        ];
        for (args, expected) in cases {
            assert_eq!(parse(args), Err(expected.to_owned()), "{args:?}");
        }
    }
}
