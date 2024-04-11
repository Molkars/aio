use std::borrow::Cow;
use std::rc::{Rc, Weak};
use crate::config::Context;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use anyhow::{bail};
use pest::iterators::{Pair, Pairs};
use pest::Parser;
use pest_derive::Parser;
use crate::config::error::EvaluationError;

#[derive(Parser)]
#[grammar = "src/config/config.pest"]
pub struct ConfigParser;

#[derive(Debug)]
pub struct Config(Group);

impl Context {
    pub fn parse_config(self: &Rc<Self>, s: &str) -> anyhow::Result<Config> {
        let content = ConfigParser::parse(Rule::file, s)?
            .next().unwrap().into_inner();

        let mut out = Group::default();
        for inner in content {
            let Rule::attribute = inner.as_rule() else {
                break;
            };

            let mut inner = inner.into_inner();
            let name = inner.next().unwrap().as_str().to_owned();
            let value = parse_value(inner.next().unwrap(), self.clone())?;
            out.inner.insert(name, value);
        }

        Ok(Config(out))
    }
}

impl Deref for Config {
    type Target = Group;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Group(Group),
    Function(Function),
    String(String),
    Int(i64),
    Path(PathBuf),
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub args: Vec<Value>,
}

macro_rules! as_impl {
    ($n:ident, $n2:ident: $v:ident => $t:ty) => {
        #[inline]
        pub fn $n(&self) -> Option<&$t> {
            match self {
                Value::$v(value) => Some(value),
                _ => None
            }
        }

        #[inline]
        pub fn $n2(&mut self) -> Option<&mut $t> {
            match self {
                Value::$v(value) => Some(value),
                _ => None
            }
        }
    };
}

impl Value {
    as_impl!(as_group, as_group_mut: Group => Group);
    as_impl!(as_function, as_function_mut: Function => Function);
    as_impl!(as_string, as_string_mut: String => String);
    as_impl!(as_int, as_int_mut: Int => i64);
    as_impl!(as_path, as_path_mut: Path => PathBuf);
}

fn parse_value(value: Pair<Rule>, context: Rc<Context>) -> anyhow::Result<Value> {
    match value.as_rule() {
        Rule::block => {
            let entry = parse_group(value.into_inner(), context)?;
            Ok(Value::Group(entry))
        }
        Rule::expr => parse_value(value.into_inner().next().unwrap(), context),
        Rule::call => {
            let mut inner = value.into_inner();
            let name = inner.next().unwrap().as_str().to_owned();
            let args = inner
                .next().unwrap()
                .into_inner()
                .map(|pair| parse_value(pair, context.clone()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Function(Function { name, args }))
        }
        Rule::string => {
            let string = value.into_inner().next().unwrap().as_str()
                .to_owned()
                .replace("\\\"", "\"")
                .replace("\\\\", "\\")
                .replace("\\n", "\n")
                .replace("\\r", "\r")
                .replace("\\t", "\t");
            // todo: \u1234
            Ok(Value::String(string))
        }
        Rule::number => {
            let value = value.as_str().parse()?;
            Ok(Value::Int(value))
        }
        Rule::path => {
            let value = Path::new(value.as_str()).to_path_buf();
            Ok(Value::Path(value))
        }
        rule => bail!("cannot parse value from {:?}", rule),
    }
}

#[derive(Default, Debug, Clone)]
pub struct Group {
    context: Weak<Context>,
    inner: hashbrown::HashMap<String, Value>,
}

fn parse_group(value: Pairs<Rule>, context: Rc<Context>) -> anyhow::Result<Group> {
    let mut out = Group::default();
    out.context = Rc::downgrade(&context);
    for attribute in value {
        let mut inner = attribute.into_inner();
        let name = inner.next().unwrap().as_str().to_owned();
        let value = parse_value(inner.next().unwrap(), context.clone())?;
        out.inner.insert(name, value);
    }
    Ok(out)
}

impl Group {
    #[inline]
    pub fn get_raw(&self, key: impl AsRef<str>) -> Option<&Value> {
        self.inner.get(key.as_ref())
    }

    #[inline]
    pub fn eval(&self, key: impl AsRef<str>) -> Result<Cow<'_, Value>, EvaluationError> {
        let key = key.as_ref();
        let Some(value) = self.get_raw(key) else {
            return Err(EvaluationError::ExpectedValue {
                key: key.to_owned(),
                type_: "value".to_owned(),
            });
        };
        let context = self.context.upgrade().unwrap();
        match value {
            Value::Function(value) => eval(value, context).map(Cow::Owned),
            value => Ok(Cow::Borrowed(value)),
        }
    }

    pub fn get_group(&self, key: impl AsRef<str>) -> Result<&Group, EvaluationError> {
        let key = key.as_ref();
        match self.get_raw(key) {
            Some(Value::Group(group)) => Ok(group),
            _ => Err(EvaluationError::ExpectedValue {
                key: key.to_owned(),
                type_: "group".to_owned(),
            })
        }
    }

    pub fn get_string(&self, key: impl AsRef<str>) -> Result<String, EvaluationError> {
        let key = key.as_ref();
        let value = self.eval(key)?;
        match value {
            Cow::Owned(Value::String(value)) => Ok(value),
            Cow::Borrowed(Value::String(value)) => Ok(value.clone()),
            _ => Err(EvaluationError::ExpectedValue {
                key: key.to_owned(),
                type_: "string".to_owned(),
            })
        }
    }

    pub fn get_int(&self, key: impl AsRef<str>) -> Result<i64, EvaluationError> {
        let key = key.as_ref();
        let value = self.eval(key)?;
        match value {
            Cow::Owned(Value::Int(value)) => Ok(value),
            Cow::Borrowed(Value::Int(value)) => Ok(*value),
            _ => Err(EvaluationError::ExpectedValue {
                key: key.to_owned(),
                type_: "string".to_owned(),
            })
        }
    }

    pub fn get_path(&self, key: impl AsRef<str>) -> Result<PathBuf, EvaluationError> {
        let key = key.as_ref();
        let value = self.eval(key)?;
        match value {
            Cow::Owned(Value::Path(value)) => Ok(value),
            Cow::Borrowed(Value::Path(value)) => Ok(value.clone()),
            _ => Err(EvaluationError::ExpectedValue {
                key: key.to_owned(),
                type_: "string".to_owned(),
            })
        }
    }
}

fn eval(function: &Function, context: Rc<Context>) -> Result<Value, EvaluationError> {
    loop {
        let Some(handler) = context.functions.get(&function.name) else {
            break Err(EvaluationError::UnknownFunction {
                name: function.name.clone()
            });
        };

        let args = function.args
            .iter()
            .map(|arg| match arg {
                Value::Function(function) => eval(function, context.clone()).map(Cow::Owned),
                value => Ok(Cow::Borrowed(value)),
            })
            .collect::<Result<Vec<_>, _>>()?;

        let value = handler.call(context.as_ref(), args.as_slice())?;

        break match value {
            Value::Function(func) => eval(&func, context.clone()),
            value => Ok(value),
        };
    }
}
