use tokio::net::TcpStream;
use uuid::Uuid;

pub trait TcpConnectionPool {
    async fn get_connection(&mut self) -> Option<TcpStream>;
    fn connection_closed(&mut self);
}


pub trait SmartTcpConnectionPool {
    async fn get_connection(&mut self, user_id: Uuid) -> Option<TcpStream>;
}
