use std::{
    cmp::Ordering,
    convert::TryFrom,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use async_std::fs::{File, OpenOptions};
use thiserror::Error;

use acd2lr_core::{
    container::{Container, ContainerError},
    xmp::rules,
};

use super::{BackupMode, FileState};

pub const SUPPORTED_EXTS: &[&str] = &["jpeg", "jpg", "tif", "tiff", "xmp", "xpacket"];

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

    pub async fn check_rewrite(&self) -> Self {
        // No state check, since we can always check a rewrite

        let path = self.path.clone();
        let (result, modified) = self.check_rewrite_inner().await;

        Self {
            path,
            last_check: modified,
            state: result,
        }
    }

    fn backup_path(&self) -> PathBuf {
        // Compute target file path
        let mut target_path = self.path().to_path_buf();
        target_path.set_extension(match target_path.extension() {
            Some(ext) => {
                let mut ext = ext.to_owned();
                ext.push(".bak");
                ext
            }
            None => std::ffi::OsString::from("bak"),
        });

        target_path
    }

    async fn backup(&self, backup_mode: BackupMode) -> Result<(), std::io::Error> {
        let target_path = self.backup_path();

        match backup_mode {
            BackupMode::BackupKeep => {
                if target_path.is_file() {
                    // The backup file already exists and we need to keep it
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "le fichier de sauvegarde existe déjà",
                    ));
                }

                async_std::fs::copy(self.path(), &target_path).await?;
            }
            BackupMode::BackupOverwrite => {
                // Don't check the existing backup
                async_std::fs::copy(self.path(), &target_path).await?;
            }
            BackupMode::NoBackups => {}
        }

        Ok(())
    }

    async fn get_apply_state(
        &self,
        file: File,
        modified: SystemTime,
        backup_mode: BackupMode,
    ) -> FileState {
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
                // Backup the file first
                match self.backup(backup_mode).await {
                    Ok(_) => {}
                    Err(e) => {
                        return FileState::BackupError(Arc::new(e));
                    }
                }

                // Open the container
                let mut container = match Container::open(file).await {
                    Ok(container) => container,
                    Err((e, _)) => {
                        return e.into();
                    }
                };

                // Write the data
                match container.write(&bytes[..]).await {
                    Ok(_) => FileState::Complete,
                    Err(e) => e.into(),
                }
            }
            other => other.clone(),
        }
    }

    async fn apply_inner(
        &self,
        backup_mode: BackupMode,
    ) -> (FileState, Option<std::time::SystemTime>) {
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
                        self.get_apply_state(file, modified, backup_mode)
                            .await
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

    pub async fn apply(&self, backup_mode: BackupMode) -> Self {
        let path = self.path.clone();
        let (result, modified) = self.apply_inner(backup_mode).await;

        Self {
            path,
            last_check: modified,
            state: result,
        }
    }

    pub fn from_dir(dir: &Path) -> Vec<Result<Arc<Self>, FileError>> {
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
                                        result.push(Self::try_from(path).map(Arc::new));
                                    }
                                }
                            } else {
                                result.extend(Self::from_dir(&path));
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

#[derive(Debug, Error)]
pub enum FileError {
    #[error("cannot open dir: {}", 0)]
    OpenDir(std::io::Error),
    #[error("cannot open file: {}", 0)]
    OpenFile(std::io::Error),
}
