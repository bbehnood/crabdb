use std::path::PathBuf;

use anyhow::Result;
use crabdb::{db::Database, run};

const LOG_FILE: &str = "db.log";

#[tokio::main]
async fn main() -> Result<()> {
    let db = Database::open(PathBuf::from(LOG_FILE))?;
    run(db).await?;

    Ok(())
}
