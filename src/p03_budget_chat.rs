// https://protohackers.com/problem/3

// I spent a while on this one debugging a seeming race condition: Sometimes when the checker ran, a later person
// joining the chat room would see 0 other people there. I added printlns and combed through the logs until I
// realized the problem: fly.io was spawning two instances of the app, and connections were split between them.
// A desirable behavior usually I'm sure, but not with an in-memory chat room. `flyctl scale count 1` fixed it.

use std::{collections::HashSet, sync::Arc};

use lazy_regex::regex_is_match;
use parking_lot::Mutex;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::broadcast::{self, Sender},
    task::JoinHandle,
};

use crate::BoxedErr;

#[derive(Clone, Debug)]
enum Message {
    Join { name: String },
    Leave { name: String },
    Message { name: String, text: String },
}

pub async fn run() -> JoinHandle<Result<(), BoxedErr>> {
    println!("Problem 3 - Budget Chat");

    let listener = TcpListener::bind("0.0.0.0:7878").await.unwrap();
    println!("Listening on port 7878");

    tokio::spawn(async move {
        let names = Arc::new(Mutex::new(HashSet::<String>::new()));
        let (tx, _) = broadcast::channel::<Message>(16);

        loop {
            let (stream, addr) = listener.accept().await?;

            let (names, tx) = (Arc::clone(&names), tx.clone());
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, names, tx).await {
                    eprintln!("error handling connection {}: {:?}", addr, e);
                };
            });
        }
    })
}

async fn handle_connection(
    mut stream: TcpStream,
    names: Arc<Mutex<HashSet<String>>>,
    tx: Sender<Message>,
) -> Result<(), BoxedErr> {
    let (read, mut write) = stream.split();
    let mut read = BufReader::new(read);

    // Pre-join, ask for username
    write
        .write_all(
            "Welcome to budgetchat. Enter a display name, letters and digits only:\n".as_bytes(),
        )
        .await?;
    let mut name = String::new();
    read.read_line(&mut name).await?;
    let name = name.trim_end();
    if !regex_is_match!("^[a-zA-Z0-9]+$", name) {
        write
            .write_all("Bad username, try again ;(\n".as_bytes())
            .await?;
        return Ok(());
    }

    // Successfully joined. Add name to the namelist, send join message to others, list users already here.
    let (already_here, mut rx) = {
        let mut names = names.lock();
        let already_here = names.iter().cloned().collect::<Vec<_>>();
        names.insert(name.into());
        if tx.receiver_count() > 0 {
            tx.send(Message::Join { name: name.into() })?;
        }
        (already_here, tx.subscribe())
    };

    write
        .write_all(
            (if already_here.is_empty() {
                "* Greetings. You're the first one here.\n".to_owned()
            } else {
                format!("* Greetings. Who's here: {}\n", already_here.join(", "))
            })
            .as_bytes(),
        )
        .await?;

    // two things now: waiting for TCP messages, and waiting for channel messages
    // Lines is cancel-safe, so it can be used in select!.
    let mut incoming_lines = read.lines();
    loop {
        tokio::select! {
            line = incoming_lines.next_line() => {
                match line? {
                    Some(line) => {
                        tx.send(Message::Message { name: name.into(), text: line })?;
                    },
                    None => {
                        let mut names = names.lock();
                        names.remove(name);
                        tx.send(Message::Leave {name: name.into() })?;
                        return Ok(());
                    },
                }

            },
            msg = rx.recv() => {
                match msg? {
                    Message::Join { name: sender_name } => {
                        write.write_all(format!("* {sender_name} has joined budget chat\n").as_bytes()).await?;
                    },
                    Message::Leave { name: sender_name } => {
                        write.write_all(format!("* {sender_name} has left budget chat\n").as_bytes()).await?;
                    },
                    Message::Message { name: sender_name, text } => {
                        if name != sender_name {
                            write.write_all(format!("[{sender_name}] {text}\n").as_bytes()).await?;
                        }
                    },
                }
            }
        }
    }
}
