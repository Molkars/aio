use std::cell::RefCell;
use hashbrown::HashMap;
use crate::config::error::FromConfigError;
use crate::db::model::Model;
use crate::db::types::TypeStore;
use crate::pest::project::ProjectConfig;

#[derive(Default)]
pub struct Context {
    pub(crate) type_store: TypeStore,
    pub(crate) models: RefCell<HashMap<String, Model>>,
}

impl Context {
    pub fn from_config(config: &ProjectConfig) -> Result<Self, FromConfigError> {
        let database = config.get("database")
            .ok_or_else(|| FromConfigError::MissingSection { section: "database".to_owned() })?
            .as_group()
            .ok_or_else(|| FromConfigError::ExpectedSection { section: "database".to_owned() })?;

        Ok(Context {
            type_store: TypeStore::default(),
            models: RefCell::default(),
        })
    }
}