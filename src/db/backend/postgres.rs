use crate::db::backend::Driver;

pub struct PostgresDriver {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) port: u16,
    pub(crate) database: String,
}

impl Driver for PostgresDriver {
    fn connect(&mut self) {

    }
}
