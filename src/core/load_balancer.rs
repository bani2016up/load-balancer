use log::{info, error};
use dashmap::DashMap;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use tokio::{io::{self, AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}};
use tokio::time::timeout;
use tokio::time::Duration;

use crate::infrastructure::fast_tcp_pool::ConnectionPool;
use crate::domain::tcp_conn_pool::FastTcpPool;



pub async fn run_load_balancer(
    listen_addr: &str,
    pool: ConnectionPool,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(listen_addr).await?;
    let pool = Arc::new(pool);
    let request_counter = Arc::new(AtomicU64::new(0));

    info!("Load balancer listening on {}", listen_addr);

    loop {
        let (mut incoming_stream, addr) = listener.accept().await?;
        let pool = Arc::clone(&pool);
        let request_id = request_counter.fetch_add(1, Ordering::Relaxed);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(pool, incoming_stream, request_id).await {
                error!("Error handling connection: {}", e);
            }
        });
    }
}


async fn handle_connection(
    pool: Arc<ConnectionPool>,
    mut incoming_stream: TcpStream,
    request_id: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();

    let mut backend = pool.get_connection(request_id).await.unwrap();

    match timeout(
        Duration::from_secs(30),
        io::copy_bidirectional(&mut incoming_stream, &mut backend)
    ).await {
        Ok(Ok((sent, received))) => {
            info!("Request {}: {}→{} bytes in {:?}",
                  request_id, sent, received, start.elapsed());
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
