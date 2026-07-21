use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::Path,
};

pub struct Wal {
    file: File,
}

impl Wal {
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(path)?;

        Ok(Self { file })
    }

    pub fn replay(
        &mut self,
        mut apply: impl FnMut(WalEntry),
    ) -> io::Result<()> {
        let reader = BufReader::new(&self.file);

        for line in reader.lines() {
            let Ok(line) = line else { break };
            let mut parts = line.splitn(3, '\t');

            let entry = match parts.next() {
                Some("SET") => match (parts.next(), parts.next()) {
                    (Some(k), Some(v)) => {
                        WalEntry::Set(k.to_owned(), v.to_owned())
                    }
                    _ => break, // malformed — likely a truncated write, stop replay
                },
                Some("DEL") => match parts.next() {
                    Some(k) => WalEntry::Del(k.to_owned()),
                    None => break,
                },
                _ => break, // unrecognized line — stop replay
            };

            apply(entry);
        }

        Ok(())
    }

    pub fn append_set(&mut self, key: &str, value: &str) -> io::Result<()> {
        writeln!(self.file, "SET\t{key}\t{value}")?;
        self.file.flush()
    }

    pub fn append_del(&mut self, key: &str) -> io::Result<()> {
        writeln!(self.file, "DEL\t{key}")?;
        self.file.flush()
    }
}

pub enum WalEntry {
    Set(String, String),
    Del(String),
}
