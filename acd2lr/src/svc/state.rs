use std::{collections::VecDeque, convert::TryFrom, path::PathBuf, sync::Arc};

use super::BackupMode;

mod file_state;
pub use file_state::*;

mod metadata_file;
pub use metadata_file::*;

#[derive(Debug, Clone)]
pub enum Event {
    Added {
        start: usize,
        files: Vec<Arc<MetadataFile>>,
    },
    Changed {
        start: usize,
        files: Vec<Arc<MetadataFile>>,
    },
}

#[derive(Debug)]
enum BackgroundTask {
    TryRewrite {
        index: usize,
        file: Arc<MetadataFile>,
    },
    Apply {
        index: usize,
        file: Arc<MetadataFile>,
        backup_mode: BackupMode,
    },
}

macro_rules! update_file {
    ($index:ident, $file:ident, $state:ident, $fn:path $(, $id:ident)*) => {
        // Find the file slot
        if let Some(state_file) = $state.files.get_mut($index) {
            // Check that the path matches
            if state_file.path() != $file.path() {
                tracing::warn!(index = %$index,
                               expected = %$file.path().display(),
                               actual = %$file.path().display(),
                               "index mismatch");
                return;
            }

            $fn($file $(, $id)*, state_file).await;

            // Notify slot update
            $state.file_events.push(Event::Changed {
                start: $index,
                files: vec![state_file.clone()],
            });
        } else {
            tracing::warn!($index = %$index,
                           file = %$file.path().display(),
                           "no file at index");
        }
    }
}

impl BackgroundTask {
    async fn try_rewrite_inner(file: Arc<MetadataFile>, state_file: &mut Arc<MetadataFile>) {
        // We are working on the right file
        // Try reading the metadata
        let new_file = file.check_rewrite().await;
        tracing::info!(new_state = ?FileStateKind::from(new_file.state()), "checked rewrite");

        // Update the slot
        *state_file = Arc::new(new_file);
    }

    async fn apply_inner(
        file: Arc<MetadataFile>,
        backup_mode: BackupMode,
        state_file: &mut Arc<MetadataFile>,
    ) {
        // We are working on the right file
        // Try reading the metadata
        let new_file = file.apply(backup_mode).await;
        tracing::info!(new_state = ?FileStateKind::from(new_file.state()), "applied rewrite");

        // Update the slot
        *state_file = Arc::new(new_file);
    }

    #[tracing::instrument(skip(state))]
    async fn try_rewrite(index: usize, file: Arc<MetadataFile>, state: &mut State) {
        update_file!(index, file, state, Self::try_rewrite_inner)
    }

    #[tracing::instrument(skip(state))]
    async fn apply(
        index: usize,
        file: Arc<MetadataFile>,
        backup_mode: BackupMode,
        state: &mut State,
    ) {
        update_file!(index, file, state, Self::apply_inner, backup_mode)
    }

    async fn run(self, state: &mut State) {
        match self {
            BackgroundTask::TryRewrite { index, file } => {
                Self::try_rewrite(index, file, state).await;
            }
            BackgroundTask::Apply {
                index,
                file,
                backup_mode,
            } => {
                Self::apply(index, file, backup_mode, state).await;
            }
        }
    }
}

#[derive(Default, Debug)]
pub struct State {
    files: Vec<Arc<MetadataFile>>,
    file_events: Vec<Event>,
    pending_tasks: VecDeque<BackgroundTask>,
}

pub type AddFilesResult = Vec<Result<Arc<MetadataFile>, FileError>>;

#[derive(Debug, Clone, Copy)]
pub enum BackgroundProgress {
    Left(usize),
    Complete,
}

impl From<usize> for BackgroundProgress {
    fn from(events_len: usize) -> Self {
        if events_len == 0 {
            Self::Complete
        } else {
            Self::Left(events_len)
        }
    }
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_files(&mut self, paths: Vec<PathBuf>) -> (AddFilesResult, usize) {
        let results: Vec<_> = paths
            .into_iter()
            .flat_map(|path| {
                if path.is_dir() {
                    MetadataFile::from_dir(&path)
                } else {
                    vec![MetadataFile::try_from(path).map(Arc::new)]
                }
            })
            .collect();

        // Range start for added events
        let start = self.files.len();
        let mut added = Vec::with_capacity(results.len());
        for ok in results.iter() {
            if let Ok(file) = ok {
                // Add the file to the list
                self.files.push(file.clone());
                added.push(file.clone());

                // Add a task to read the file again
                self.pending_tasks.push_back(BackgroundTask::TryRewrite {
                    index: self.files.len() - 1,
                    file: file.clone(),
                });
            }
        }

        if !added.is_empty() {
            self.file_events.push(Event::Added {
                start,
                files: added,
            });
        }

        // Return the result
        (results, self.pending_tasks.len())
    }

    /// # Returns
    ///
    /// The pending number of background tasks.
    pub fn start_apply(&mut self, backup_mode: BackupMode) -> usize {
        for (index, file) in self.files.iter().enumerate() {
            if matches!(file.state(), FileState::Ready(_)) {
                // The file is ready to be rewritten
                tracing::debug!(path = %file.path().display(), "queuing file for apply");
                self.pending_tasks.push_back(BackgroundTask::Apply {
                    index,
                    file: file.clone(),
                    backup_mode,
                });
            }
        }

        self.pending_tasks.len()
    }

    pub async fn poll_bg(&mut self) -> BackgroundProgress {
        if let Some(task) = self.pending_tasks.pop_front() {
            // Something to do
            task.run(self).await;

            BackgroundProgress::from(self.pending_tasks.len())
        } else {
            // Nothing to do
            futures::future::pending::<()>().await;

            BackgroundProgress::Complete
        }
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        self.file_events.drain(..).collect()
    }
}
