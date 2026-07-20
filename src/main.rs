use std::io;

use crabdb::{db::Database, run};

fn main() -> io::Result<()> {
    let db = Database::new();
    run(db)?;

    Ok(())
}
