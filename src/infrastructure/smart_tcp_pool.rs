use log::error;
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
    backend_selector: usize,
}

impl SmartTcpConnPool {
    pub fn new(backends_hosts: Vec<ConnString>) -> Self {
        SmartTcpConnPool {
            available_backends: backends_hosts,
            user_session_map: std::collections::HashMap::new(),
            user_last_connection_time: std::collections::HashMap::new(),
            backend_selector: 0,
        }
    }

    fn get_or_assign_backend(&mut self, user_id: Uuid) -> Uuid {
        if let Some(&backend_uuid) = self.user_session_map.get(&user_id) {
            backend_uuid
        } else {
            let backend_uuid = self.next_backend().get_uuid();
            self.user_session_map.insert(user_id, backend_uuid);
            self.user_last_connection_time.insert(user_id, std::time::Instant::now());
            backend_uuid
        }
    }

    fn next_backend(&mut self) -> &ConnString {
        let current = self.backend_selector;
        if current < self.available_backends.len() - 1 {
            self.backend_selector = current + 1;
            &self.available_backends[current]
        } else {
            self.backend_selector = 0;
            &self.available_backends[0]
        }
    }
}

impl SmartTcpConnectionPool for SmartTcpConnPool {
    async fn get_connection(&mut self, user_id: Uuid) -> Option<TcpStream> {
        let backend_uuid = self.get_or_assign_backend(user_id);

        let conn_string = self.available_backends
            .iter()
            .find(|c| c.get_uuid() == backend_uuid)?
            .clone();

        match TcpStream::connect(&conn_string.address()).await {
            Ok(stream) => Some(stream),
            Err(e) => {
                error!("Failed to connect to backend {}: {}", conn_string.get_uuid(), e);
                None
            }
        }
    }
}
