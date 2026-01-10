use log::{error, info};

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::{io, time::Duration};
use tokio::time::timeout;
use tokio::net::{TcpListener, TcpStream};

use crate::{config::app::AppConfig, domain::tcp_conn_pool::FastTcpPool};
use crate::infrastructure::fast_tcp_pool::ConnectionPool;

pub async fn run_load_balancer(
    app_config: AppConfig,
    pool: ConnectionPool,
) -> Result<(), Box<dyn std::error::Error>> {

    let listen_addr = &app_config.listen_addr.unwrap();

    let listener = TcpListener::bind(listen_addr).await?;
    let pool = Arc::new(pool);
    let request_counter = Arc::new(AtomicU64::new(0));

    info!("Load balancer listening on {}", listen_addr);

    loop {
        let (mut incoming_stream, addr) = listener.accept().await?;
        let pool = Arc::clone(&pool);
        let request_id = request_counter.fetch_add(1, Ordering::Relaxed);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(pool, incoming_stream, request_id, app_config.request_timout_sec).await {
                error!("Error handling connection: {}", e);
            }
        });
    }
}

async fn handle_connection(
    pool: Arc<ConnectionPool>,
    mut incoming_stream: TcpStream,
    request_id: u64,
    timeout_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();

    let mut backend = pool.get_connection(request_id).await.unwrap();

    match timeout(
        Duration::from_secs(timeout_ms),
        io::copy_bidirectional(&mut incoming_stream, &mut backend),
    )
    .await
    {
        Ok(Ok((sent, received))) => {
            info!(
                "Request {}: {}→{} bytes in {:?}",
                request_id,
                sent,
                received,
                start.elapsed()
            );
        }
        Ok(Err(e)) => {
            error!("Copy error {}: {}", request_id, e);
        }
        Err(_) => {
            error!("Timeout for {}", request_id);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::backend_conn::ConnString;
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
