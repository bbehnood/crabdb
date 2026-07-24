use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Set(String, String),
    Get(String),
    Delete(String),
    Exit,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ParseError {
    #[error("error: missing key\n\nUsage: {0}")]
    MissingKey(&'static str),

    #[error("error: missing value\n\nUsage: {0}")]
    MissingValue(&'static str),

    #[error("error: extra argument '{0}'")]
    ExtraArgument(String),

    #[error("error: unknown command '{0}'")]
    UnknownCommand(String),
}

const SET_COMMAND_USAGE: &str = "SET <key> <value>";
const GET_COMMAND_USAGE: &str = "GET <key>";
const DELETE_COMMAND_USAGE: &str = "DELETE <key>";

fn split_word(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    match s.find(char::is_whitespace) {
        Some(idx) => (&s[..idx], s[idx..].trim_start()),
        None => (s, ""),
    }
}

pub fn parse(input: &str) -> Result<Command, ParseError> {
    let (cmd, rest) = split_word(input);

    match cmd {
        c if c.eq_ignore_ascii_case("SET") => {
            let (key, value) = split_word(rest);

            if key.is_empty() {
                return Err(ParseError::MissingKey(SET_COMMAND_USAGE));
            }

            if value.is_empty() {
                return Err(ParseError::MissingValue(SET_COMMAND_USAGE));
            }

            Ok(Command::Set(key.to_owned(), value.to_owned()))
        }

        c if c.eq_ignore_ascii_case("GET") => {
            let (key, extra) = split_word(rest);

            if key.is_empty() {
                return Err(ParseError::MissingKey(GET_COMMAND_USAGE));
            }

            if !extra.is_empty() {
                return Err(ParseError::ExtraArgument(extra.to_owned()));
            }

            Ok(Command::Get(key.to_owned()))
        }

        c if c.eq_ignore_ascii_case("DELETE") => {
            let (key, extra) = split_word(rest);

            if key.is_empty() {
                return Err(ParseError::MissingKey(DELETE_COMMAND_USAGE));
            }

            if !extra.is_empty() {
                return Err(ParseError::ExtraArgument(extra.to_owned()));
            }

            Ok(Command::Delete(key.to_owned()))
        }

        c if c.eq_ignore_ascii_case("EXIT")
            || c.eq_ignore_ascii_case("QUIT") =>
        {
            Ok(Command::Exit)
        }

        cmd => Err(ParseError::UnknownCommand(cmd.to_owned())),
    }
}
