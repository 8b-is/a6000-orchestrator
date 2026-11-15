//! Multiplex incomplete requests before handing complete frames to blocking handlers.
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const LIMIT: usize = 16 * 1024;
const PENDING: usize = 64;
const TICK: Duration = Duration::from_millis(5);
const DEADLINE: Duration = Duration::from_secs(5);

struct Request {
    stream: TcpStream,
    bytes: Vec<u8>,
    length: Option<usize>,
    deadline: Instant,
}

impl Request {
    fn read_ready(&mut self) -> io::Result<bool> {
        let mut chunk = [0; 4096];
        loop {
            let before = self.bytes.len();
            let room = (LIMIT - before).min(chunk.len());
            let n = match self.stream.read(&mut chunk[..room]) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                other => other?,
            };
            self.bytes.extend_from_slice(&chunk[..n]);
            if self.length.is_none() {
                let start = before.saturating_sub(3);
                if let Some(pos) = self.bytes[start..].windows(4).position(|w| w == b"\r\n\r\n") {
                    let end = start + pos + 4;
                    let mut length = None;
                    for line in String::from_utf8_lossy(&self.bytes[..end]).lines().skip(1) {
                        if let Some((k, v)) = line.split_once(':') {
                            if k.trim().eq_ignore_ascii_case("transfer-encoding") {
                                return Err(io::ErrorKind::InvalidData.into());
                            }
                            if k.trim().eq_ignore_ascii_case("content-length") {
                                let size = v.trim().parse::<usize>().map_err(|_| io::ErrorKind::InvalidData)?;
                                if length.replace(size).is_some() || size > LIMIT - end {
                                    return Err(io::ErrorKind::InvalidData.into());
                                }
                            }
                        }
                    }
                    self.length = Some(end + length.unwrap_or(0));
                }
            }
            if let Some(length) = self.length {
                if self.bytes.len() >= length {
                    self.bytes.truncate(length);
                    return Ok(true);
                }
            }
            if self.bytes.len() == LIMIT {
                return Err(io::ErrorKind::InvalidData.into());
            }
        }
    }
}

pub fn serve(
    incoming: impl Iterator<Item = io::Result<TcpStream>>,
    workers: usize,
    handle: impl Fn(TcpStream, Vec<u8>) + Sync,
) {
    let (accept, sockets) = mpsc::sync_channel::<TcpStream>(PENDING);
    let (dispatch, jobs) = mpsc::sync_channel::<(TcpStream, Vec<u8>)>(PENDING);
    let jobs = Mutex::new(jobs);
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let job = jobs.lock().unwrap().recv();
                match job {
                    Ok((stream, bytes)) => handle(stream, bytes),
                    Err(_) => break,
                }
            });
        }
        scope.spawn(move || {
            let mut pending: Vec<Request> = Vec::new();
            let mut accepting = true;
            while accepting || !pending.is_empty() {
                if accepting && pending.len() < PENDING {
                    let next = if pending.is_empty() {
                        sockets.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
                    } else {
                        sockets.recv_timeout(TICK)
                    };
                    match next {
                        Ok(stream) if stream.set_nonblocking(true).is_ok() => pending.push(Request {
                            stream, bytes: Vec::new(), length: None, deadline: Instant::now() + DEADLINE,
                        }),
                        Err(mpsc::RecvTimeoutError::Disconnected) => accepting = false,
                        _ => {}
                    }
                } else {
                    thread::sleep(TICK);
                }
                let mut i = 0;
                while i < pending.len() {
                    if Instant::now() >= pending[i].deadline {
                        pending.swap_remove(i);
                        continue;
                    }
                    match pending[i].read_ready() {
                        Ok(false) => i += 1,
                        Ok(true) => {
                            let req = pending.swap_remove(i);
                            if let Err(mpsc::TrySendError::Full((mut stream, _))) =
                                dispatch.try_send((req.stream, req.bytes)) {
                                let _ = stream.write(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                            }
                        }
                        Err(_) => { pending.swap_remove(i); }
                    }
                }
            }
        });
        for socket in incoming {
            match socket {
                Ok(stream) => { if accept.send(stream).is_err() { break; } }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => { eprintln!("HTTP accept: {e}"); thread::sleep(Duration::from_millis(100)); }
            }
        }
        drop(accept);
    });
}
