use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, Read, Write},
    path::PathBuf,
};

pub struct Wal {
    path: PathBuf,
    file: File,
}

pub enum WalEntry {
    Set(String, String),
    Del(String),
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
        let mut reader = BufReader::new(&self.file);

        while let Some(entry) = read_entry(&mut reader)? {
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
            write_set(&mut file, key, value)?;
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
        write_set(&mut self.file, key, value)?;
        self.file.sync_data()?;

        Ok(())
    }

    pub fn append_del(&mut self, key: &str) -> io::Result<()> {
        write_del(&mut self.file, key)?;
        self.file.sync_data()?;

        Ok(())
    }

    pub fn file_size(&self) -> io::Result<u64> {
        Ok(self.file.metadata()?.len())
    }
}

#[repr(u8)]
enum RecordType {
    Set = 1,
    Del = 2,
}

impl TryFrom<u8> for RecordType {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(RecordType::Set),
            2 => Ok(RecordType::Del),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown record type: {value}"),
            )),
        }
    }
}

fn try_read_exact<R: Read>(reader: &mut R, buf: &mut [u8]) -> io::Result<bool> {
    match reader.read_exact(buf) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(err) => Err(err),
    }
}

fn read_entry<R: Read>(reader: &mut R) -> io::Result<Option<WalEntry>> {
    let mut record_type = [0; 1];
    if !try_read_exact(reader, &mut record_type)? {
        return Ok(None);
    }

    let record_type = u8::from_le_bytes(record_type);
    let record_type = RecordType::try_from(record_type)?;

    let Some(key_len) = read_u32(reader)? else {
        return Ok(None);
    };

    let mut key = vec![0; key_len as usize];
    if !try_read_exact(reader, &mut key)? {
        return Ok(None);
    }

    match record_type {
        RecordType::Set => {
            let Some(value_len) = read_u32(reader)? else {
                return Ok(None);
            };

            let mut value = vec![0; value_len as usize];
            if !try_read_exact(reader, &mut value)? {
                return Ok(None);
            }

            Ok(Some(WalEntry::Set(
                String::from_utf8(key).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8")
                })?,
                String::from_utf8(value).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8")
                })?,
            )))
        }

        RecordType::Del => {
            Ok(Some(WalEntry::Del(String::from_utf8(key).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8")
            })?)))
        }
    }
}

fn read_u32<R: Read>(reader: &mut R) -> io::Result<Option<u32>> {
    let mut buf = [0; 4];
    if try_read_exact(reader, &mut buf)? {
        Ok(Some(u32::from_le_bytes(buf)))
    } else {
        Ok(None)
    }
}

fn write_set<W: Write>(
    writer: &mut W,
    key: &str,
    value: &str,
) -> io::Result<()> {
    writer.write_all(&[RecordType::Set as u8])?;
    writer.write_all(&(key.len() as u32).to_le_bytes())?;
    writer.write_all(key.as_bytes())?;
    writer.write_all(&(value.len() as u32).to_le_bytes())?;
    writer.write_all(value.as_bytes())?;

    Ok(())
}

fn write_del<W: Write>(writer: &mut W, key: &str) -> io::Result<()> {
    writer.write_all(&[RecordType::Del as u8])?;
    writer.write_all(&(key.len() as u32).to_le_bytes())?;
    writer.write_all(key.as_bytes())?;

    Ok(())
}
