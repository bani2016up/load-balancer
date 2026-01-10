use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Mutex;
use tokio::net::TcpStream;

use crate::domain::backend_conn::ConnString;
use crate::domain::tcp_conn_pool::FastTcpPool;

#[derive(Clone)]
pub struct ConnectionPool {
    backends: Arc<Vec<ConnString>>,
    current: Arc<AtomicUsize>,
    pools: Arc<Vec<Mutex<VecDeque<TcpStream>>>>,
    max_pool_size: usize,
}

impl ConnectionPool {

    pub fn new(backends: Vec<ConnString>, max_pool_size: usize) -> ConnectionPool {
        ConnectionPool {
            backends: Arc::new(backends.clone()),
            current: Arc::new(AtomicUsize::new(0)),
            pools: Arc::new(backends.iter().map(|_| Mutex::new(VecDeque::new())).collect()),
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
