mod domain;
mod infrastructure;

use tokio::net::TcpListener;

use std::sync::Arc;
use tokio::io;
use tokio::sync::Mutex;

use crate::domain::tcp_conn_pool::TcpConnectionPool;
use crate::infrastructure::tcp_round_pool::RoundTcpConnectionPool;

#[tokio::main]
async fn main() {
    let listen_addr = "127.0.0.1:8080";
    let target_addr = "127.0.0.1:3000";

    let pool = Arc::new(Mutex::new(RoundTcpConnectionPool::new(target_addr, 10)));
    let listener = TcpListener::bind(listen_addr).await.unwrap();

    println!("Load balancer listening on {}", listen_addr);

    loop {
        let (mut incoming_stream, _) = listener.accept().await.unwrap();
        let pool_clone = Arc::clone(&pool);

        tokio::spawn(async move {
            let mut target_stream = {
                let mut pool_guard = pool_clone.lock().await;
                match pool_guard.get_connection().await {
                    Some(stream) => stream,
                    None => {
                        eprintln!("No connections available in pool");
                        return;
                    }
                }
            };

            match io::copy_bidirectional(&mut incoming_stream, &mut target_stream).await {
                Ok((from_client, from_server)) => {
                    println!(
                        "Client wrote {} bytes; server wrote {} bytes",
                        from_client, from_server
                    );
                }
                Err(e) => {
                    eprintln!("Error during proxying: {}", e);
                }
            }

            let mut pool_guard = pool_clone.lock().await;
            pool_guard.connection_closed();
        });
    }
}
