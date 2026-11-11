mod config;
mod core;
mod domain;
mod infrastructure;

use crate::config::app::AppConfig;
use crate::config::router_map::RouterMap;
use crate::core::load_balancer::run_load_balancer;
use crate::infrastructure::backends::Backends;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};

#[tokio::main]
async fn main() -> io::Result<()> {
    colog::init();
    let listen_addr = "127.0.0.1:8080";
    let target_addr = Ipv4Addr::LOCALHOST;

    let replicas_start = 3000;
    let replicas_count = 20;

    let mut app_config = AppConfig::new();
    app_config.listener(listen_addr.to_string());
    app_config.request_timeout(30);
    if let Some(limit) = std::env::var_os("LOAD_BALANCER_MAX_CONNECTIONS") {
        app_config.max_active_connections = limit
            .to_str()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "LOAD_BALANCER_MAX_CONNECTIONS must be a positive integer",
                )
            })?;
    }

    let mut router_map = RouterMap::new();
    router_map.map_route_range(listen_addr, &target_addr.to_string(), 3000, 3020);

    app_config.router(router_map);
    app_config.build();

    let addresses = (replicas_start..(replicas_start + replicas_count))
        .map(|port| SocketAddr::from((target_addr, port)))
        .collect();
    let backends = Backends::new(addresses);

    run_load_balancer(app_config, backends).await
}
