use crate::db::model::Model;

pub trait Driver {
    fn migrate_up(&mut self, model: &Model) -> anyhow::Result<()>;
    fn migrate_down(&mut self, model: &Model) -> anyhow::Result<()>;
}