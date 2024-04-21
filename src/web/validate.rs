use std::path::{Path, PathBuf};
use thiserror::Error;
use crate::debug_write;
use crate::parser::ParseError;
use crate::simpl::parser::{SimplFile};
use crate::web::Context;
use crate::web::context::RouteMap;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("route map error: {0}")]
    RouteMapError(RouteMapError),
    #[error("error parsing file {0}: {1}")]
    ParseError(PathBuf, ParseError),
}

#[derive(Debug, Error)]
pub enum RouteMapError {
    #[error("{0}")]
    IoError(std::io::Error),
}

pub fn validate(context: &Context) -> Result<(), ValidationError> {
    validate_route_map(&context.route_map)?;

    Ok(())
}

fn validate_route_map(map: &RouteMap) -> Result<(), ValidationError> {
    for (_name, path) in &map.handlers {
        validate_route_handler(path.as_path())?;
    }
    Ok(())
}

fn validate_route_handler(path: &Path) -> Result<(), ValidationError> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| ValidationError::RouteMapError(RouteMapError::IoError(e)))?;
    let file: SimplFile = contents.parse()
        .map_err(|e| ValidationError::ParseError(path.to_path_buf(), e))?;
    debug_write(&file, &*path.file_name().unwrap().to_string_lossy()).unwrap();

    Ok(())
}