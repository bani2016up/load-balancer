mod config;
mod core;
mod domain;
mod infrastructure;

use crate::config::app::AppConfig;
use crate::config::router_map::RouterMap;
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

    let mut app_config = AppConfig::new();
    app_config.listener(listen_addr.to_string());
    app_config.request_timeout(30);

    let mut router_map = RouterMap::new();
    router_map.map_route_range(listen_addr, target_addr, 3000, 3020);

    app_config.router(router_map);
    app_config.build();

    let mut connections: Vec<ConnString> = Vec::new();

    for i in replicas_start..(replicas_start + replicas_count) {
        connections.push(ConnString::new(target_addr.to_string(), i));
    }

    let pool = ConnectionPool::new(connections, replicas_count as usize);

    run_load_balancer(app_config, pool)
        .await
        .expect("Failed to run load balancer");
}
