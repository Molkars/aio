use std::cell::RefCell;
use std::path::PathBuf;
use hashbrown::HashMap;
use crate::config;
use crate::config::error::FromConfigError;
use crate::db::backend::{Driver, PostgresDriver};
use crate::db::ast::Model;
use crate::db::types::TypeStore;
use crate::parser::Ident;

pub struct Context {
    pub(crate) type_store: TypeStore,
    pub(crate) models: RefCell<HashMap<Ident, Model>>,
    pub(crate) driver: RefCell<Box<dyn Driver>>,
    pub(crate) path: PathBuf,
}

impl Context {
    pub fn from_config(config: &config::Config) -> Result<Self, FromConfigError> {
        let db_config = config.get_group("database")?;
        let mut path = db_config.get_path("path")?;
        if path.is_relative() {
            path = config.root.join(&path);
        }
        let path = path.canonicalize()?;

        let database_type = db_config.get_string("type")?;
        let driver: Box<dyn Driver> = match database_type.as_str() {
            "postgres" => {
                let host = db_config.get_string("host")?;
                let username = db_config.get_string("username")?;
                let password = db_config.get_string("password")?;
                let port = db_config.get_int::<u16>("port")?;
                let database = db_config.get_string("database")?;
                let driver = PostgresDriver::new(host, username, password, port, database)
                    .map_err(|e| FromConfigError::Custom(format!("unable to connect to database: {e}")))?;
                Box::new(driver)
            }
            ty => return Err(FromConfigError::Custom(format!("invalid database type: {:?}", ty))),
        };

        Ok(Context {
            type_store: TypeStore::default(),
            models: RefCell::default(),
            driver: RefCell::new(driver),
            path,
        })
    }
}
