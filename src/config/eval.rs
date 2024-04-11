use std::borrow::Cow;
use std::env::VarError;
use anyhow::bail;
use crate::config;
use crate::config::ast::Value;
use crate::config::error::EvaluationError;

pub(super) fn build_context(context: &mut config::Context) {
    context.add_function("Env", EnvFunction).unwrap();
}

impl config::Context {
    pub fn add_function(&mut self, name: impl Into<String>, func: impl Function) -> anyhow::Result<()> {
        let name = name.into();
        if self.functions.contains_key(&name) {
            bail!("Function {name:?} already exists!");
        }
        self.functions.insert(name, Box::new(func));
        Ok(())
    }
}

pub trait Function: 'static {
    fn call(&self, context: &config::Context, args: &[Cow<Value>]) -> Result<Value, EvaluationError>;
}

pub struct EnvFunction;

impl EnvFunction {
    pub const DESCRIPTOR: &'static str = "Env(key, [default-value])";
}

impl Function for EnvFunction {
    fn call(&self, _context: &config::Context, args: &[Cow<Value>]) -> Result<Value, EvaluationError> {
        let (key, default_value) = match args {
            [key] => (key, None),
            [key, default] => (key, Some(default)),
            _ => return Err(EvaluationError::ArgumentIssue {
                function: Self::DESCRIPTOR.to_owned(),
                issue: format!("Expected 1 or 2 arguments, instead found {} arguments", args.len()),
            })
        };

        let key = key.as_string()
            .ok_or_else(|| EvaluationError::ArgumentTypeIssue {
                function: Self::DESCRIPTOR.to_owned(),
                argument: "key".to_owned(),
                type_: "string".to_owned(),
            })?;
        match (std::env::var(key), default_value) {
            (Ok(value), _) => Ok(Value::String(value)),
            (Err(VarError::NotPresent), Some(default)) => Ok(default.as_ref().clone()),
            (Err(e), _) => Err(EvaluationError::EvaluationError {
                function: Self::DESCRIPTOR.to_owned(),
                message: format!("{}", e),
            }),
        }
    }
}
