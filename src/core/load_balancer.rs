use log::{debug, error, info};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::time::timeout;

use crate::config::app::AppConfig;
use crate::infrastructure::round_robin_backends::RoundRobinBackends;

pub async fn run_load_balancer(
    app_config: AppConfig,
    backends: RoundRobinBackends,
) -> io::Result<()> {
    let limit = app_config.max_active_connections;
    if !(1..=Semaphore::MAX_PERMITS).contains(&limit) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "connection limit is out of range",
        ));
    }
    let listen_addr = app_config.listen_addr.as_ref().unwrap();
    let listener = TcpListener::bind(listen_addr).await?;
    info!("Load balancer listening on {}", listen_addr);
    serve(listener, backends, app_config).await
}

async fn serve(
    listener: TcpListener,
    backends: RoundRobinBackends,
    app_config: AppConfig,
) -> io::Result<()> {
    let backends = Arc::new(backends);
    let permits = Arc::new(Semaphore::new(app_config.max_active_connections));
    let request_counter = AtomicU64::new(0);
    let connect_timeout = app_config.connect_timeout;
    let relay_timeout = Duration::from_secs(app_config.request_timout_sec);

    loop {
        let (incoming_stream, addr) = listener.accept().await?;
        let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
            debug!("Rejected client {}: active connection limit reached", addr);
            continue;
        };
        let backends = Arc::clone(&backends);
        let request_id = request_counter.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(error) = handle_connection(
                &backends,
                incoming_stream,
                request_id,
                connect_timeout,
                relay_timeout,
            )
            .await
            {
                error!("Connection {}: {}", request_id, error);
            }
        });
    }
}

async fn handle_connection(
    backends: &RoundRobinBackends,
    mut incoming_stream: TcpStream,
    request_id: u64,
    connect_timeout: Duration,
    relay_timeout: Duration,
) -> io::Result<()> {
    let start = Instant::now();
    let mut backend = timeout(connect_timeout, backends.connect())
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "backend connect timed out"))??;

    let (sent, received) = timeout(
        relay_timeout,
        io::copy_bidirectional(&mut incoming_stream, &mut backend),
    )
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "relay timed out"))??;

    info!(
        "Request {}: {}→{} bytes in {:?}",
        request_id,
        sent,
        received,
        start.elapsed()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn backends(addr: SocketAddr) -> RoundRobinBackends {
        RoundRobinBackends::new(vec![addr])
    }

    async fn client_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (proxy, _) = listener.accept().await.unwrap();
        (client, proxy)
    }

    #[tokio::test]
    async fn unavailable_backend_returns_io_error_and_closes_client() {
        let socket = tokio::net::TcpSocket::new_v4().unwrap();
        socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let (mut client, incoming) = client_pair().await;
        let result = handle_connection(
            &backends(socket.local_addr().unwrap()),
            incoming,
            0,
            Duration::from_millis(100),
            Duration::from_secs(1),
        )
        .await;

        assert!(matches!(
            result.unwrap_err().kind(),
            io::ErrorKind::ConnectionRefused | io::ErrorKind::TimedOut
        ));
        assert_eq!(client.read(&mut [0]).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn rejects_at_capacity_and_admits_after_disconnect() {
        let backend = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstreams = backends(backend.local_addr().unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut config = AppConfig::new();
        config.max_active_connections = 1;
        let proxy = tokio::spawn(serve(listener, upstreams, config));

        timeout(Duration::from_secs(2), async {
            let mut first = TcpStream::connect(addr).await.unwrap();
            let (mut upstream, _) = backend.accept().await.unwrap();

            let mut rejected = TcpStream::connect(addr).await.unwrap();
            assert_eq!(rejected.read(&mut [0]).await.unwrap(), 0);

            first.shutdown().await.unwrap();
            assert_eq!(upstream.read(&mut [0]).await.unwrap(), 0);
            upstream.shutdown().await.unwrap();
            assert_eq!(first.read(&mut [0]).await.unwrap(), 0);

            loop {
                let _next = TcpStream::connect(addr).await.unwrap();
                if timeout(Duration::from_millis(20), backend.accept())
                    .await
                    .is_ok()
                {
                    break;
                }
            }
        })
        .await
        .unwrap();
        proxy.abort();
    }

    #[tokio::test]
    async fn relays_large_payloads_across_half_close() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstreams = backends(listener.local_addr().unwrap());
        let (mut client, incoming) = client_pair().await;
        let proxy = tokio::spawn(async move {
            handle_connection(
                &upstreams,
                incoming,
                0,
                Duration::from_secs(1),
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        });
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            stream.read_to_end(&mut request).await.unwrap();
            assert_eq!(request, vec![0x51; 65_536]);
            stream.write_all(&[0x82; 65_536]).await.unwrap();
            stream.shutdown().await.unwrap();
        });
        client.write_all(&[0x51; 65_536]).await.unwrap();
        client.shutdown().await.unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        assert_eq!(response, vec![0x82; 65_536]);
        server.await.unwrap();
        proxy.await.unwrap();
    }
}
