use uuid::Uuid;



#[derive(Debug, Clone)]
pub enum Status {
    Created,
    Processing,
    Completed,
    Failed
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

    pub fn get_user_id(&self) -> Uuid {
        self.user_id
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
