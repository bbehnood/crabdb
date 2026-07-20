use thiserror::Error;

use crate::db::Database;

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

    #[error("error: unknown command '{0}'")]
    UnknownCommand(String),
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ExecError {
    #[error("error: no value found for key '{0}'")]
    InvalidKey(String),
}

const SET_COMMAND_USAGE: &str = "SET <key> <value>";
const GET_COMMAND_USAGE: &str = "GET <key>";
const DELETE_COMMAND_USAGE: &str = "DELETE <key>";

pub fn parse(input: &str) -> Result<Command, ParseError> {
    let mut parts = input.splitn(3, char::is_whitespace);

    match parts.next() {
        Some(cmd) => match cmd {
            c if c.eq_ignore_ascii_case("SET") => {
                let Some(key) = parts.next() else {
                    return Err(ParseError::MissingKey(SET_COMMAND_USAGE));
                };

                let Some(value) = parts.next() else {
                    return Err(ParseError::MissingValue(SET_COMMAND_USAGE));
                };

                Ok(Command::Set(key.to_owned(), value.to_owned()))
            }

            c if c.eq_ignore_ascii_case("GET") => {
                let Some(key) = parts.next() else {
                    return Err(ParseError::MissingKey(GET_COMMAND_USAGE));
                };

                Ok(Command::Get(key.to_owned()))
            }

            c if c.eq_ignore_ascii_case("DELETE") => {
                let Some(key) = parts.next() else {
                    return Err(ParseError::MissingKey(DELETE_COMMAND_USAGE));
                };

                Ok(Command::Delete(key.to_owned()))
            }

            c if c.eq_ignore_ascii_case("EXIT")
                || c.eq_ignore_ascii_case("QUIT") =>
            {
                Ok(Command::Exit)
            }

            cmd => Err(ParseError::UnknownCommand(cmd.to_owned())),
        },

        None => unreachable!("Empty commands don't reach parsing."),
    }
}

pub fn execute(db: &mut Database, cmd: Command) -> Result<&str, ExecError> {
    match cmd {
        Command::Set(key, value) => {
            db.set(key, value);
            Ok("OK")
        }

        Command::Get(key) => {
            db.get(&key).ok_or(ExecError::InvalidKey(key.to_string()))
        }

        Command::Delete(key) => {
            if db.delete(&key) {
                Ok("OK")
            } else {
                Err(ExecError::InvalidKey(key.to_string()))
            }
        }

        Command::Exit => {
            std::process::exit(0);
        }
    }
}
