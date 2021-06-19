use std::sync::Arc;

use strum_macros::{AsRefStr, EnumDiscriminants};

use acd2lr_core::{
    acdsee::AcdSeeError,
    container::{ContainerError, ContainerRewriteError, ContainerWriteError},
    xmp::WriteError,
};

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
    BackupError(Arc<std::io::Error>),
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
            FileState::BackupError(error) => write!(f, "Impossible de sauvegarder: {}", error),
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
