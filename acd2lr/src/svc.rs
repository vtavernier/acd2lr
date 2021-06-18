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
    ProgressUpdate { current: usize, total: usize },
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
        let mut current_progress_total: Option<usize> = None;

        loop {
            // Listen for child tasks and channels
            select! {
                result = rx.recv().fuse() => {
                    match result {
                        Ok(request) => match request {
                            Request::OpenPaths(paths) => {
                                let (result, bg_tasks) = state.add_files(paths);

                                current_progress_total = Some(bg_tasks);
                                self.ui
                                    .send(Message::AddPathsComplete(result))
                                    .unwrap();
                            }
                        },
                        Err(_) => {
                            // All senders were dropped
                            break;
                        }
                    }
                },
                progress = state.poll_bg().fuse() => {
                    // No further processing required
                    match progress {
                        BackgroundProgress::Left(left) => {
                            let total = current_progress_total.unwrap_or_else(|| {
                                tracing::warn!("no total progress");
                                left + 1
                            });

                            self.ui.send(Message::ProgressUpdate {
                                current: total - left,
                                total,
                            }).unwrap();
                        },
                        BackgroundProgress::Complete => {
                            match current_progress_total.take() {
                                Some(total) => {
                                    self.ui.send(Message::ProgressUpdate {
                                        current: total,
                                        total,
                                    }).unwrap();
                                },
                                None => {
                                    self.ui.send(Message::ProgressUpdate {
                                        current: 1,
                                        total: 1
                                    }).unwrap();
                                }
                            }
                        }
                    }
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
