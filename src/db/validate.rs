mod error;
mod model;
mod query;

pub use error::ValidationError;
use crate::db::Context;
use crate::pest::db::QQLFile;

pub type Result<T> = core::result::Result<T, ValidationError>;

pub fn validate(
    context: &Context,
    file: &QQLFile,
) -> Result<()> {
    file.models
        .values()
        .try_for_each(|model| model::validate(context, model))?;

    file.queries
        .values()
        .try_for_each(|query| query::validate(context, query))?;

    Ok(())
}