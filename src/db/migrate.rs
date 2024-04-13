use crate::db::Context;

pub fn migrate_up(context: &Context) -> anyhow::Result<()> {
    let mut driver = context.driver.borrow_mut();
    for model in context.models.borrow().values() {
        driver.migrate_up(model)?;
    }
    Ok(())
}

pub fn migrate_down(context: &Context) -> anyhow::Result<()> {
    let mut driver = context.driver.borrow_mut();
    for model in context.models.borrow().values() {
        driver.migrate_down(model)?;
    }
    Ok(())
}