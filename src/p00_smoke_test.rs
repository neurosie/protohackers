// https://protohackers.com/problem/0

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

use crate::BoxedErr;

pub async fn run(port: Option<u16>) -> JoinHandle<Result<(), BoxedErr>> {
    println!("Problem 0 - Smoke Test");

    let port = port.unwrap_or(7878);
    let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap();
    println!("Listening on port {port}");

    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await?;

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream).await {
                    eprintln!("error echoing to socket: {:?}", e);
                };
            });
        }
    })
}

async fn handle_connection(mut stream: TcpStream) -> Result<(), BoxedErr> {
    let mut buf = vec![];
    stream.read_to_end(&mut buf).await?;
    stream.write_all(&buf).await?;

    Ok(())
}

#[tokio::test]
async fn test() {
    let port = 7800;
    let server_task = run(Some(port)).await;

    let mut client = TcpStream::connect(("0.0.0.0", port)).await.unwrap();
    let mut buf = vec![];
    client.write_all(b"Hello world!").await.unwrap();
    assert!(client.try_read(&mut buf).is_err());

    client.shutdown().await.unwrap();
    client.read_buf(&mut buf).await.unwrap();
    assert_eq!(&buf, &b"Hello world!");

    server_task.abort();
}
