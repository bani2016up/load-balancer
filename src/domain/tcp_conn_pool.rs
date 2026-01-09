use tokio::net::TcpStream;

pub trait TcpConnectionPool {
    fn new(target_addr: &str, max_connections: usize) -> Self;
    async fn get_connection(&mut self) -> Option<TcpStream>;
    fn connection_closed(&mut self);
}
