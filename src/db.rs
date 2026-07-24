use std::{
    collections::HashMap,
    io::{self},
    path::PathBuf,
};

use thiserror::Error;

use crate::{
    command::Command,
    wal::{Wal, WalEntry},
};

pub struct Database {
    data: HashMap<String, String>,
    wal: Wal,
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("error: no value found for key '{0}'")]
    InvalidKey(String),

    #[error(transparent)]
    Io(#[from] io::Error),
}

impl Database {
    pub fn open(path: PathBuf) -> Result<Self, DatabaseError> {
        let mut wal = Wal::open(path)?;
        let mut data = HashMap::new();

        wal.replay(|entry| match entry {
            WalEntry::Set(k, v) => {
                data.insert(k, v);
            }
            WalEntry::Del(k) => {
                data.remove(&k);
            }
        })?;

        Ok(Self { data, wal })
    }

    pub fn execute(&mut self, cmd: Command) -> Result<String, DatabaseError> {
        match cmd {
            Command::Set(key, value) => {
                self.set(key, value)?;
                Ok("OK".to_owned())
            }

            Command::Get(key) => Ok(format!("VALUE {}", self.get(&key)?)),

            Command::Delete(key) => {
                self.delete(&key)?;
                Ok("OK".to_owned())
            }

            Command::Exit => {
                std::process::exit(0);
            }
        }
    }

    fn set(&mut self, key: String, value: String) -> Result<(), DatabaseError> {
        self.wal.append_set(&key, &value)?;
        self.data.insert(key, value);

        self.check_compaction()?;
        Ok(())
    }

    fn get(&self, key: &str) -> Result<&str, DatabaseError> {
        self.data
            .get(key)
            .map(String::as_str)
            .ok_or(DatabaseError::InvalidKey(key.to_owned()))
    }

    fn delete(&mut self, key: &str) -> Result<(), DatabaseError> {
        if !self.data.contains_key(key) {
            return Err(DatabaseError::InvalidKey(key.to_owned()));
        }
        self.wal.append_del(key)?;
        self.data.remove(key);

        self.check_compaction()?;
        Ok(())
    }

    fn check_compaction(&mut self) -> io::Result<()> {
        let size = self.wal.file_size()?;

        if size > 16 * 1024 * 1024 {
            self.wal.compact(
                self.data.iter().map(|(k, v)| (k.as_str(), v.as_str())),
            )?;
        }

        Ok(())
    }
}
