use std::rc::Rc;
use crate::db::types::Type;
use crate::parser::Ident;

#[derive(Debug, Clone)]
pub struct Model {
    pub name: Ident,
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

#[derive(Debug, Clone)]
pub struct ModelField {
    pub name: Ident,
    pub repr: Rc<dyn Type>,
    pub optional: bool,
    pub arg: Option<u64>,
}