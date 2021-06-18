use std::{
    cmp::Ordering,
    collections::VecDeque,
    convert::TryFrom,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use acd2lr_core::{
    acdsee::AcdSeeError,
    container::{Container, ContainerError, ContainerRewriteError, ContainerWriteError},
    xmp::{rules, WriteError},
};
use async_std::fs::{File, OpenOptions};
use strum_macros::{AsRefStr, EnumDiscriminants};
use thiserror::Error;

pub const SUPPORTED_EXTS: &[&str] = &["jpeg", "jpg", "tif", "tiff", "xmp", "xpacket"];

#[derive(Debug, Clone, EnumDiscriminants)]
#[strum_discriminants(name(FileStateKind), derive(AsRefStr))]
pub enum FileState {
    Init,
    IoError(Arc<std::io::Error>),
    NoXmpData,
    NoAcdData,
    ContainerError(Arc<ContainerError>),
    XmpRewriteError(Arc<WriteError>),
    InvalidAcdseeData(Arc<AcdSeeError>),
    Ready(Arc<Vec<u8>>),
    RewriteError(Arc<ContainerRewriteError>),
    Complete,
    ApplyError(Arc<ContainerWriteError>),
}

impl std::fmt::Display for FileState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Translate from english
        match self {
            FileState::Init => write!(f, "En attente"),
            FileState::IoError(error) => write!(f, "Erreur E/S: {}", error),
            FileState::NoXmpData => write!(f, "Aucune donnée XMP présente"),
            FileState::NoAcdData => write!(f, "Aucune donnée ACDSee présente"),
            FileState::ContainerError(error) => write!(f, "Erreur de lecture: {}", error),
            FileState::XmpRewriteError(error) => write!(f, "Erreur d'écriture: {}", error),
            FileState::InvalidAcdseeData(error) => write!(f, "Données ACDSee invalides: {}", error),
            FileState::Ready(_) => write!(f, "Prêt pour la réecriture"),
            FileState::RewriteError(error) => {
                write!(f, "Erreur de préparation à la réecriture: {}", error)
            }
            FileState::Complete => write!(f, "Succès"),
            FileState::ApplyError(error) => write!(f, "Erreur de réecriture: {}", error),
        }
    }
}

impl From<Result<FileState, ContainerError>> for FileState {
    fn from(result: Result<FileState, ContainerError>) -> Self {
        match result {
            Ok(result) => result,
            Err(error) => Self::ContainerError(Arc::new(error)),
        }
    }
}

impl From<std::io::Error> for FileState {
    fn from(io: std::io::Error) -> Self {
        Self::IoError(Arc::new(io))
    }
}

impl From<ContainerWriteError> for FileState {
    fn from(e: ContainerWriteError) -> Self {
        Self::ApplyError(Arc::new(e))
    }
}

impl Default for FileState {
    fn default() -> Self {
        Self::Init
    }
}

#[derive(Debug)]
pub struct MetadataFile {
    path: Arc<PathBuf>,
    last_check: Option<std::time::SystemTime>,
    state: FileState,
}

impl MetadataFile {
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn state(&self) -> &FileState {
        &self.state
    }

    async fn get_rewrite_state(
        &self,
        file: File,
    ) -> Result<(FileState, File), (ContainerError, File)> {
        // Open the container
        let mut container = Container::open(file)
            .await
            .map_err(|(e, f)| (e.into(), f))?;

        // Read the xmp data
        let data = match container.read_xmp().await {
            Ok(data) => data,
            Err(e) => {
                return Err((e, container.into_inner()));
            }
        };

        if let Some(xmp) = data {
            // Try to read the acdsee data
            match xmp.acdsee_data() {
                Ok(acd) => {
                    // We have some data, check if it requires rewrites?
                    let mut rules = acd.to_ruleset();
                    if rules.is_empty() {
                        return Ok((FileState::NoAcdData, container.into_inner()));
                    } else {
                        // There are some rules, so try to apply them
                        rules.push(rules::xmp_metadata_date());

                        match xmp.write_events(rules) {
                            Ok(rewritten) => {
                                // We have an XML event stream ready, try to prepare the rewritten content
                                match container.prepare_write(&rewritten).await {
                                    Ok(packet) => {
                                        // Everything works, including the rewrite back to the file
                                        Ok((
                                            FileState::Ready(Arc::new(packet)),
                                            container.into_inner(),
                                        ))
                                    }
                                    Err(error) => {
                                        // Failed the last part
                                        Ok((
                                            FileState::RewriteError(Arc::new(error)),
                                            container.into_inner(),
                                        ))
                                    }
                                }
                            }
                            Err(error) => Ok((
                                FileState::XmpRewriteError(Arc::new(error)),
                                container.into_inner(),
                            )),
                        }
                    }
                }
                Err(error) => Ok((
                    FileState::InvalidAcdseeData(Arc::new(error)),
                    container.into_inner(),
                )),
            }
        } else {
            Ok((FileState::NoXmpData, container.into_inner()))
        }
    }

    async fn check_rewrite_inner(&self) -> (FileState, Option<std::time::SystemTime>) {
        // Open the file
        match File::open(&*self.path).await {
            Ok(file) => match file.metadata().await {
                Ok(metadata) => match metadata.modified() {
                    Ok(modified) => (
                        self.get_rewrite_state(file)
                            .await
                            .map(|(s, _)| s)
                            .map_err(|(e, _)| e)
                            .into(),
                        Some(modified),
                    ),
                    Err(error) => (error.into(), None),
                },
                Err(error) => (error.into(), None),
            },
            Err(error) => (error.into(), None),
        }
    }

    async fn check_rewrite(&self) -> Self {
        // No state check, since we can always check a rewrite

        let path = self.path.clone();
        let (result, modified) = self.check_rewrite_inner().await;

        Self {
            path,
            last_check: modified,
            state: result,
        }
    }

    async fn get_apply_state(
        &self,
        file: File,
        modified: SystemTime,
    ) -> Result<FileState, ContainerError> {
        // Check if we need to check_rewrite first
        let reread_state;
        let (state, file) = if self
            .last_check
            .map(|known_modified| modified > known_modified)
            .unwrap_or(true)
        {
            // The file was modified, thus the known state is stale
            // Try to rewrite it first
            let (state, file) = match self.get_rewrite_state(file).await {
                Ok((res, file)) => (FileState::from(Ok(res)), file),
                Err((err, file)) => (FileState::from(Err(err)), file),
            };

            reread_state = state;
            (&reread_state, file)
        } else {
            (self.state(), file)
        };

        // If the new state is ready, we can proceed
        match state {
            FileState::Ready(bytes) => {
                // Open the container
                let mut container = match Container::open(file).await {
                    Ok(container) => container,
                    Err((e, _)) => {
                        return Ok(e.into());
                    }
                };

                // Write the data
                match container.write(&bytes[..]).await {
                    Ok(_) => Ok(FileState::Complete),
                    Err(e) => Ok(e.into()),
                }
            }
            other => Ok(other.clone()),
        }
    }

    async fn apply_inner(&self) -> (FileState, Option<std::time::SystemTime>) {
        // Open the file r/w
        match OpenOptions::new()
            .read(true)
            .write(true)
            .open(&*self.path)
            .await
        {
            Ok(file) => match file.metadata().await {
                Ok(metadata) => match metadata.modified() {
                    Ok(modified) => (
                        self.get_apply_state(file, modified).await.into(),
                        Some(modified),
                    ),
                    Err(error) => (error.into(), None),
                },
                Err(error) => (error.into(), None),
            },
            Err(error) => (error.into(), None),
        }
    }

    async fn apply(&self) -> Self {
        let path = self.path.clone();
        let (result, modified) = self.apply_inner().await;

        Self {
            path,
            last_check: modified,
            state: result,
        }
    }
}

impl TryFrom<PathBuf> for MetadataFile {
    type Error = FileError;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        Ok(Self {
            path: Arc::new(value),
            last_check: None,
            state: Default::default(),
        })
    }
}

fn files_from_dir(dir: &Path) -> Vec<Result<Arc<MetadataFile>, FileError>> {
    let mut result = Vec::new();

    match std::fs::read_dir(&dir) {
        Ok(read_dir) => {
            for file in read_dir {
                match file {
                    Ok(file) => {
                        let path = file.path();
                        if path.is_file() {
                            if let Some(ext) = path
                                .extension()
                                .and_then(|ext| ext.to_str())
                                .map(|ext| ext.to_ascii_lowercase())
                            {
                                if SUPPORTED_EXTS.binary_search(&ext.as_str()).is_ok() {
                                    result.push(MetadataFile::try_from(path).map(Arc::new));
                                }
                            }
                        } else {
                            result.extend(files_from_dir(&path));
                        }
                    }
                    Err(error) => {
                        result.push(Err(FileError::OpenFile(error)));
                    }
                }
            }
        }
        Err(error) => {
            result.push(Err(FileError::OpenDir(error)));
        }
    }

    result.sort_by(|a, b| match (a, b) {
        (Ok(a), Ok(b)) => a.path.cmp(&b.path),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        _ => Ordering::Equal,
    });

    result
}

#[derive(Debug, Error)]
pub enum FileError {
    #[error("cannot open dir: {}", 0)]
    OpenDir(std::io::Error),
    #[error("cannot open file: {}", 0)]
    OpenFile(std::io::Error),
}

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
    },
}

macro_rules! update_file {
    ($index:ident, $file:ident, $state:ident, $fn:path) => {
        // Find the file slot
        if let Some(state_file) = $state.files.get_mut($index) {
            // Check that the path matches
            if state_file.path.as_ref() != $file.path.as_ref() {
                tracing::warn!(index = %$index,
                               expected = %$file.path.display(),
                               actual = %$file.path.display(),
                               "index mismatch");
                return;
            }

            $fn($file, state_file).await;

            // Notify slot update
            $state.file_events.push(Event::Changed {
                start: $index,
                files: vec![state_file.clone()],
            });
        } else {
            tracing::warn!($index = %$index,
                           file = %$file.path.display(),
                           "no file at index");
        }
    }
}

impl BackgroundTask {
    async fn try_rewrite_inner(file: Arc<MetadataFile>, state_file: &mut Arc<MetadataFile>) {
        // We are working on the right file
        // Try reading the metadata
        let new_file = file.check_rewrite().await;
        tracing::info!(new_state = ?FileStateKind::from(&new_file.state).as_ref(), "checked rewrite");

        // Update the slot
        *state_file = Arc::new(new_file);
    }

    async fn apply_inner(file: Arc<MetadataFile>, state_file: &mut Arc<MetadataFile>) {
        // We are working on the right file
        // Try reading the metadata
        let new_file = file.apply().await;
        tracing::info!(new_state = ?FileStateKind::from(&new_file.state).as_ref(), "applied rewrite");

        // Update the slot
        *state_file = Arc::new(new_file);
    }

    #[tracing::instrument(skip(state))]
    async fn try_rewrite(index: usize, file: Arc<MetadataFile>, state: &mut State) {
        update_file!(index, file, state, Self::try_rewrite_inner)
    }

    #[tracing::instrument(skip(state))]
    async fn apply(index: usize, file: Arc<MetadataFile>, state: &mut State) {
        update_file!(index, file, state, Self::apply_inner)
    }

    async fn run(self, state: &mut State) {
        match self {
            BackgroundTask::TryRewrite { index, file } => {
                Self::try_rewrite(index, file, state).await;
            }
            BackgroundTask::Apply { index, file } => {
                Self::apply(index, file, state).await;
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
                    files_from_dir(&path)
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
    pub fn start_apply(&mut self) -> usize {
        for (index, file) in self.files.iter().enumerate() {
            if matches!(file.state, FileState::Ready(_)) {
                // The file is ready to be rewritten
                tracing::debug!(path = %file.path().display(), "queuing file for apply");
                self.pending_tasks.push_back(BackgroundTask::Apply {
                    index,
                    file: file.clone(),
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
