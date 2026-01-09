use uuid::Uuid;



#[derive(Debug)]
pub enum Status {
    Created,
    Processing,
    Completed,
    Failed
}

#[derive(Debug)]
pub struct Request {
    uuid: Uuid,
    target: String,
    status: Status,
    time_taken: Option<f64>,
    bytes: Option<usize>,
}



impl Request {
    pub fn new(target: String) -> Request {
        Request {
            uuid: Uuid::new_v4(),
            target,
            status: Status::Created,
            time_taken: None,
            bytes: None,
        }
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
