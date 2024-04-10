use hashbrown::HashSet;
use crate::db::Context;
use crate::db::model::{Model, ModelField};
use crate::db::types::TypeStore;
use crate::db::validate::ValidationError;
use crate::pest::db::model as pest;

pub(super) fn validate(context: &Context, model: &pest::Model) -> crate::db::validate::Result<()> {
    let mut new_model = Model {
        name: model.name.clone(),
        fields: Vec::new(),
    };

    let mut field_names = HashSet::<String>::new();
    for field in model.fields.iter() {
        if field_names.contains(field.name.as_str()) {
            return Err(ValidationError::DuplicateField {
                model: model.name.clone(),
                field: field.name.clone(),
            });
        }
        field_names.insert(field.name.clone());

        let field = validate_field(&context.type_store, &model, field)?;
        new_model.fields.push(field);
    }

    context.models
        .borrow_mut()
        .insert(model.name.clone(), new_model);

    Ok(())
}

fn validate_field(
    type_store: &TypeStore,
    model: &pest::Model,
    field: &pest::ModelField,
) -> crate::db::validate::Result<ModelField> {
    let type_ = type_store.get(&field.type_.name)
        .ok_or_else(|| ValidationError::UnknownFieldType {
            model: model.name.clone(),
            field: field.name.clone(),
            type_name: field.type_.name.clone(),
        })?;

    Ok(ModelField {
        name: model.name.clone(),
        type_,
    })
}
