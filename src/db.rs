use std::{
    collections::HashMap,
    io::{self},
    path::Path,
};

use thiserror::Error;

use crate::wal::{Wal, WalEntry};

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
    pub fn open(path: &Path) -> Result<Self, DatabaseError> {
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

    pub fn set(
        &mut self,
        key: String,
        value: String,
    ) -> Result<(), DatabaseError> {
        self.wal.append_set(&key, &value)?;
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
        self.wal.append_del(key)?;
        self.data.remove(key);
        Ok(())
    }
}
