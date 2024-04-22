use std::path::PathBuf;
use thiserror::Error;
use crate::parser::{Ident, ParseError};

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("unknown import path: {path}")]
    UnknownImportPath {
        path: Ident,
    },
    #[error("unable to read file {path}: {error}")]
    ReadFileError {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error("unable to parse file {path}: {error}")]
    ParseFileError {
        path: PathBuf,
        error: ParseError
    },
}