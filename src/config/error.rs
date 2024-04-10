use thiserror::Error;

#[derive(Error)]
pub enum FromConfigError {
    #[error("missing section {section:?}")]
    MissingSection {
        section: String,
    },
    #[error("expected section {section:?}")]
    ExpectedSection {
        section: String,
    }
}
