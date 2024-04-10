use std::rc::Rc;
use crate::db::types::Type;

pub struct Model {
    pub name: String,
    pub fields: Vec<ModelField>,
}

impl Model {
    #[inline]
    pub fn has_field(&self, field: impl AsRef<str>) -> bool {
        let field = field.as_ref();
        self.fields.iter()
            .any(|f| f.name == field)
    }
}

pub struct ModelField {
    pub name: String,
    pub type_: Rc<dyn Type>,
}