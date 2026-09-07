//! Dependency-free HTTP fixture server. Every request is recorded with timing.
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const TEST_USER_AGENT: &str =
    "mcp-crates/0.1.0 (https://example.invalid/test-repository; tests@example.invalid)";

#[derive(Clone, Debug)]
pub struct Request {
    pub path: String,
    pub headers: String,
    pub at: Instant,
}

pub struct Reply {
    pub status: u16,
    pub body: String,
    pub headers: Vec<(String, String)>,
    pub delay: Duration,
}
impl Reply {
    pub fn json(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into(),
            headers: Vec::new(),
            delay: Duration::ZERO,
        }
    }
    pub fn status(status: u16) -> Self {
        Self {
            status,
            ..Self::json("{}")
        }
    }
}

pub struct Mock {
    pub url: String,
    requests: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Mock {
    pub fn start(handler: impl Fn(&Request, usize) -> Reply + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_requests = requests.clone();
        let worker_stop = stop.clone();
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::SeqCst) {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(_) => break,
                };
                // Accepted sockets can inherit O_NONBLOCK on macOS. These
                // reads/writes are blocking: otherwise an early WouldBlock
                // closes the connection before the request arrives.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 1024];
                while !bytes.ends_with(b"\r\n\r\n") && bytes.len() < 16384 {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => bytes.extend_from_slice(&buffer[..n]),
                    }
                }
                if !bytes.ends_with(b"\r\n\r\n") {
                    continue;
                }
                let headers = String::from_utf8(bytes).unwrap();
                let path = headers.split_whitespace().nth(1).unwrap().to_owned();
                let request = Request {
                    path,
                    headers,
                    at: Instant::now(),
                };
                let index = {
                    let mut recorded = worker_requests.lock().unwrap();
                    let index = recorded.len();
                    recorded.push(request.clone());
                    index
                };
                let reply = handler(&request, index);
                let ready = Instant::now() + reply.delay;
                while Instant::now() < ready && !worker_stop.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(5));
                }
                if worker_stop.load(Ordering::SeqCst) {
                    break;
                }
                let mut headers = format!(
                    "HTTP/1.1 {} Fixture\r\nConnection: close\r\nContent-Type: application/json\r\n",
                    reply.status
                );
                if !reply
                    .headers
                    .iter()
                    .any(|(key, _)| key.eq_ignore_ascii_case("Content-Length"))
                {
                    headers.push_str(&format!("Content-Length: {}\r\n", reply.body.len()));
                }
                for (key, value) in reply.headers {
                    headers.push_str(&format!("{key}: {value}\r\n"));
                }
                headers.push_str("\r\n");
                let _ = stream.write_all(headers.as_bytes());
                let _ = stream.write_all(reply.body.as_bytes());
            }
        });
        Self {
            url,
            requests,
            stop,
            worker: Some(worker),
        }
    }

    pub fn fixtures() -> Self {
        Self::start(|request, _| match request.path.as_str() {
            path if path.starts_with("/api/v1/crates?") => {
                Reply::json(include_str!("../fixtures/search_itoa.json"))
            }
            "/api/v1/crates/itoa" => Reply::json(include_str!("../fixtures/crate_itoa.json")),
            "/api/v1/crates/itoa/versions" => {
                Reply::json(include_str!("../fixtures/versions_itoa.json"))
            }
            _ => Reply::status(404),
        })
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.url.trim_start_matches("http://"));
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
