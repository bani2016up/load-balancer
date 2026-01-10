mod core;
mod domain;
mod infrastructure;

use crate::core::load_balancer::run_load_balancer;
use crate::domain::backend_conn::ConnString;
use crate::infrastructure::fast_tcp_pool::ConnectionPool;


#[tokio::main]
async fn main() {
    colog::init();
    let listen_addr = "127.0.0.1:8080";
    let target_addr = "127.0.0.1";

    let replicas_start = 3000;
    let replicas_count = 20;

    let mut connections: Vec<ConnString> = Vec::new();

    for i in replicas_start..(replicas_start + replicas_count) {
        connections.push(ConnString::new(target_addr.to_string(), i));
    }

    let pool = ConnectionPool::new(connections, replicas_count as usize);

    run_load_balancer(listen_addr, pool).await.expect("Failed to run load balancer");
}
