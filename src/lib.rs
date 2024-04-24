use std::path::Path;
use std::{collections::VecDeque, sync::*};

pub enum Command<'a> {
    Cd(Box<&'a Path>),
}

pub enum Response<'a> {
    Success,
    Redirection(Command<'a>),
    Error(String),
    Continue,
    Exit,
}

pub struct Channel<Response> {
    result_queue: Mutex<VecDeque<Response>>,
    command_exec: Condvar,
}

impl<Response> Channel<Response> {
    pub fn new() -> Self {
        Self {
            result_queue: Mutex::new(VecDeque::new()),
            command_exec: Condvar::new(),
        }
    }

    pub fn send(&self, message: Response) {
        self.result_queue.lock().unwrap().push_back(message);
        self.command_exec.notify_one();
    }

    pub fn receive(&self) -> Response {
        let mut queue = self.result_queue.lock().unwrap();
        loop {
            if let Some(message) = queue.pop_front() {
                return message;
            }
            queue = self.command_exec.wait(queue).unwrap();
        }
    }
}
