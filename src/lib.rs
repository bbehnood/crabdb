use std::{
    io::{self},
    sync::{Arc, Mutex},
};

use anyhow::Result;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

use crate::{command::parse, db::Database};

pub mod command;
pub mod db;
pub mod wal;

pub async fn run(db: Database) -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;
    let db = Arc::new(Mutex::new(db));

    println!("Listening on 127.0.0.1:6379");

    loop {
        let (stream, _) = listener.accept().await?;
        let db = Arc::clone(&db);

        tokio::spawn(async move {
            if let Err(err) = handle_client(stream, db).await {
                eprintln!("Client error: {err}");
            }
        });
    }
}

async fn handle_client(
    stream: TcpStream,
    db: Arc<Mutex<Database>>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();

    let mut reader = BufReader::new(reader);

    let mut line = String::new();

    while reader.read_line(&mut line).await? != 0 {
        line.clear();

        let response = match parse(line.trim_end()) {
            Ok(cmd) => db
                .lock()
                .unwrap()
                .execute(cmd)
                .unwrap_or_else(|err| err.to_string()),

            Err(err) => err.to_string(),
        };

        writer.write_all(response.as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }

    Ok(())
}
