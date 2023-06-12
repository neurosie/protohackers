// https://protohackers.com/problem/5

use lazy_regex::regex_replace;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("Problem 5 - Mob in the Middle");

    let listener = TcpListener::bind("0.0.0.0:7878").await?;
    println!("Listening on port 7878");

    loop {
        let (stream, addr) = listener.accept().await?;

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                eprintln!("error handling connection {}: {:?}", addr, e);
            };
        });
    }
}

async fn handle_connection(mut client_stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut server_stream = TcpStream::connect("chat.protohackers.com:16963").await?;
    let (client_read, mut client_write) = client_stream.split();
    let (server_read, mut server_write) = server_stream.split();
    let mut client_buf = vec![];
    let mut server_buf = vec![];
    let mut client_read = BufReader::new(client_read);
    let mut server_read = BufReader::new(server_read);

    loop {
        tokio::select! {
            res = client_read.read_until(b'\n', &mut client_buf) => {
                if res? == 0 {
                    println!("client: EOF");
                    return Ok(());
                } else {
                    let line = String::from_utf8(std::mem::take(&mut client_buf)).expect("received invalid UTF-8");
                    println!("client: {}", line.trim_end());
                    server_write.write_all(rewrite_boguscoin(&line).as_bytes()).await?;
                }
            },
            res = server_read.read_until(b'\n', &mut server_buf) => {
                if res? == 0 {
                    println!("server: EOF");
                    return Ok(());
                } else {
                    let line = String::from_utf8(std::mem::take(&mut server_buf)).expect("received invalid UTF-8");
                    println!("server: {}", line.trim_end());
                    client_write.write_all(rewrite_boguscoin(&line).as_bytes()).await?;
                }
            },
        }
    }
}

fn rewrite_boguscoin(input: &str) -> String {
    input
        .split(' ')
        .map(|word| {
            regex_replace!(r"(^7[a-zA-Z0-9]{25,34})(\n|$)"m, word, |_, _, suffix| {
                format!("7YWHMfk9JZe0LM0g1ZauHuiSxhI{suffix}")
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}
