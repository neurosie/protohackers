// https://protohackers.com/problem/0

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("Problem 0 - Smoke Test");

    let listener = TcpListener::bind("0.0.0.0:7878").await?;
    println!("Listening on port 7878");

    loop {
        let (stream, _) = listener.accept().await?;

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                eprintln!("error echoing to socket: {:?}", e);
            };
        });
    }
}

async fn handle_connection(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = vec![];
    stream.read_to_end(&mut buf).await?;
    stream.write_all(&buf).await?;

    Ok(())
}
