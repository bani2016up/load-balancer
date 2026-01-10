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

    pub fn get_host(&self) -> &str {
        &self.host
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }

    pub fn get_uuid(&self) -> Uuid {
        self.uuid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_test() {
        let result = ConnString::new("ip".to_string(), 80);
        assert_eq!(result.address(), "ip:80");
    }

    #[test]
    fn constructor_from_string_test() {
        let conn_string = "ip:80";
        let result = ConnString::new_from_address(conn_string).expect("Test faild");
        assert_eq!(result.address(), conn_string);
    }
}
