use log::{info, warn};
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;

const RETRY_AFTER_MS: u64 = 1_000;
const CONNECT_ATTEMPTS: usize = 2;

pub struct Backends {
    entries: Box<[Backend]>,
    current: AtomicUsize,
    started: Instant,
}

struct Backend {
    address: SocketAddr,
    active: AtomicUsize,
    retry_at_ms: AtomicU64,
}

pub struct BackendConnection<'a> {
    pub stream: TcpStream,
    _load: BackendLoad<'a>,
}

struct BackendLoad<'a>(&'a Backend);

impl<'a> BackendLoad<'a> {
    fn new(backend: &'a Backend) -> Self {
        backend.active.fetch_add(1, Ordering::Relaxed);
        Self(backend)
    }
}

impl Drop for BackendLoad<'_> {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Backends {
    pub fn new(addresses: Vec<SocketAddr>) -> Self {
        assert!(!addresses.is_empty(), "at least one backend is required");
        Self {
            entries: addresses
                .into_iter()
                .map(|address| Backend {
                    address,
                    active: AtomicUsize::new(0),
                    retry_at_ms: AtomicU64::new(0),
                })
                .collect(),
            current: AtomicUsize::new(0),
            started: Instant::now(),
        }
    }

    pub async fn connect(&self, connect_timeout: Duration) -> io::Result<BackendConnection<'_>> {
        let mut last_error = None;
        for _ in 0..CONNECT_ATTEMPTS {
            let now_ms = self.elapsed_ms();
            let probe_until_ms = now_ms + connect_timeout.as_millis() as u64 + RETRY_AFTER_MS;
            let Some((backend, probing)) = self.select(now_ms, probe_until_ms) else {
                break;
            };
            let load = BackendLoad::new(backend);
            let result = timeout(connect_timeout, TcpStream::connect(backend.address))
                .await
                .unwrap_or_else(|_| {
                    Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "backend connect timed out",
                    ))
                });

            match result {
                Ok(stream) => {
                    if probing {
                        backend.retry_at_ms.store(0, Ordering::Relaxed);
                        info!("Backend {} is available again", backend.address);
                    }
                    return Ok(BackendConnection {
                        stream,
                        _load: load,
                    });
                }
                Err(error) => {
                    if !matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionRefused
                            | io::ErrorKind::ConnectionReset
                            | io::ErrorKind::TimedOut
                            | io::ErrorKind::HostUnreachable
                            | io::ErrorKind::NetworkUnreachable
                    ) {
                        return Err(error);
                    }
                    let previous = backend
                        .retry_at_ms
                        .fetch_max(self.elapsed_ms() + RETRY_AFTER_MS, Ordering::Relaxed);
                    if previous == 0 {
                        warn!("Backend {} connect failed: {}", backend.address, error);
                    }
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| io::Error::other("no backend available")))
    }

    fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn select(&self, now_ms: u64, probe_until_ms: u64) -> Option<(&Backend, bool)> {
        let start = self.current.fetch_add(1, Ordering::Relaxed) % self.entries.len();
        let (before, after) = self.entries.split_at(start);
        loop {
            let mut best = None;
            let mut least_active = usize::MAX;
            for backend in after.iter().chain(before) {
                let retry_at = backend.retry_at_ms.load(Ordering::Relaxed);
                if retry_at > now_ms {
                    continue;
                }
                let active = backend.active.load(Ordering::Relaxed);
                if active < least_active {
                    best = Some((backend, retry_at));
                    least_active = active;
                    if active == 0 {
                        break;
                    }
                }
            }

            let (backend, retry_at) = best?;
            if retry_at == 0 {
                return Some((backend, false));
            }
            // An expired cooldown admits one probe. A canceled probe becomes
            // eligible again when its reservation expires.
            if backend
                .retry_at_ms
                .compare_exchange(
                    retry_at,
                    probe_until_ms,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return Some((backend, true));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::{TcpListener, TcpSocket};

    #[tokio::test]
    async fn selects_least_active_backend_and_rotates_ties() {
        let first = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let second = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let first_addr = first.local_addr().unwrap();
        let second_addr = second.local_addr().unwrap();
        let backends = Backends::new(vec![first_addr, second_addr]);
        let limit = Duration::from_secs(1);

        let first_conn = backends.connect(limit).await.unwrap();
        let second_conn = backends.connect(limit).await.unwrap();
        assert_eq!(first_conn.stream.peer_addr().unwrap(), first_addr);
        assert_eq!(second_conn.stream.peer_addr().unwrap(), second_addr);
        let tied_conn = backends.connect(limit).await.unwrap();
        assert_eq!(tied_conn.stream.peer_addr().unwrap(), first_addr);
        assert_ne!(
            first_conn.stream.local_addr().unwrap(),
            tied_conn.stream.local_addr().unwrap()
        );

        drop(second_conn);
        for _ in 0..2 {
            let next_conn = backends.connect(limit).await.unwrap();
            assert_eq!(next_conn.stream.peer_addr().unwrap(), second_addr);
        }
    }

    #[tokio::test]
    async fn retries_unavailable_backend_and_releases_its_load() {
        let socket = TcpSocket::new_v4().unwrap();
        socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let healthy = listener.local_addr().unwrap();
        let backends = Backends::new(vec![socket.local_addr().unwrap(), healthy]);
        let limit = Duration::from_millis(100);

        let connection = backends.connect(limit).await.unwrap();
        assert_eq!(connection.stream.peer_addr().unwrap(), healthy);
        assert_eq!(backends.entries[0].active.load(Ordering::Relaxed), 0);
        let next = backends.connect(limit).await.unwrap();
        assert_eq!(next.stream.peer_addr().unwrap(), healthy);
    }

    #[test]
    fn cooldown_allows_one_probe_at_a_time() {
        let backends = Backends::new(vec!["127.0.0.1:3000".parse().unwrap()]);
        backends.entries[0]
            .retry_at_ms
            .store(500, Ordering::Relaxed);
        assert!(backends.select(499, 750).is_none());
        assert!(matches!(backends.select(500, 750), Some((_, true))));
        assert!(backends.select(500, 750).is_none());
        assert!(matches!(backends.select(750, 1_000), Some((_, true))));
    }
}
