use tokio::net::TcpStream;
use crate::domain::tcp_conn_pool::TcpConnectionPool;

pub struct RoundTcpConnectionPool {
    target_addr: String,
    max_connections: usize,
    current_connections: usize,
    available_connections: Vec<TcpStream>,
}

impl TcpConnectionPool for RoundTcpConnectionPool {
    fn new(target_addr: &str, max_connections: usize) -> RoundTcpConnectionPool {
        RoundTcpConnectionPool {
            target_addr: target_addr.to_string(),
            max_connections,
            current_connections: 0,
            available_connections: Vec::with_capacity(max_connections),
        }
    }

    async fn get_connection(&mut self) -> Option<TcpStream> {
        if let Some(stream) = self.available_connections.pop() {
            return Some(stream);
        }

        let total_connections = self.current_connections + self.available_connections.len();
        if total_connections < self.max_connections {
            match TcpStream::connect(&self.target_addr).await {
                Ok(stream) => {
                    self.current_connections += 1;
                    Some(stream)
                }
                Err(_) => None,
            }
        } else {
            None
        }
    }

    fn connection_closed(&mut self) {
        self.current_connections -= 1;
    }

}
