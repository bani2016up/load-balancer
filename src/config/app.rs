use crate::config::router_map::RouterMap;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub listen_addr: Option<String>,
    pub request_timout_sec: u64,
    pub router_map: Option<RouterMap>,
    is_built: bool,
}

impl AppConfig {
    pub fn new() -> AppConfig {
        AppConfig {
            listen_addr: None,
            request_timout_sec: 30,
            router_map: None,
            is_built: false,
        }
    }

    pub fn router(&mut self, router_map: RouterMap) {
        self.router_map = Some(router_map);
    }

    pub fn listener(&mut self, listen_addr: String) {
        self.listen_addr = Some(listen_addr);
    }

    pub fn request_timeout(&mut self, timeout_ms: u64) {
        self.request_timout_sec = timeout_ms;
    }

    pub fn build(&mut self) {
        if self.listen_addr.is_none() {
            panic!("Listener address is not provided");
        }
        if self.router_map.is_none() {
            panic!("Router map is not provided");
        }
        self.is_built = true;
    }

    pub fn is_built(&self) -> bool {
        self.is_built
    }
}
