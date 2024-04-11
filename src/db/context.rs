use std::cell::RefCell;
use hashbrown::HashMap;
use crate::config;
use crate::config::error::FromConfigError;
use crate::db::backend::{Driver, PostgresDriver};
use crate::db::model::Model;
use crate::db::types::TypeStore;

pub struct Context {
    pub(crate) type_store: TypeStore,
    pub(crate) models: RefCell<HashMap<String, Model>>,
    pub(crate) driver: Box<dyn Driver>,
}

impl Context {
    pub fn from_config(config: &config::Config) -> Result<Self, FromConfigError> {
        let db_config = config.get_group("database")?;

        let database_type = db_config.get_string("type")?;
        let driver = match database_type.as_str() {
            "postgres" => {
                let _username = db_config.get_string("username")?;
                let _password = db_config.get_string("password")?;
                let _port = db_config.get_int("port")?;
                let _database = db_config.get_string("database")?;
                Box::new(PostgresDriver)
            }
            ty => return Err(FromConfigError::Custom(format!("invalid database type: {:?}", ty))),
        };

        Ok(Context {
            type_store: TypeStore::default(),
            models: RefCell::default(),
            driver,
        })
    }
}