use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ValidationError {
    #[error("model {model} has a duplicated field {field:?}")]
    DuplicateField {
        model: String,
        field: String,
    },
    #[error("{model}.{field} has unknown type {type_name:?}")]
    UnknownFieldType {
        model: String,
        field: String,
        type_name: String,
    },
    #[error("query {query} has a duplicate argument {argument:?}")]
    DuplicateQueryArgument {
        query: String,
        argument: String,
    },
    #[error("query {query} uses unknown variable {variable:?}")]
    UnknownQueryVariable {
        query: String,
        variable: String,
    },
    #[error("query {query} uses an ambiguous field {field}")]
    AmbiguousQueryField {
        query: String,
        field: String,
    },
    #[error("query {query} uses {model}.{field}, however {model:?} is not a model")]
    QueryUnknownModel {
        query: String,
        model: String,
        field: String,
    },
    #[error("query {query} uses {model}.{field}, however {model} has no field {field:?}")]
    QueryUnknownField {
        query: String,
        model: String,
        field: String,
    }
}
