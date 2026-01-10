


#[derive(Debug, Clone)]
pub struct RouterMap {
    map: std::collections::HashMap<String, String>,
}

impl RouterMap {
    pub fn new() -> Self {
        RouterMap {
            map: std::collections::HashMap::new(),
        }
    }

    pub fn map_route(&mut self, incoming: &str, backend: &str) {
        self.map.insert(incoming.to_string(), backend.to_string());
    }

    pub fn map_route_range(
        &mut self,
        incoming: &str,
        backend_host_only: &str,
        port_start: usize,
        port_end: usize,
    ) {
        for port in port_start..=port_end {
            self.map_route(format!("{}:{}", incoming, port).as_str(), backend_host_only);
        }
    }
}
