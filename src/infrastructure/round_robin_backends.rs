use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpStream;

pub struct RoundRobinBackends {
    backends: Vec<SocketAddr>,
    current: AtomicUsize,
}

impl RoundRobinBackends {
    pub fn new(backends: Vec<SocketAddr>) -> Self {
        assert!(!backends.is_empty(), "at least one backend is required");
        Self {
            backends,
            current: AtomicUsize::new(0),
        }
    }

    pub async fn connect(&self) -> io::Result<TcpStream> {
        let index = self.current.fetch_add(1, Ordering::Relaxed) % self.backends.len();
        TcpStream::connect(self.backends[index]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn connects_to_backends_in_round_robin_order() {
        let first = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let second = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let first_addr = first.local_addr().unwrap();
        let second_addr = second.local_addr().unwrap();
        let backends = RoundRobinBackends::new(vec![first_addr, second_addr]);

        let first_conn = backends.connect().await.unwrap();
        let second_conn = backends.connect().await.unwrap();
        let next_conn = backends.connect().await.unwrap();
        assert_eq!(first_conn.peer_addr().unwrap(), first_addr);
        assert_eq!(second_conn.peer_addr().unwrap(), second_addr);
        assert_eq!(next_conn.peer_addr().unwrap(), first_addr);
        assert_ne!(
            first_conn.local_addr().unwrap(),
            next_conn.local_addr().unwrap()
        );
    }
}
