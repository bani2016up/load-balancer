use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ConnString {
    uuid: Uuid,
    host: String,
    port: u16,
}


impl ConnString {
    pub fn new(host: String, port: u16) -> Self {
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

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn get_uuid(&self) -> Uuid {
        self.uuid
    }

}
