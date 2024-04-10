use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use anyhow::{anyhow, bail};
use pest::iterators::{Pair, Pairs};
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "src/pest/project.pest"]
pub struct ProjectParser;

#[derive(Debug)]
pub struct ProjectConfig(Group);

impl FromStr for ProjectConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut content = ProjectParser::parse(Rule::file, s)?
            .next().unwrap().into_inner();

        let mut out = Group::default();
        for inner in content {
            let Rule::attribute = inner.as_rule() else {
                break;
            };

            let mut inner = inner.into_inner();
            let name = inner.next().unwrap().as_str().to_owned();
            let value = inner.next().unwrap().try_into()?;
            out.inner.insert(name, value);
        }

        Ok(ProjectConfig(out))
    }
}

impl Deref for ProjectConfig {
    type Target = Group;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub enum Value {
    Group(Group),
    String(String),
    Call(String, Vec<Value>),
    Int(u64),
    Path(PathBuf),
}

impl Value {
    #[inline]
    pub fn as_group(&self) -> Option<&Group> {
        match self {
            Self::Group(group) => Some(group),
            _ => None,
        }
    }
}

impl TryFrom<Pair<'_, Rule>> for Value {
    type Error = anyhow::Error;

    fn try_from(value: Pair<Rule>) -> Result<Self, Self::Error> {
        match value.as_rule() {
            Rule::block => {
                let entry = Group::try_from(value.into_inner())?;
                Ok(Value::Group(entry))
            }
            Rule::expr => Value::try_from(value.into_inner().next().unwrap()),
            Rule::call => {
                let mut inner = value.into_inner();
                let name = inner.next().unwrap().as_str().to_owned();
                let args = inner
                    .next().unwrap()
                    .into_inner()
                    .map(Value::try_from)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::Call(name, args))
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
}

#[derive(Default, Debug)]
pub struct Group {
    inner: hashbrown::HashMap<String, Value>,
}

impl TryFrom<Pairs<'_, Rule>> for Group {
    type Error = anyhow::Error;

    fn try_from(value: Pairs<Rule>) -> Result<Self, Self::Error> {
        let mut out = Group::default();
        for attribute in value {
            let mut inner = attribute.into_inner();
            let name = inner.next().unwrap().as_str().to_owned();
            let value = inner.next().unwrap().try_into()?;
            out.inner.insert(name, value);
        }
        Ok(out)
    }
}

impl Group {
    #[inline]
    pub fn get(&self, key: impl AsRef<str>) -> Option<&Value> {
        self.inner.get(key.as_ref())
    }
}