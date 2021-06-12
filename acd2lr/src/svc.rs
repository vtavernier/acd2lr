use std::{mem::ManuallyDrop, path::PathBuf, thread::JoinHandle};

mod state;
pub use state::*;

/// A request from the UI to the backend
#[derive(Debug)]
pub enum Request {
    OpenPaths(Vec<PathBuf>),
}

pub type RequestSender = std::sync::mpsc::Sender<Request>;
pub type RequestReceiver = std::sync::mpsc::Receiver<Request>;

/// A message from the backend to the UI
#[derive(Debug)]
pub enum Message {
    Status(String),
    AddPathsComplete(AddFilesResult),
}

pub type MessageSender = glib::Sender<Message>;

pub struct Service {
    ui: MessageSender,
}

impl Service {
    pub fn new(ui: MessageSender) -> Self {
        Self { ui }
    }

    fn run(self, rx: RequestReceiver) {
        info!("started backend service");

        // Initialize service state
        let mut state = State::new();

        while let Ok(message) = rx.recv() {
            match message {
                Request::OpenPaths(paths) => {
                    self.ui
                        .send(Message::AddPathsComplete(state.add_files(paths)))
                        .unwrap();
                }
            }
        }
    }

    pub fn spawn(self) -> ServiceHandle {
        // Create the request channel
        let (tx, rx) = std::sync::mpsc::channel::<Request>();
        // Create the thread handle
        let join_handle = std::thread::spawn(move || self.run(rx));

        ServiceHandle {
            tx: ManuallyDrop::new(tx),
            join_handle: ManuallyDrop::new(join_handle),
        }
    }
}

pub struct ServiceHandle {
    tx: ManuallyDrop<RequestSender>,
    join_handle: ManuallyDrop<JoinHandle<()>>,
}

impl ServiceHandle {
    pub fn send_request(&self, request: Request) {
        self.tx.send(request).expect("failed sending request")
    }
}

impl Drop for ServiceHandle {
    fn drop(&mut self) {
        unsafe {
            let tx = ManuallyDrop::take(&mut self.tx);
            let join_handle = ManuallyDrop::take(&mut self.join_handle);

            // Drop the channel so the thread will terminate
            drop(tx);

            // Join the thread
            join_handle.join().unwrap();
        }
    }
}
