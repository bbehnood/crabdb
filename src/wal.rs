use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::PathBuf,
};

pub struct Wal {
    path: PathBuf,
    file: File,
}

impl Wal {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;

        Ok(Self { path, file })
    }

    pub fn replay(
        &mut self,
        mut apply: impl FnMut(WalEntry),
    ) -> io::Result<()> {
        let reader = BufReader::new(&self.file);

        for line in reader.lines() {
            let line = line?;
            let mut parts = line.splitn(3, '\t');

            let entry = match parts.next() {
                Some("SET") => match (parts.next(), parts.next()) {
                    (Some(k), Some(v)) => {
                        WalEntry::Set(k.to_owned(), v.to_owned())
                    }
                    _ => break,
                },

                Some("DEL") => match parts.next() {
                    Some(k) => WalEntry::Del(k.to_owned()),
                    None => break,
                },

                _ => break,
            };

            apply(entry);
        }

        Ok(())
    }

    pub fn compact<'a>(
        &mut self,
        entries: impl Iterator<Item = (&'a str, &'a str)>,
    ) -> io::Result<()> {
        let temp = self.path.with_extension("compact");

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temp)?;

        for (key, value) in entries {
            writeln!(file, "SET\t{key}\t{value}")?;
        }

        file.sync_all()?;
        drop(file);

        std::fs::rename(&temp, &self.path)?;

        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&self.path)?;

        Ok(())
    }

    pub fn append_set(&mut self, key: &str, value: &str) -> io::Result<()> {
        writeln!(self.file, "SET\t{key}\t{value}")?;
        self.file.sync_data()
    }

    pub fn append_del(&mut self, key: &str) -> io::Result<()> {
        writeln!(self.file, "DEL\t{key}")?;
        self.file.sync_data()
    }

    pub fn file_size(&self) -> io::Result<u64> {
        Ok(self.file.metadata()?.len())
    }
}

pub enum WalEntry {
    Set(String, String),
    Del(String),
}
