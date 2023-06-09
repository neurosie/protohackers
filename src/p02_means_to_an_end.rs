// https://protohackers.com/problem/2

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub async fn run(listener: TcpListener) -> Result<(), Box<dyn std::error::Error>> {
    println!("Problem 2 - Means to an End");
    loop {
        let (stream, addr) = listener.accept().await?;

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                eprintln!("error handling connection {}: {:?}", addr, e);
            };
        });
    }
}

async fn handle_connection(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let (mut read, mut write) = stream.split();
    let mut entries = vec![];

    loop {
        let mut buf = [0; 9];
        read.read_exact(&mut buf).await?;
        let cmd = buf[0];
        let arg1 = i32::from_be_bytes(buf[1..=4].try_into().unwrap());
        let arg2 = i32::from_be_bytes(buf[5..=8].try_into().unwrap());
        if cmd == b'I' {
            entries.push((arg1, arg2)); // (timestamp, price)
        } else {
            let (mintime, maxtime) = (arg1, arg2);
            let mut sum: i64 = 0;
            let mut count = 0;
            for &(timestamp, price) in &entries {
                if mintime <= timestamp && timestamp <= maxtime {
                    sum += price as i64;
                    count += 1;
                }
            }
            let avg = if count > 0 { sum / count } else { 0 };
            write.write_all(&(avg as i32).to_be_bytes()).await?;
        }
    }
}
