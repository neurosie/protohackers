use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();

    for stream in listener.incoming() {
        thread::spawn(|| -> std::io::Result<()> {
            handle_connection(stream?)?;
            Ok(())
        });
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream) -> std::io::Result<()> {
    // let stream = BufReader::new(stream);
    let mut buf = vec![];
    stream.read_to_end(&mut buf)?;
    stream.write_all(&buf)?;

    Ok(())
}
