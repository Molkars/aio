use anyhow::Context as _Context;
use postgres::{Client, Config, NoTls};
use crate::db::backend::Driver;
use crate::db::model::{Model, ModelField};
use crate::db::types::{DataType};

pub struct PostgresDriver {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) port: u16,
    pub(crate) database: String,
    pub(crate) client: Client,
}

impl PostgresDriver {
    pub fn new(host: String, username: String, password: String, port: u16, database: String) -> anyhow::Result<Self> {
        let client = Config::new()
            .host(&host)
            .user(&username)
            .password(&password)
            .port(port)
            .dbname(&database)
            .connect(NoTls)?;

        Ok(Self { username, password, port, database, client })
    }
}

impl Driver for PostgresDriver {
    fn migrate_up(&mut self, model: &Model) -> anyhow::Result<()> {
        use std::fmt::Write;

        let mut builder = String::new();
        write!(&mut builder, "create table if not exists {:?} (", &model.name)?;
        for (i, field) in model.fields.iter().enumerate() {
            if i > 0 {
                write!(&mut builder, ",")?;
            }
            writeln!(&mut builder)?;

            let type_def = self.type_definition(field)
                .with_context(|| format!("field {:?} has invalid type {:?}", &field.name, field.repr))?;
            write!(&mut builder, "  {} {}", field.name, type_def)?;
        }
        write!(&mut builder, "\n)")?;

        self.client.query(&builder, &[])?;

        Ok(())
    }

    fn migrate_down(&mut self, model: &Model) -> anyhow::Result<()> {
        use std::fmt::Write;

        let mut builder = String::new();
        writeln!(&mut builder, "drop table if exists {:?};", model.name)?;

        self.client.execute(&builder, &[])?;
        Ok(())
    }
}

impl PostgresDriver {
    fn type_definition(&self, type_: &ModelField) -> anyhow::Result<String> {
        use std::fmt::Write;
        let mut builder = String::new();

        match type_.repr.data_type() {
            DataType::UUID => builder.push_str("UUID DEFAULT gen_random_uuid()"),
            DataType::String => {
                builder.push_str("varchar");
                if let Some(arg) = type_.arg {
                    write!(&mut builder, "({})", arg)?;
                }
            }
            DataType::DateTime => builder.push_str("timestamp"),
        };

        if !type_.optional {
            builder.push_str(" NOT NULL");
        }

        Ok(builder)
    }
}