use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Wal {
    path: PathBuf,
    file: File,
}

#[derive(Debug)]
pub enum WalEntry {
    Set(String, String),
    Del(String),
}

const MAX_RECORD_LEN: u32 = 64 * 1024 * 1024; // 64 MiB

impl Wal {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;

        lock_exclusive(&file, &path)?;

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

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&self.path)?;
        lock_exclusive(&file, &self.path)?;
        self.file = file;

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

fn lock_exclusive(file: &File, path: &Path) -> io::Result<()> {
    file.try_lock().map_err(|err| {
        let err: io::Error = err.into();
        if err.kind() == io::ErrorKind::WouldBlock {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "database file '{}' is already open (and locked) by \
                     another crabdb process",
                    path.display()
                ),
            )
        } else {
            err
        }
    })
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

fn invalid_data(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

fn check_record_len(len: u32, field: &str) -> io::Result<()> {
    if len > MAX_RECORD_LEN {
        return Err(invalid_data(format!(
            "WAL record {field} length {len} exceeds maximum of \
             {MAX_RECORD_LEN} bytes; the log file may be corrupted"
        )));
    }

    Ok(())
}

fn read_entry<R: Read>(reader: &mut R) -> io::Result<Option<WalEntry>> {
    let mut record_type = [0; 1];
    if !try_read_exact(reader, &mut record_type)? {
        return Ok(None);
    }

    let mut checksum = crc32fast::Hasher::new();
    checksum.update(&record_type);

    let record_type = RecordType::try_from(record_type[0])?;

    let Some(key_len) = read_u32_checked(reader, &mut checksum)? else {
        return Ok(None);
    };
    check_record_len(key_len, "key")?;

    let mut key = vec![0; key_len as usize];
    if !try_read_exact(reader, &mut key)? {
        return Ok(None);
    }
    checksum.update(&key);

    let entry = match record_type {
        RecordType::Set => {
            let Some(value_len) = read_u32_checked(reader, &mut checksum)?
            else {
                return Ok(None);
            };
            check_record_len(value_len, "value")?;

            let mut value = vec![0; value_len as usize];
            if !try_read_exact(reader, &mut value)? {
                return Ok(None);
            }
            checksum.update(&value);

            WalEntry::Set(
                String::from_utf8(key)
                    .map_err(|_| invalid_data("invalid UTF-8"))?,
                String::from_utf8(value)
                    .map_err(|_| invalid_data("invalid UTF-8"))?,
            )
        }

        RecordType::Del => WalEntry::Del(
            String::from_utf8(key)
                .map_err(|_| invalid_data("invalid UTF-8"))?,
        ),
    };

    let Some(stored) = read_u32(reader)? else {
        return Ok(None);
    };

    let computed = checksum.finalize();
    if stored != computed {
        return Err(invalid_data(format!(
            "WAL checksum mismatch (expected {computed:#010x}, found \
             {stored:#010x}); the log file may be corrupted"
        )));
    }

    Ok(Some(entry))
}

fn read_u32<R: Read>(reader: &mut R) -> io::Result<Option<u32>> {
    let mut buf = [0; 4];
    if try_read_exact(reader, &mut buf)? {
        Ok(Some(u32::from_le_bytes(buf)))
    } else {
        Ok(None)
    }
}

fn read_u32_checked<R: Read>(
    reader: &mut R,
    checksum: &mut crc32fast::Hasher,
) -> io::Result<Option<u32>> {
    let mut buf = [0; 4];
    if !try_read_exact(reader, &mut buf)? {
        return Ok(None);
    }
    checksum.update(&buf);
    Ok(Some(u32::from_le_bytes(buf)))
}

fn write_set<W: Write>(
    writer: &mut W,
    key: &str,
    value: &str,
) -> io::Result<()> {
    let mut checksum = crc32fast::Hasher::new();
    let record_type = [RecordType::Set as u8];
    let key_len = (key.len() as u32).to_le_bytes();
    let value_len = (value.len() as u32).to_le_bytes();

    checksum.update(&record_type);
    checksum.update(&key_len);
    checksum.update(key.as_bytes());
    checksum.update(&value_len);
    checksum.update(value.as_bytes());

    writer.write_all(&record_type)?;
    writer.write_all(&key_len)?;
    writer.write_all(key.as_bytes())?;
    writer.write_all(&value_len)?;
    writer.write_all(value.as_bytes())?;
    writer.write_all(&checksum.finalize().to_le_bytes())?;

    Ok(())
}

fn write_del<W: Write>(writer: &mut W, key: &str) -> io::Result<()> {
    let mut checksum = crc32fast::Hasher::new();
    let record_type = [RecordType::Del as u8];
    let key_len = (key.len() as u32).to_le_bytes();

    checksum.update(&record_type);
    checksum.update(&key_len);
    checksum.update(key.as_bytes());

    writer.write_all(&record_type)?;
    writer.write_all(&key_len)?;
    writer.write_all(key.as_bytes())?;
    writer.write_all(&checksum.finalize().to_le_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn temp_path() -> PathBuf {
        let dir = tempfile::tempdir().expect("create temp dir");
        // Leak the TempDir so it isn't cleaned up while the test still
        // needs the path; the OS will reclaim it either way.
        let path = dir.path().join("db.log");
        std::mem::forget(dir);
        path
    }

    #[test]
    fn round_trips_set_and_del_through_replay() {
        let path = temp_path();
        let mut wal = Wal::open(path.clone()).expect("open wal");

        wal.append_set("a", "1").expect("append set a");
        wal.append_set("b", "2").expect("append set b");
        wal.append_del("a").expect("append del a");
        drop(wal);

        let mut wal = Wal::open(path).expect("reopen wal");
        let mut applied = Vec::new();
        wal.replay(|entry| applied.push(entry)).expect("replay");

        let rendered: Vec<(String, Option<String>)> = applied
            .into_iter()
            .map(|entry| match entry {
                WalEntry::Set(k, v) => (k, Some(v)),
                WalEntry::Del(k) => (k, None),
            })
            .collect();

        assert_eq!(
            rendered,
            vec![
                ("a".to_owned(), Some("1".to_owned())),
                ("b".to_owned(), Some("2".to_owned())),
                ("a".to_owned(), None),
            ]
        );
    }

    #[test]
    fn replay_tolerates_a_truncated_trailing_record() {
        let path = temp_path();
        let mut wal = Wal::open(path.clone()).expect("open wal");
        wal.append_set("foo", "bar").expect("append set foo");
        wal.append_set("baz", "qux").expect("append set baz");
        drop(wal);

        // Simulate a crash mid-write of the last record by chopping a few
        // trailing bytes off the file.
        let len = std::fs::metadata(&path).expect("stat").len();
        let file = OpenOptions::new().write(true).open(&path).expect("open");
        file.set_len(len - 3).expect("truncate");
        drop(file);

        let mut wal = Wal::open(path).expect("reopen wal");
        let mut applied = Vec::new();
        wal.replay(|entry| applied.push(entry)).expect("replay must not error");

        // Only the first, untouched record should have survived.
        assert_eq!(applied.len(), 1);
        assert!(
            matches!(&applied[0], WalEntry::Set(k, v) if k == "foo" && v == "bar")
        );
    }

    #[test]
    fn replay_rejects_a_corrupted_record() {
        let path = temp_path();
        let mut wal = Wal::open(path.clone()).expect("open wal");
        wal.append_set("foo", "bar").expect("append set foo");
        drop(wal);

        // Flip a byte inside the value's content bytes specifically (as
        // opposed to some arbitrary file offset, which could land inside a
        // length prefix instead and just exercise the truncation path).
        // Length prefixes stay valid, so the record still fully reads, but
        // its checksum no longer matches.
        let mut bytes = std::fs::read(&path).expect("read");
        let value_pos = bytes
            .windows(3)
            .position(|w| w == b"bar")
            .expect("find value bytes in record");
        bytes[value_pos] ^= 0xFF;
        std::fs::write(&path, &bytes).expect("write corrupted");

        let mut wal = Wal::open(path).expect("reopen wal");
        let result = wal.replay(|_| {});

        let err = result.expect_err("corrupted record must be rejected");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn replay_rejects_an_oversized_length_prefix() {
        // Hand-craft a record whose key length prefix is absurd, and make
        // sure it's rejected up front instead of attempting a huge alloc.
        let mut bytes = Vec::new();
        bytes.push(RecordType::Set as u8);
        bytes.extend_from_slice(&(u32::MAX).to_le_bytes());

        let mut reader = Cursor::new(bytes);
        let result = read_entry(&mut reader);

        let err = result.expect_err("oversized length must be rejected");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn open_fails_when_already_locked_by_another_handle() {
        let path = temp_path();
        let _first =
            Wal::open(path.clone()).expect("first open should succeed");

        let second = Wal::open(path);
        let err = second.expect_err("second concurrent open should fail");
        assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
    }

    #[test]
    fn compact_rewrites_log_to_only_current_entries() {
        let path = temp_path();
        let mut wal = Wal::open(path.clone()).expect("open wal");
        wal.append_set("a", "1").expect("append a");
        wal.append_set("b", "2").expect("append b");
        wal.append_del("a").expect("append del a");

        wal.compact([("b", "2")].into_iter()).expect("compact");
        drop(wal);

        let mut wal = Wal::open(path).expect("reopen after compact");
        let mut applied = Vec::new();
        wal.replay(|entry| applied.push(entry)).expect("replay");

        assert_eq!(applied.len(), 1);
        assert!(
            matches!(&applied[0], WalEntry::Set(k, v) if k == "b" && v == "2")
        );
    }
}
