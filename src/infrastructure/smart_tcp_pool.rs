use uuid::Uuid;
use std::collections::HashMap;
use tokio::net::TcpStream;
use crate::domain::tcp_conn_pool::SmartTcpConnectionPool;
use crate::domain::backend_conn::ConnString;

#[derive(Debug, Clone)]
pub struct SmartTcpConnPool {
    available_backends: Vec<ConnString>,
    user_session_map: HashMap<Uuid, Uuid>,
    user_last_connection_time: HashMap<Uuid, std::time::Instant>,
}

impl SmartTcpConnPool {
    pub fn new(backends_hosts: Vec<ConnString>) -> Self {
        SmartTcpConnPool {
            available_backends: backends_hosts,
            user_session_map: std::collections::HashMap::new(),
            user_last_connection_time: std::collections::HashMap::new(),
        }
    }
}

impl SmartTcpConnectionPool for SmartTcpConnPool {
    async fn get_connection(&mut self, user_id: Uuid) -> Option<TcpStream> {
        let conn_string = match self.user_session_map.get(&user_id) {
            Some(conn_id) => {
                self.available_backends.iter().find(|c| c.get_uuid() == *conn_id)
            }
            None => {
                let conn_string = self.available_backends.first()?;
                self.user_session_map.insert(user_id, conn_string.get_uuid());
                self.user_last_connection_time.insert(user_id, std::time::Instant::now());
                Some(conn_string)
            }
        }?;

        match TcpStream::connect(&conn_string.address()).await {
            Ok(stream) => Some(stream),
            Err(e) => {
                eprintln!("Failed to connect to backend {}: {}", conn_string.get_uuid(), e);
                None
            }
        }
    }
}
