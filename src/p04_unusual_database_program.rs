// https://protohackers.com/problem/4

use std::collections::HashMap;
use tokio::net::UdpSocket;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("Problem 4 - Unusual Database Program");

    let udp_address = std::env::var("UDP_ADDRESS").unwrap_or_else(|_| "0.0.0.0".into());
    let sock = UdpSocket::bind((udp_address, 7878)).await?;
    println!("Listening on port 7878");

    let mut db = HashMap::<String, String>::new();
    const VERSION_KEY: &str = "version";
    db.insert(VERSION_KEY.into(), String::from("Hayes's nice database 🐌"));

    let mut buf = [0; 1000];
    loop {
        let (len, addr) = sock.recv_from(&mut buf).await?;
        let data = std::str::from_utf8(&buf[..len]).expect("msg is not valid UTF-8");
        if data.contains('=') {
            // Insert request
            let (key, value) = data.split_once('=').unwrap();
            if key != VERSION_KEY {
                db.insert(key.into(), value.into());
            }
        } else {
            // Query request
            let value = db.get(data).map_or("", String::as_str);
            sock.send_to(format!("{data}={value}").as_bytes(), addr)
                .await?;
        }
    }
}
