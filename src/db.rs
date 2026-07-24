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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path() -> PathBuf {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("db.log");
        // Leak the TempDir so the directory sticks around for the test;
        // the OS reclaims it regardless once the process exits.
        std::mem::forget(dir);
        path
    }

    #[test]
    fn set_then_get_round_trips() {
        let mut db = Database::open(temp_path()).expect("open");
        db.execute(Command::Set("key".to_owned(), "value".to_owned()))
            .expect("set");

        assert_eq!(
            db.execute(Command::Get("key".to_owned())).unwrap(),
            "VALUE value".to_owned()
        );
    }

    #[test]
    fn get_missing_key_is_an_error() {
        let mut db = Database::open(temp_path()).expect("open");
        let err = db.execute(Command::Get("nope".to_owned())).unwrap_err();
        assert!(matches!(err, DatabaseError::InvalidKey(k) if k == "nope"));
    }

    #[test]
    fn delete_removes_key() {
        let mut db = Database::open(temp_path()).expect("open");
        db.execute(Command::Set("key".to_owned(), "value".to_owned()))
            .expect("set");
        db.execute(Command::Delete("key".to_owned())).expect("delete");

        let err = db.execute(Command::Get("key".to_owned())).unwrap_err();
        assert!(matches!(err, DatabaseError::InvalidKey(_)));
    }

    #[test]
    fn delete_missing_key_is_an_error() {
        let mut db = Database::open(temp_path()).expect("open");
        let err = db.execute(Command::Delete("nope".to_owned())).unwrap_err();
        assert!(matches!(err, DatabaseError::InvalidKey(k) if k == "nope"));
    }

    #[test]
    fn data_survives_reopen() {
        let path = temp_path();

        let mut db = Database::open(path.clone()).expect("open");
        db.execute(Command::Set("a".to_owned(), "1".to_owned()))
            .expect("set a");
        db.execute(Command::Set("b".to_owned(), "2".to_owned()))
            .expect("set b");
        db.execute(Command::Delete("a".to_owned())).expect("delete a");
        drop(db);

        let mut db = Database::open(path).expect("reopen");
        assert!(matches!(
            db.execute(Command::Get("a".to_owned())),
            Err(DatabaseError::InvalidKey(_))
        ));
        assert_eq!(
            db.execute(Command::Get("b".to_owned())).unwrap(),
            "VALUE 2".to_owned()
        );
    }
}
