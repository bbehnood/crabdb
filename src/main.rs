use std::path::Path;

use anyhow::Result;
use crabdb::{db::Database, run};

const LOG_FILE: &str = "db.log";

fn main() -> Result<()> {
    let db = Database::open(Path::new(LOG_FILE))?;
    run(db)?;

    Ok(())
}
