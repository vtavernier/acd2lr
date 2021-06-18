use std::{mem::ManuallyDrop, path::PathBuf};

use async_std::{
    channel,
    task::{block_on, JoinHandle},
};
use futures::{select, FutureExt};

mod state;
pub use state::*;

/// A request from the UI to the backend
#[derive(Debug)]
pub enum Request {
    OpenPaths(Vec<PathBuf>),
}

pub type RequestSender = channel::Sender<Request>;
pub type RequestReceiver = channel::Receiver<Request>;

/// A message from the backend to the UI
#[derive(Debug)]
pub enum Message {
    Status(String),
    AddPathsComplete(AddFilesResult),
    FileStateUpdate(Vec<Event>),
}

pub type MessageSender = glib::Sender<Message>;

pub struct Service {
    ui: MessageSender,
}

impl Service {
    pub fn new(ui: MessageSender) -> Self {
        Self { ui }
    }

    async fn run(self, rx: RequestReceiver) {
        info!("started backend service");

        // Initialize service state
        let mut state = State::new();

        loop {
            // Listen for child tasks and channels
            select! {
                result = rx.recv().fuse() => {
                    match result {
                        Ok(request) => match request {
                            Request::OpenPaths(paths) => {
                                self.ui
                                    .send(Message::AddPathsComplete(state.add_files(paths)))
                                    .unwrap();
                            }
                        },
                        Err(_) => {
                            // All senders were dropped
                            break;
                        }
                    }
                },
                _ = state.poll_bg().fuse() => {
                    // No further processing required
                }
            }

            let events = state.drain_events();
            if !events.is_empty() {
                self.ui.send(Message::FileStateUpdate(events)).unwrap();
            }
        }
    }

    pub fn spawn(self) -> ServiceHandle {
        // Create the request channel
        let (tx, rx) = channel::unbounded();
        // Create the thread handle
        let join_handle = async_std::task::spawn(self.run(rx));

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
        block_on(self.tx.send(request)).expect("failed sending request")
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
            async_std::task::block_on(join_handle);
        }
    }
}
