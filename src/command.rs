use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Set(String, String),
    Get(String),
    Delete(String),
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

        cmd => Err(ParseError::UnknownCommand(cmd.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_set_with_single_space() {
        assert_eq!(
            parse("SET key value").unwrap(),
            Command::Set("key".to_owned(), "value".to_owned())
        );
    }

    #[test]
    fn set_with_repeated_spaces_does_not_leak_into_value() {
        // Regression test: `splitn` used to leave a leading space stuck to
        // the front of the value whenever there was more than one space
        // between key and value.
        assert_eq!(
            parse("SET key  value").unwrap(),
            Command::Set("key".to_owned(), "value".to_owned())
        );

        assert_eq!(
            parse("SET   key    value").unwrap(),
            Command::Set("key".to_owned(), "value".to_owned())
        );
    }

    #[test]
    fn set_value_may_contain_internal_spaces() {
        assert_eq!(
            parse("SET key hello world").unwrap(),
            Command::Set("key".to_owned(), "hello world".to_owned())
        );
    }

    #[test]
    fn set_missing_value_is_an_error() {
        assert!(matches!(parse("SET key"), Err(ParseError::MissingValue(_))));
    }

    #[test]
    fn set_missing_key_is_an_error() {
        assert!(matches!(parse("SET"), Err(ParseError::MissingKey(_))));
    }

    #[test]
    fn parses_get() {
        assert_eq!(parse("GET key").unwrap(), Command::Get("key".to_owned()));
    }

    #[test]
    fn get_missing_key_is_an_error() {
        assert!(matches!(parse("GET"), Err(ParseError::MissingKey(_))));
    }

    #[test]
    fn get_with_extra_argument_is_an_error() {
        assert!(matches!(
            parse("GET key extra"),
            Err(ParseError::ExtraArgument(_))
        ));
    }

    #[test]
    fn parses_delete() {
        assert_eq!(
            parse("DELETE key").unwrap(),
            Command::Delete("key".to_owned())
        );
    }

    #[test]
    fn delete_with_extra_argument_is_an_error() {
        assert!(matches!(
            parse("DELETE key extra"),
            Err(ParseError::ExtraArgument(_))
        ));
    }

    #[test]
    fn command_names_are_case_insensitive() {
        assert_eq!(
            parse("set key value").unwrap(),
            Command::Set("key".to_owned(), "value".to_owned())
        );
    }

    #[test]
    fn unknown_command_is_an_error() {
        assert!(matches!(
            parse("FROB key"),
            Err(ParseError::UnknownCommand(cmd)) if cmd == "FROB"
        ));
    }
}
