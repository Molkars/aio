mod error;
pub mod model;
pub mod query;

pub use error::ValidationError;
use crate::db;
use crate::db::parser::QQLFile;

pub type Result<T> = core::result::Result<T, ValidationError>;

pub fn validate_file(
    context: &db::Context,
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

pub fn validate_database(db_context: &db::Context) -> anyhow::Result<()> {
    for file in db_context.path.read_dir()? {
        let file = file?;

        let file_type = file.file_type()?;
        if !file_type.is_file() {
            continue;
        }

        let content = std::fs::read_to_string(file.path())?;
        let qql_ast: QQLFile = content.parse()?;
        validate_file(&db_context, &qql_ast)?;
    }
    Ok(())
}

