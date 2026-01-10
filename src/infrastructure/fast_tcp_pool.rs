use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::domain::backend_conn::ConnString;
use crate::domain::tcp_conn_pool::FastTcpPool;

#[derive(Clone)]
pub struct ConnectionPool {
    pub backends: Arc<Vec<ConnString>>,
    pub current: Arc<AtomicUsize>,
    pub pools: Arc<Vec<Mutex<VecDeque<TcpStream>>>>,
    pub max_pool_size: usize,
}

impl ConnectionPool {
    pub fn new(backends: Vec<ConnString>, max_pool_size: usize) -> ConnectionPool {
        ConnectionPool {
            backends: Arc::new(backends.clone()),
            current: Arc::new(AtomicUsize::new(0)),
            pools: Arc::new(
                backends
                    .iter()
                    .map(|_| Mutex::new(VecDeque::new()))
                    .collect(),
            ),
            max_pool_size,
        }
    }

    pub async fn return_connection(&self, backend_idx: usize, stream: TcpStream) {
        let mut pool = self.pools[backend_idx].lock().await;
        if pool.len() < self.max_pool_size {
            pool.push_back(stream);
        }
    }
}

impl FastTcpPool for ConnectionPool {
    async fn get_connection(&self, session_id: u64) -> Option<TcpStream> {
        let backend_idx = self.current.fetch_add(1, Ordering::Relaxed) % self.backends.len();

        let mut pool = self.pools[backend_idx].lock().await;
        if let Some(stream) = pool.pop_front() {
            return Some(stream);
        }

        let backend = &self.backends[backend_idx];
        TcpStream::connect(&backend.address()).await.ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn constructor_test() {
        let backends = vec![
            ConnString::new("127.0.0.1".to_string(), 8080),
            ConnString::new("127.0.0.1".to_string(), 8081),
        ];
        let pool = ConnectionPool::new(backends.clone(), 10);

        assert_eq!(pool.backends.len(), 2);
        assert_eq!(pool.max_pool_size, 10);
        assert_eq!(pool.current.load(Ordering::Relaxed), 0);
        assert_eq!(pool.pools.len(), 2);
    }

    #[tokio::test]
    async fn get_connection_round_robin_test() {
        let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr1 = listener1.local_addr().unwrap();

        let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener2.local_addr().unwrap();

        let backends = vec![
            ConnString::new(addr1.ip().to_string(), addr1.port()),
            ConnString::new(addr2.ip().to_string(), addr2.port()),
        ];
        let pool = ConnectionPool::new(backends, 10);

        let conn1 = pool.get_connection(1).await.unwrap();
        assert_eq!(conn1.peer_addr().unwrap(), addr1);

        let conn2 = pool.get_connection(2).await.unwrap();
        assert_eq!(conn2.peer_addr().unwrap(), addr2);

        let conn3 = pool.get_connection(3).await.unwrap();
        assert_eq!(conn3.peer_addr().unwrap(), addr1);
    }

    #[tokio::test]
    async fn return_connection_test() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let backends = vec![ConnString::new(addr.ip().to_string(), addr.port())];
        let pool = ConnectionPool::new(backends, 10);

        let stream = TcpStream::connect(addr).await.unwrap();
        pool.return_connection(0, stream).await;

        let pool_guard = pool.pools[0].lock().await;
        assert_eq!(pool_guard.len(), 1);
    }

    #[tokio::test]
    async fn return_connection_respects_max_pool_size_test() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let backends = vec![ConnString::new(addr.ip().to_string(), addr.port())];
        let pool = ConnectionPool::new(backends, 2);

        for _ in 0..3 {
            let stream = TcpStream::connect(addr).await.unwrap();
            pool.return_connection(0, stream).await;
        }

        let pool_guard = pool.pools[0].lock().await;
        assert_eq!(pool_guard.len(), 2);
    }

    #[tokio::test]
    async fn get_connection_reuses_pooled_connection_test() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let backends = vec![ConnString::new(addr.ip().to_string(), addr.port())];
        let pool = ConnectionPool::new(backends, 10);

        let stream = TcpStream::connect(addr).await.unwrap();
        let local_addr = stream.local_addr().unwrap();
        pool.return_connection(0, stream).await;

        let reused_stream = pool.get_connection(1).await.unwrap();
        assert_eq!(reused_stream.local_addr().unwrap(), local_addr);
    }

    #[tokio::test]
    async fn get_connection_creates_new_when_pool_empty_test() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let backends = vec![ConnString::new(addr.ip().to_string(), addr.port())];
        let pool = ConnectionPool::new(backends, 10);

        let conn = pool.get_connection(1).await.unwrap();
        assert_eq!(conn.peer_addr().unwrap(), addr);

        let pool_guard = pool.pools[0].lock().await;
        assert_eq!(pool_guard.len(), 0);
    }

    #[tokio::test]
    async fn concurrent_get_connection_test() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let backends = vec![ConnString::new(addr.ip().to_string(), addr.port())];
        let pool = ConnectionPool::new(backends, 10);

        let mut handles = vec![];
        for i in 0..10 {
            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move { pool_clone.get_connection(i).await });
            handles.push(handle);
        }

        let mut success_count = 0;
        for handle in handles {
            if handle.await.unwrap().is_some() {
                success_count += 1;
            }
        }

        assert_eq!(success_count, 10);
    }

    #[tokio::test]
    async fn get_connection_handles_invalid_backend_test() {
        let backends = vec![ConnString::new("127.0.0.1".to_string(), 9999)];
        let pool = ConnectionPool::new(backends, 10);

        let result = pool.get_connection(1).await;
        assert!(result.is_none());
    }
}
