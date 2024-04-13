use hashbrown::HashSet;
use crate::db::{Context, parser};
use crate::db::parser::qql;
use crate::db::validate::ValidationError;
use crate::parser::Ident;

pub(crate) fn validate(context: &Context, query: &parser::Query) -> super::Result<()> {
    let mut args = HashSet::<Ident>::new();
    for arg in &query.args {
        if args.contains(arg) {
            return Err(ValidationError::DuplicateQueryArgument {
                query: query.name.clone(),
                argument: arg.clone(),
            });
        }
        args.insert(arg.clone());
    }

    let principal_model = match &query.statement.selectors.as_slice() {
        [selector] => Some(selector.name.clone()),
        _ => None,
    };

    let mut query_context = QueryContext {
        context,
        query,
        args: &args,
        principal_model,
    };
    query_context.validate_quantifier(&query.statement.quantifier)?;

    Ok(())
}

struct QueryContext<'a> {
    context: &'a Context,
    query: &'a parser::Query,
    args: &'a HashSet<Ident>,

    principal_model: Option<Ident>,
}

impl<'a> QueryContext<'a> {
    fn validate_quantifier(&mut self, quantifier: &qql::Quantifier) -> super::Result<()> {
        match quantifier {
            qql::Quantifier::Expr(expr) => {
                self.validate_expr(expr)?;
                Ok(())
            }
            _ => Ok(())
        }
    }

    fn validate_expr(&mut self, expr: &qql::Expr) -> super::Result<()> {
        match expr {
            qql::Expr::Binary(l, _, r) => {
                self.validate_expr(l.as_ref())?;
                self.validate_expr(r.as_ref())?;
                Ok(())
            }
            qql::Expr::Unary(_, r) => {
                self.validate_expr(r.as_ref())?;
                Ok(())
            }
            qql::Expr::Number(_) => Ok(()),
            qql::Expr::Interp(var) => {
                if !self.args.contains(var) {
                    Err(ValidationError::UnknownQueryVariable {
                        query: self.query.name.clone(),
                        variable: var.clone(),
                    })
                } else {
                    Ok(())
                }
            }
            qql::Expr::Field(model, field) => {
                match (&self.principal_model, model) {
                    (_, Some(model)) | (Some(model), _) => {
                        let models = self.context.models.borrow();
                        match models.get(model) {
                            Some(model) => {
                                if !model.has_field(field) {
                                    Err(ValidationError::QueryUnknownField {
                                        query: self.query.name.clone(),
                                        model: model.name.clone(),
                                        field: field.clone(),
                                    })
                                } else {
                                    Ok(())
                                }
                            }
                            None => Err(ValidationError::QueryUnknownModel {
                                query: self.query.name.clone(),
                                model: model.clone(),
                                field: field.clone(),
                            }),
                        }
                    }
                    (None, None) => Err(ValidationError::AmbiguousQueryField {
                        query: self.query.name.clone(),
                        field: field.clone(),
                    })
                }
            }
        }
    }
}