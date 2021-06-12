use std::{
    convert::TryFrom,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;

pub const SUPPORTED_EXTS: &[&str] = &["jpeg", "jpg", "tif", "tiff", "xmp"];

#[derive(Debug)]
pub struct File {
    path: PathBuf,
}

impl TryFrom<PathBuf> for File {
    type Error = FileError;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        Ok(Self { path: value })
    }
}

fn files_from_dir(dir: &Path) -> Vec<Result<Arc<File>, FileError>> {
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
                                    result.push(File::try_from(path).map(Arc::new));
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
enum Event {
    Added(std::ops::Range<usize>),
    Changed(std::ops::Range<usize>),
}

#[derive(Default, Debug, Clone)]
pub struct State {
    files: Vec<Arc<File>>,
    file_events: Vec<Event>,
}

pub type AddFilesResult = Vec<Result<Arc<File>, FileError>>;

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_files(&mut self, paths: Vec<PathBuf>) -> AddFilesResult {
        let results: Vec<_> = paths
            .into_iter()
            .flat_map(|path| {
                if path.is_dir() {
                    files_from_dir(&path)
                } else {
                    vec![File::try_from(path).map(Arc::new)]
                }
            })
            .collect();

        // Range start for added events
        let start = self.files.len();
        for ok in results.iter().filter(|res| res.is_ok()) {
            self.files.push(ok.as_ref().unwrap().clone());
        }

        let end = self.files.len();
        if end != start {
            self.file_events.push(Event::Added(start..end));
        }

        // Return the result
        results
    }
}
