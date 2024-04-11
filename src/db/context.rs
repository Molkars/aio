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
                let username = db_config.get_string("username")?;
                let password = db_config.get_string("password")?;
                let port = db_config.get_int::<u16>("port")?;
                let database = db_config.get_string("database")?;
                Box::new(PostgresDriver {
                    username,
                    password,
                    port,
                    database,
                })
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