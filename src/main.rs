mod domain;
mod infrastructure;

use std::sync::Arc;
use tokio::io;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::domain::request::{Request, Status};
use crate::domain::tcp_conn_pool::SmartTcpConnectionPool;
use crate::infrastructure::smart_tcp_pool::{ConnString, SmartTcpConnPool};

#[tokio::main]
async fn main() {
    let listen_addr = "127.0.0.1:8080";
    let target_addr = "127.0.0.1:3000";
    let user_id = Uuid::new_v4();

    let pool = Arc::new(Mutex::new(
        SmartTcpConnPool::new(
            10,
             vec![ConnString::new_from_address(target_addr).expect("UNDEFIEND"); 10]
            )
        )
    );
    let listener = TcpListener::bind(listen_addr).await.unwrap();

    println!("Load balancer listening on {}", listen_addr);

    loop {
        let (mut incoming_stream, _) = listener.accept().await.unwrap();
        let mut request = Request::new(target_addr.to_string());
        let time_start = std::time::Instant::now();
        let pool_clone = Arc::clone(&pool);

        tokio::spawn(async move {
            let mut target_stream = {
                let mut pool_guard = pool_clone.lock().await;
                match pool_guard.get_connection(user_id).await {
                    Some(stream) => stream,
                    None => {
                        eprintln!("No connections available in pool");
                        return;
                    }
                }
            };
            request.set_status(Status::Processing);
            match io::copy_bidirectional(&mut incoming_stream, &mut target_stream).await {
                Ok((from_client, from_server)) => {
                    request.set_status(Status::Completed);
                    let duration = (std::time::Instant::now() - time_start).as_secs_f64();
                    request.set_time_taken(duration);
                    println!("{:?}", request);
                }
                Err(e) => {
                    request.set_status(Status::Failed);
                    eprintln!("Error during proxying: {}", e);
                }
            }
        });
    }
}
