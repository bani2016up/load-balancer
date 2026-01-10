use tokio::net::TcpStream;
use uuid::Uuid;

pub trait TcpConnectionPool {
    async fn get_connection(&mut self) -> Option<TcpStream>;
    fn connection_closed(&mut self);
}


pub trait SmartTcpConnectionPool {
    fn get_connection(&mut self, user_id: Uuid) -> impl std::future::Future<Output = Option<TcpStream>> + Send;
}


pub trait FastTcpPool {
    fn get_connection(&self, session_id: u64) -> impl std::future::Future<Output = Option<TcpStream>> + Send;
}
