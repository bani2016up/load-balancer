use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Created,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct Request {
    uuid: Uuid,
    target: String,
    status: Status,
    user_id: Uuid,
    time_taken: Option<f64>,
    bytes: Option<usize>,
}

impl Request {
    pub fn new(target: String, user_id: Uuid) -> Request {
        Request {
            uuid: Uuid::new_v4(),
            target: target,
            status: Status::Created,
            user_id: user_id,
            time_taken: None,
            bytes: None,
        }
    }

    pub fn get_target(&self) -> &str {
        &self.target
    }

    pub fn get_user_id(&self) -> Uuid {
        self.user_id
    }

    pub fn get_status(&self) -> &Status {
        &self.status
    }

    pub fn set_status(&mut self, status: Status) {
        self.status = status;
    }

    pub fn set_bytes(&mut self, bytes: usize) {
        self.bytes = Some(bytes);
    }

    pub fn set_time_taken(&mut self, time_taken: f64) {
        self.time_taken = Some(time_taken);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_test() {
        let request: Request = Request::new("example.com".to_string(), Uuid::new_v4());
        assert_eq!(request.get_target(), "example.com");
        assert_eq!(request.get_status(), &Status::Created);
        assert_eq!(request.bytes, None);
        assert_eq!(request.time_taken, None);
    }

    #[test]
    fn change_status_test() {
        let mut request = Request::new("example.com".to_string(), Uuid::new_v4());
        request.set_status(Status::Processing);
        assert_eq!(request.status, Status::Processing);
    }

    #[test]
    fn set_bytes_test() {
        let mut request = Request::new("example.com".to_string(), Uuid::new_v4());
        request.set_bytes(1024);
        assert_eq!(request.bytes, Some(1024));
    }

    #[test]
    fn set_time_taken_test() {
        let mut request = Request::new("example.com".to_string(), Uuid::new_v4());
        request.set_time_taken(0.5);
        assert_eq!(request.time_taken, Some(0.5));
    }
}
