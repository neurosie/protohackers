// https://protohackers.com/problem/6

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::TcpListener,
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
    time::{Instant, Interval},
};

use crate::BoxedErr;

#[derive(Debug)]
enum ClientMessage {
    PlateReading {
        plate: String,
        timestamp: u32,
        road: u16,
        mile: u16,
        limit: u16,
    },
    DispatcherId {
        roads: Vec<u16>,
        ticketer: Sender<Ticket>,
    },
}

#[derive(Debug)]
struct Ticket {
    plate: String,
    road: u16,
    mile_1: u16,
    timestamp_1: u32,
    mile_2: u16,
    timestamp_2: u32,
    speed: u16,
}

pub async fn run(port: Option<u16>) -> JoinHandle<Result<(), BoxedErr>> {
    println!("Problem 6 - Speed Daemon");

    let port = port.unwrap_or(7878);
    let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap();
    println!("Listening on port {port}");

    tokio::spawn(async move {
        let (tx, mut rx) = mpsc::channel(100);

        // Main bookkeeping for ticketing. handle incoming messages from connection tasks,
        // calculate speeds, and possibly send back tickets.
        tokio::spawn(async move {
            let mut dispatchers = HashMap::<u16 /* road */, Vec<Sender<Ticket>>>::new();
            let mut ticket_queue = vec![];
            let mut readings = HashMap::<
                (String /* plate */, u16 /* road */),
                BTreeMap<u32 /* timestamp */, u16 /* mile */>,
            >::new();
            let mut ticketed_days = HashSet::<(String /* plate */, u32 /* day */)>::new();

            loop {
                match rx.recv().await {
                    Some(ClientMessage::PlateReading {
                        plate,
                        timestamp,
                        road,
                        mile,
                        limit,
                    }) => {
                        let day = timestamp / 86400;
                        if ticketed_days.contains(&(plate.clone(), day)) {
                            continue;
                        }
                        let relevant_readings = readings.entry((plate.clone(), road)).or_default();
                        let violation: Option<(u16, u32, u16, u32, u16)> = 'violation: {
                            // Check the reading before
                            if let Some((&timestamp_2, &mile_2)) =
                                relevant_readings.range(..timestamp).rev().next()
                            {
                                if !ticketed_days.contains(&(plate.clone(), timestamp_2 / 86400)) {
                                    let speed_mph = (mile.abs_diff(mile_2) as u32 * 60 * 60 * 100
                                        / (timestamp - timestamp_2))
                                        as u16;
                                    if speed_mph > limit * 100 {
                                        break 'violation Some((
                                            mile_2,
                                            timestamp_2,
                                            mile,
                                            timestamp,
                                            speed_mph,
                                        ));
                                    }
                                }
                            }
                            // Check the reading after
                            if let Some((&timestamp_2, &mile_2)) =
                                relevant_readings.range(timestamp..).next()
                            {
                                if !ticketed_days.contains(&(plate.clone(), timestamp_2 / 86400)) {
                                    let speed_mph = (mile.abs_diff(mile_2) as u32 * 60 * 60 * 100
                                        / (timestamp_2 - timestamp))
                                        as u16;
                                    if speed_mph > limit * 100 {
                                        break 'violation Some((
                                            mile,
                                            timestamp,
                                            mile_2,
                                            timestamp_2,
                                            speed_mph,
                                        ));
                                    }
                                }
                            }
                            None
                        };
                        relevant_readings.insert(timestamp, mile);

                        if let Some((mile_1, timestamp_1, mile_2, timestamp_2, speed)) = violation {
                            for day in timestamp_1 / 86400..=timestamp_2 / 86400 {
                                ticketed_days.insert((plate.clone(), day));
                            }

                            // Attempt to dispatch ticket.
                            let mut ticket = Some(Ticket {
                                plate,
                                road,
                                mile_1,
                                timestamp_1,
                                mile_2,
                                timestamp_2,
                                speed,
                            });
                            // Try the dispatchers for this road. If one is disconnected, send will return an error.
                            if let Some(ticketers) = dispatchers.get_mut(&road) {
                                while let Some(ticketer) = ticketers.first() {
                                    if let Err(err) = ticketer.send(ticket.take().unwrap()).await {
                                        ticket = Some(err.0);
                                        ticketers.remove(0);
                                    } else {
                                        break;
                                    }
                                }
                            }
                            // No dispatcher available, enqueue the ticket.
                            if let Some(ticket) = ticket {
                                ticket_queue.push(ticket);
                            }
                        }
                    }
                    Some(ClientMessage::DispatcherId { roads, ticketer }) => {
                        // Dispatch any pending tickets that match.
                        'outer: for i in 0..ticket_queue.len() {
                            while i < ticket_queue.len() && roads.contains(&ticket_queue[i].road) {
                                if let Err(err) = ticketer.send(ticket_queue.remove(i)).await {
                                    ticket_queue.push(err.0);
                                    break 'outer;
                                }
                            }
                        }
                        for road in roads {
                            dispatchers.entry(road).or_default().push(ticketer.clone());
                        }
                    }
                    None => return,
                };
            }
        });

        loop {
            let (stream, addr) = listener.accept().await?;
            let tx = tx.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, tx).await {
                    eprintln!("error handling connection {}: {:?}", addr, e);
                };
            });
        }
    })
}

#[derive(Debug)]
enum ClientIdentity {
    Unset,
    Camera { road: u16, mile: u16, limit: u16 },
    Dispatcher,
}

#[derive(Debug)]
enum TaskEvent {
    DispatcherIdentfied { ticket_receiver: Receiver<Ticket> },
    HeartbeatRequested { interval: u32 },
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    tx: Sender<ClientMessage>,
) -> Result<(), BoxedErr> {
    let (read, write) = stream.into_split();
    let mut read = BufReader::new(read);
    let mut write = BufWriter::new(write);

    let (task_tx, mut task_rx) = mpsc::channel(2);

    // Task for reading from the stream
    let mut read_task: JoinHandle<Result<Option<String>, BoxedErr>> = tokio::spawn(async move {
        let mut identity = ClientIdentity::Unset;
        let mut requested_heartbeat = false;
        loop {
            match read.read_u8().await {
                // IAmCamera
                Ok(0x80) => match identity {
                    ClientIdentity::Unset => {
                        let road = read.read_u16().await?;
                        let mile = read.read_u16().await?;
                        let limit = read.read_u16().await?;
                        identity = ClientIdentity::Camera { road, mile, limit };
                    }
                    _ => return Ok(Some("id is already set".into())),
                },
                // IAmDispatcher
                Ok(0x81) => match identity {
                    ClientIdentity::Unset => {
                        let num_roads = read.read_u8().await?;
                        let mut roads = Vec::with_capacity(num_roads as usize);
                        for _ in 0..num_roads {
                            roads.push(read.read_u16().await?);
                        }
                        if !roads.is_empty() {
                            let (ticket_tx, ticket_rx) = mpsc::channel(10);
                            tx.send(ClientMessage::DispatcherId {
                                roads,
                                ticketer: ticket_tx,
                            })
                            .await?;
                            task_tx
                                .send(TaskEvent::DispatcherIdentfied {
                                    ticket_receiver: ticket_rx,
                                })
                                .await?;
                        }
                        identity = ClientIdentity::Dispatcher;
                    }
                    _ => return Ok(Some("id is already set".into())),
                },
                // Plate
                Ok(0x20) => match identity {
                    ClientIdentity::Camera { road, mile, limit } => {
                        let len = read.read_u8().await?;
                        let mut plate = Vec::with_capacity(len as usize);
                        for _ in 0..len {
                            plate.push(read.read_u8().await?);
                        }
                        let plate = String::from_utf8(plate).expect("plate is invalid UTF-8");
                        let timestamp = read.read_u32().await?;
                        tx.send(ClientMessage::PlateReading {
                            plate,
                            timestamp,
                            road,
                            mile,
                            limit,
                        })
                        .await?;
                    }
                    _ => return Ok(Some("not a camera".into())),
                },
                // WantHeartbeat
                Ok(0x40) => {
                    if requested_heartbeat {
                        return Ok(Some("already requested heartbeat".into()));
                    }
                    requested_heartbeat = true;
                    let interval = read.read_u32().await?;
                    if interval > 0 {
                        task_tx
                            .send(TaskEvent::HeartbeatRequested { interval })
                            .await?;
                    }
                }
                // Client disconnected
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
                _ => return Ok(Some("invalid message type".into())),
            }
        }
    });

    // Task for writing to the stream.
    let mut ticket_receiver: Option<Receiver<Ticket>> = None;
    let mut heartbeat: Option<Interval> = None;

    loop {
        tokio::select! {
            result = &mut read_task => {
                return match result? {
                    Ok(Some(reportable_error)) => {
                        write.write_u8(0x10).await?; // Error
                        write.write_u8(reportable_error.len() as u8).await?;
                        write.write_all(reportable_error.as_bytes()).await?;
                        write.flush().await?;
                        Err(reportable_error.into())
                    }
                    Ok(None) => Ok(()),
                    Err(e) => Err(e),
                }
            }
            msg = task_rx.recv() => {
                match msg {
                    Some(TaskEvent::DispatcherIdentfied { ticket_receiver: ticket_rx }) => {
                        ticket_receiver = Some(ticket_rx);
                    },
                    Some(TaskEvent::HeartbeatRequested {interval}) => {
                        heartbeat = Some(tokio::time::interval(Duration::from_millis(interval as u64 * 100)));
                    }
                    _ => {},
                }
            }
            ticket = maybe_receive(&mut ticket_receiver) => {
                let Ticket { plate, road, mile_1, timestamp_1, mile_2, timestamp_2, speed } = ticket.expect("ticketer channel closed. this shouldn't happen");
                write.write_u8(0x21).await?; // Ticket
                write.write_u8(plate.len() as u8).await?;
                write.write_all(plate.as_bytes()).await?;
                write.write_u16(road).await?;
                write.write_u16(mile_1).await?;
                write.write_u32(timestamp_1).await?;
                write.write_u16(mile_2).await?;
                write.write_u32(timestamp_2).await?;
                write.write_u16(speed).await?;
                write.flush().await?;
            }
            _ = maybe_tick(&mut heartbeat) => {
                write.write_u8(0x41).await?; // Heartbeat
                write.flush().await?;
            }
        }
    }
}

// There may be a smart way to write something like map_or_pend for using Optional futures in a select.
// But without any type wizardry:
async fn maybe_receive<T>(rx: &mut Option<Receiver<T>>) -> Option<T> {
    match rx.as_mut() {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}

async fn maybe_tick(interval: &mut Option<Interval>) -> Instant {
    match interval.as_mut() {
        Some(interval) => interval.tick().await,
        None => std::future::pending().await,
    }
}

#[tokio::test]
async fn test_ticket() {
    use tokio::net::TcpStream;

    let port = 7806;
    let server_task = run(Some(port)).await;

    tokio::try_join!(
        tokio::spawn(async move {
            let mut camera_1 = TcpStream::connect(("0.0.0.0", port)).await.unwrap();
            camera_1
                .write_all(&[0x80, 0x00, 0x7b, 0x00, 0x08, 0x00, 0x3c]) // IAmCamera{road: 123, mile: 8, limit: 60}
                .await
                .unwrap();
            camera_1
                .write_all(&[0x20, 0x04, 0x55, 0x4e, 0x31, 0x58, 0x00, 0x00, 0x00, 0x00]) // Plate{plate: "UN1X", timestamp: 0}
                .await
                .unwrap();
        }),
        tokio::spawn(async move {
            let mut camera_2 = TcpStream::connect(("0.0.0.0", port)).await.unwrap();
            camera_2
                .write_all(&[0x80, 0x00, 0x7b, 0x00, 0x09, 0x00, 0x3c]) // IAmCamera{road: 123, mile: 9, limit: 60}
                .await
                .unwrap();
            camera_2
                .write_all(&[0x20, 0x04, 0x55, 0x4e, 0x31, 0x58, 0x00, 0x00, 0x00, 0x2d]) // Plate{plate: "UN1X", timestamp: 45}
                .await
                .unwrap();
        }),
        tokio::spawn(async move {
            let mut dispatcher = TcpStream::connect(("0.0.0.0", port)).await.unwrap();
            dispatcher
                .write_all(&[0x81, 0x01, 0x00, 0x7b]) // IAmDispatcher{roads: [123]}
                .await
                .unwrap();
            let mut buf = [0u8; 22];
            dispatcher.read_exact(&mut buf).await.unwrap();
            assert_eq!(
                buf,
                [
                    0x21, 0x04, 0x55, 0x4e, 0x31, 0x58, 0x00, 0x7b, 0x00, 0x08, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x2d, 0x1f, 0x40
                ] // Ticket{plate: "UN1X", road: 123, mile1: 8, timestamp1: 0, mile2: 9, timestamp2: 45, speed: 8000}
            );
        })
    )
    .unwrap();

    server_task.abort();
}

#[tokio::test]
async fn test_heartbeat() {
    use tokio::net::TcpStream;
    use tokio::time::sleep;

    let port = 7806;
    let server_task = run(Some(port)).await;

    let mut client = TcpStream::connect(("0.0.0.0", port)).await.unwrap();
    client
        .write_all(&[0x40, 0x00, 0x00, 0x00, 0x0a]) // WantHeartbeat{interval: 10}
        .await
        .unwrap();

    sleep(Duration::from_secs(10)).await;
    let mut buf = vec![];
    client.read_buf(&mut buf).await.unwrap();

    assert!(buf.len() == 10);

    server_task.abort();
}
