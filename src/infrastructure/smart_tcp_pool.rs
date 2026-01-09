use uuid::Uuid;
use std::collections::HashMap;
use tokio::net::TcpStream;
use crate::domain::tcp_conn_pool::SmartTcpConnectionPool;



#[derive(Debug, Clone)]
pub struct ConnString {
    uuid: Uuid,
    host: String,
    port: u16,
}

#[derive(Debug, Clone)]
pub struct SmartTcpConnPool {
    available_backends: Vec<ConnString>,
    user_session_map: HashMap<Uuid, Uuid>,
    user_last_connection_time: HashMap<Uuid, std::time::Instant>,
    max_connections: usize,
}

impl ConnString {
    fn new(host: String, port: u16) -> Self {
        ConnString {
            uuid: Uuid::new_v4(),
            host,
            port,
        }
    }

    pub fn new_from_address(address: &str) -> Result<Self, String> {
        let parts: Vec<&str> = address.split(':').collect();
        if parts.len() != 2 {
            return Err("Invalid address format: expected 'host:port'".to_string());
        }
        let host = parts[0].to_string();
        let port = parts[1]
            .parse()
            .map_err(|_| format!("Invalid port number: {}", parts[1]))?;
        Ok(Self::new(host, port))
    }

    fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

}

impl SmartTcpConnPool {
    pub fn new(max_connections: usize, backends_hosts: Vec<ConnString>) -> Self {
        SmartTcpConnPool {
            available_backends: backends_hosts,
            user_session_map: std::collections::HashMap::new(),
            user_last_connection_time: std::collections::HashMap::new(),
            max_connections,
        }
    }
}

impl SmartTcpConnectionPool for SmartTcpConnPool {
    async fn get_connection(&mut self, user_id: Uuid) -> Option<TcpStream> {
        let conn_string = match self.user_session_map.get(&user_id) {
            Some(conn_id) => {
                self.available_backends.iter().find(|c| c.uuid == *conn_id)
            }
            None => {
                let conn_string = self.available_backends.first()?;
                self.user_session_map.insert(user_id, conn_string.uuid);
                self.user_last_connection_time.insert(user_id, std::time::Instant::now());
                Some(conn_string)
            }
        }?;

        match TcpStream::connect(&conn_string.address()).await {
            Ok(stream) => Some(stream),
            Err(e) => {
                eprintln!("Failed to connect to backend {}: {}", conn_string.uuid, e);
                None
            }
        }
    }
}
