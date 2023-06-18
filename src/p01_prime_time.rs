// https://protohackers.com/problem/1

use serde::Deserialize;
use serde_json::{json, Number};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

use crate::BoxedErr;

#[derive(Deserialize)]
struct Request {
    method: String,
    number: Number,
}

pub async fn run() -> JoinHandle<Result<(), BoxedErr>> {
    println!("Problem 1 - Prime Time");

    let listener = TcpListener::bind("0.0.0.0:7878").await.unwrap();
    println!("Listening on port 7878");

    tokio::spawn(async move {
        loop {
            let (stream, addr) = listener.accept().await?;

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream).await {
                    eprintln!("error handling connection {}: {:?}", addr, e);
                };
            });
        }
    })
}

async fn handle_connection(mut stream: TcpStream) -> Result<(), BoxedErr> {
    let (read, mut write) = stream.split();
    let mut read = BufReader::new(read);

    loop {
        let mut buf = vec![];
        if read.read_until(b'\n', &mut buf).await? == 0 {
            // EOF reached
            break;
        }
        let conformant_number = match serde_json::from_slice::<Request>(&buf) {
            Ok(req) => {
                if req.method == "isPrime" {
                    Some(req.number)
                } else {
                    None
                }
            }
            Err(_) => None,
        };
        // if the request is malformed, send back a malformed response and close the connection.
        if conformant_number.is_none() {
            write.write_all("no".as_bytes()).await?;
            break;
        }
        let answer = conformant_number
            .unwrap()
            .as_u64()
            .filter(is_prime)
            .is_some();
        let response = json!({"method": "isPrime", "prime": answer});
        write
            .write_all(format!("{}\n", response).as_bytes())
            .await?;
    }
    Ok(())
}

// naive prime algo
fn is_prime(x: &u64) -> bool {
    for i in 2..=(*x as f64).sqrt() as u64 {
        if x % i == 0 {
            return false;
        }
    }
    *x != 0 && *x != 1
}
