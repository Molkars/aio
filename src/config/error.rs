use thiserror::Error;
use crate::parser::Ident;

#[derive(Debug, Error)]
pub enum FromConfigError {
    #[error("missing section {section:?}")]
    MissingSection {
        section: String,
    },
    #[error("expected item at {path:?}")]
    ExpectedItem {
        path: String,
    },
    #[error("evaluation error: {0}")]
    EvaluationError(#[from] EvaluationError),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("error: {0}")]
    Custom(String),
}

impl FromConfigError {
    #[inline]
    pub fn expected_item(path: impl Into<String>) -> Self {
        Self::ExpectedItem {
            path: path.into(),
        }
    }
}

impl<'a> From<&'a str> for FromConfigError {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self::Custom(value.to_owned())
    }
}

impl From<String> for FromConfigError {
    #[inline]
    fn from(value: String) -> Self {
        Self::Custom(value)
    }
}

#[derive(Debug, Error)]
pub enum EvaluationError {
    #[error("expected {type_} from {key:?}")]
    ExpectedValue {
        key: String,
        type_: String,
    },
    #[error("unknown function {name:?}")]
    UnknownFunction {
        name: Ident,
    },
    #[error("{function}: {issue}")]
    ArgumentIssue {
        function: String,
        issue: String,
    },
    #[error("{function}: '{argument}' must be a {type_}")]
    ArgumentTypeIssue {
        function: String,
        argument: String,
        type_: String,
    },
    #[error("{function}: {message}")]
    EvaluationError {
        function: String,
        message: String,
    }
}
