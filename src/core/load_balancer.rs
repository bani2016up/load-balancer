use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io;
use uuid::Uuid;
use tokio::net::{TcpListener, TcpStream};

use crate::domain::request::{Request, Status};
use crate::domain::tcp_conn_pool::SmartTcpConnectionPool;



pub async fn run_load_balancer<TcpPool>(
    listen_addr: &str,
    target_addr: &str,
    pool: TcpPool,
) -> Result<(), Box<dyn std::error::Error>>
where
    TcpPool: SmartTcpConnectionPool + Send + Sync + 'static,
{
    let listener = TcpListener::bind(listen_addr).await?;
    let pool = Arc::new(Mutex::new(pool));

    println!("Load balancer listening on {}", listen_addr);

    loop {
        let (incoming_stream, addr) = listener.accept().await?;
        println!("New connection from: {}", addr);

        let pool_clone = Arc::clone(&pool);
        let target_addr = target_addr.to_string();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(
                pool_clone,
                incoming_stream,
                addr,
                &target_addr
            ).await {
                eprintln!("Error handling connection: {}", e);
            }
        });
    }
}

async fn handle_connection<TcpPool>(
    pool: Arc<Mutex<TcpPool>>,
    mut incoming_stream: TcpStream,
    client_addr: std::net::SocketAddr,
    target_addr: &str,
) -> Result<(), Box<dyn std::error::Error>>
where
    TcpPool: SmartTcpConnectionPool,
{
    let request_id = Uuid::new_v4();
    let mut request = Request::new(
        target_addr.to_string(),
        request_id,
    );

    request.set_status(Status::Processing);
    let time_start = std::time::Instant::now();


    let mut target_stream = {
        let mut pool_guard = pool.lock().await;
        match pool_guard.get_connection(Uuid::new_v4()).await {
            Some(stream) => stream,
            None => {
                eprintln!("No connections available in pool");
                request.set_status(Status::Failed);
                return Ok(());
            }
        }
    };

    match io::copy_bidirectional(&mut incoming_stream, &mut target_stream).await {
        Ok((from_client, from_server)) => {
            request.set_status(Status::Completed);
            let duration = (std::time::Instant::now() - time_start).as_secs_f64();
            request.set_time_taken(duration);
            println!("Request completed: {:?}", request);
            println!("Bytes: client→server={}, server→client={}", from_client, from_server);
        }
        Err(e) => {
            request.set_status(Status::Failed);
            eprintln!("Error during proxying: {}", e);
        }
    }

    Ok(())
}
