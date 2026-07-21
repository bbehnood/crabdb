use std::io::{self, Write};

use crate::{
    command::{execute, parse},
    db::Database,
};

pub mod command;
pub mod db;
pub mod wal;

pub fn run(mut db: Database) -> io::Result<()> {
    println!("=== CrabDB ===");
    println!("Commands:");
    println!("  SET <key> <value>");
    println!("  GET <key>");
    println!("  DELETE <id>");
    println!("  EXIT");

    loop {
        print!("crabdb> ");
        io::stdout().flush()?;

        let mut input = String::new();
        let bytes = io::stdin().read_line(&mut input)?;

        if bytes == 0 {
            println!();
            break Ok(());
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let cmd = match parse(input) {
            Ok(cmd) => cmd,
            Err(err) => {
                eprintln!("{err}");
                continue;
            }
        };

        match execute(&mut db, cmd) {
            Ok(result) => println!("{result}"),
            Err(err) => eprintln!("{err}"),
        }
    }
}
