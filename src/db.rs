use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::Path,
};

use thiserror::Error;

pub struct Database {
    data: HashMap<String, String>,
    log: File,
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("error: no value found for key '{0}'")]
    InvalidKey(String),

    #[error(transparent)]
    Io(#[from] io::Error),
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, DatabaseError> {
        let mut data = HashMap::new();

        if path.exists() {
            let file = File::open(path)?;
            for line in BufReader::new(file).lines() {
                let line = line?;
                let mut parts = line.splitn(3, '\t');

                match parts.next() {
                    Some("SET") => {
                        let (Some(key), Some(value)) =
                            (parts.next(), parts.next())
                        else {
                            break;
                        };

                        data.insert(key.to_owned(), value.to_owned());
                    }

                    Some("DEL") => {
                        let Some(key) = parts.next() else {
                            break;
                        };

                        data.remove(key)
                            .ok_or(DatabaseError::InvalidKey(key.to_owned()))?;
                    }

                    _ => {}
                }
            }
        }

        let log = OpenOptions::new().create(true).append(true).open(path)?;

        Ok(Self { data, log })
    }

    pub fn set(
        &mut self,
        key: String,
        value: String,
    ) -> Result<(), DatabaseError> {
        writeln!(self.log, "SET\t{key}\t{value}")?;
        self.log.flush()?;
        self.data.insert(key, value);

        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<&str, DatabaseError> {
        self.data
            .get(key)
            .map(String::as_str)
            .ok_or(DatabaseError::InvalidKey(key.to_owned()))
    }

    pub fn delete(&mut self, key: &str) -> Result<(), DatabaseError> {
        if !self.data.contains_key(key) {
            return Err(DatabaseError::InvalidKey(key.to_owned()));
        }

        writeln!(self.log, "DEL\t{key}")?;
        self.log.flush()?;
        self.data.remove(key);

        Ok(())
    }
}
