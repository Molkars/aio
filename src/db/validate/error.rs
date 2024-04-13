use thiserror::Error;
use crate::parser::Ident;

#[derive(Error, Debug, Clone)]
pub enum ValidationError {
    #[error("model {model} has a duplicated field {field:?}")]
    DuplicateField {
        model: Ident,
        field: Ident,
    },
    #[error("{model}.{field} has unknown type {type_name:?}")]
    UnknownFieldType {
        model: Ident,
        field: Ident,
        type_name: Ident,
    },
    #[error("query {query} has a duplicate argument {argument:?}")]
    DuplicateQueryArgument {
        query: Ident,
        argument: Ident,
    },
    #[error("query {query} uses unknown variable {variable:?}")]
    UnknownQueryVariable {
        query: Ident,
        variable: Ident,
    },
    #[error("query {query} uses an ambiguous field {field}")]
    AmbiguousQueryField {
        query: Ident,
        field: Ident,
    },
    #[error("query {query} uses {model}.{field}, however {model:?} is not a model")]
    QueryUnknownModel {
        query: Ident,
        model: Ident,
        field: Ident,
    },
    #[error("query {query} uses {model}.{field}, however {model} has no field {field:?}")]
    QueryUnknownField {
        query: Ident,
        model: Ident,
        field: Ident,
    }
}
