use std::path::PathBuf;

use anyhow::Result;
use crabdb::{db::Database, run};

const LOG_FILE: &str = "db.log";

fn main() -> Result<()> {
    let db = Database::open(PathBuf::from(LOG_FILE))?;
    run(db)?;

    Ok(())
}
