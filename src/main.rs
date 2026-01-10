mod core;
mod domain;
mod infrastructure;

use crate::core::load_balancer::run_load_balancer;
use crate::domain::backend_conn::ConnString;
use crate::infrastructure::smart_tcp_pool::SmartTcpConnPool;

#[tokio::main]
async fn main() {
    let listen_addr = "127.0.0.1:8080";
    let target_addr = "127.0.0.1:3000";

    let pool = SmartTcpConnPool::new(vec![
        ConnString::new_from_address(target_addr)
            .expect("Invalid adress");
        10
    ]);

    run_load_balancer(listen_addr, target_addr, pool).await.expect("Failed to run load balancer");
}
