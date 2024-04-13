use std::any::type_name;
use std::borrow::Cow;
use std::fmt::{Debug, Display, Formatter};
use std::rc::{Rc, Weak};
use crate::config::Context;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use anyhow::Context as _Context;
use hashbrown::HashMap;
use crate::config::error::EvaluationError;
use crate::parser::{Ident, ParseError, ParsePrimitive, Parser};

#[derive(Clone)]
pub struct ConfigParser<'a> {
    context: Rc<Context>,
    inner: Parser<'a>,
}

impl<'a> ConfigParser<'a> {
    pub fn whitespace(parser: &mut Parser) {
        while parser.take(|c| char::is_ascii_whitespace(&c)) {}
    }

    pub fn new(context: Rc<Context>, contents: &'a str) -> Self {
        Self {
            context,
            inner: Parser::new(contents)
                .with_whitespace(Self::whitespace),
        }
    }

    fn parse_separated_terminated<T>(
        &mut self,
        terminal: impl ParsePrimitive + Copy + Debug,
        separator: impl ParsePrimitive + Copy,
        parse_fn: impl Fn(&mut Self) -> Result<T, ParseError>,
    ) -> Result<Vec<T>, ParseError> {
        let mut out = Vec::new();
        while !self.inner.at_end() && !self.inner.peek(terminal) {
            out.push(parse_fn(self)?);
            if !self.inner.take(separator) {
                break;
            }
        }
        Ok(out)
    }

    pub fn parse_string(&mut self) -> Result<Option<String>, ParseError> {
        self.inner.atomic(|parser| {
            let string_type;
            if parser.take('"') {
                string_type = '"';
            } else if parser.take('\'') {
                string_type = '\'';
            } else {
                return Ok(None);
            }

            let start = parser.location;
            let mut previous_content_index = start.index;
            let mut content = Option::<String>::None;
            while !parser.at_end() && !parser.peek(string_type) {
                if parser.take(|c| c != '\\') {
                    continue;
                }

                // we need to fill the content string with previous normal characters
                let content = content.get_or_insert_with(String::new);
                content.push_str(&parser.source[previous_content_index..parser.location.index]);

                let escape_location = parser.location;
                parser.location.advance('\\');
                let escaped_char = parser.take_char()
                    .ok_or_else(|| ParseError::new("unterminated string, expected escape character after '\\'", parser.location))?;

                match escaped_char {
                    'n' => content.push('\n'),
                    'r' => content.push('\r'),
                    't' => content.push('\t'),
                    '\\' => content.push('\\'),
                    '\'' => content.push('\''),
                    '"' => content.push('"'),
                    'u' => {
                        let Some(hex_digits) = parser.remaining().get(..4) else {
                            return Err(ParseError::new_spanned(
                                "Expected 4 hex-digits after \\u",
                                escape_location,
                                1 + (parser.location.index - escape_location.index),
                            ));
                        };

                        let code = u32::from_str_radix(hex_digits, 16)
                            .map_err(|e| ParseError::new_spanned(
                                format!("Expected 4 hex-digits after \\u, instead found {:?}. ({})", hex_digits, e),
                                escape_location,
                                4 + (parser.location.index - escape_location.index),
                            ))?;
                        let char = char::from_u32(code)
                            .ok_or_else(|| ParseError::new_spanned(
                                format!("Invalid unicode escape, {} is not a valid character", hex_digits),
                                escape_location,
                                4 + (parser.location.index - escape_location.index),
                            ))?;

                        content.push(char);
                    }
                    _ => return Err(ParseError::new_spanned(
                        r#"invalid escape character, expected "#,
                        escape_location,
                        parser.location.index - escape_location.index,
                    )),
                };

                previous_content_index = parser.location.index;
            }
            parser.expect(string_type)?;

            let content = match content {
                Some(mut content) => {
                    content.push_str(&parser.source[previous_content_index..parser.location.index]);
                    content
                }
                None => parser.source[start.index..parser.location.index - 1].to_owned(),
            };

            Ok(Some(content))
        })
    }

    pub fn parse_int<T: TryFrom<i64>>(&mut self) -> Result<Option<T>, ParseError>
        where T::Error: Display
    {
        self.inner.atomic(|parser| {
            let start = parser.location;
            if !parser.take(|c| char::is_ascii_digit(&c)) {
                return Ok(None);
            }

            while parser.take(|c| char::is_ascii_digit(&c) || c == '_') {}
            let end = parser.location.index;
            let lex = parser.source[start.index..end]
                .replace('_', "")
                .parse::<i64>()
                .map_err(|e| ParseError::new_spanned(
                    format!("unable to parse number: {e}"),
                    start,
                    end - start.index,
                ))?;
            let value = T::try_from(lex)
                .map_err(|e| ParseError::new_spanned(
                    format!("unable to parse number: {e}"),
                    start,
                    end - start.index,
                ))?;
            Ok(Some(value))
        })
    }

    pub fn parse_ident(&mut self) -> Option<Ident> {
        self.inner.atomic(|parser| {
            let start = parser.location;
            if !parser.take(|c| char::is_ascii_alphabetic(&c) || c == '_' || c == '-') {
                return Ok(None);
            }

            while parser.take(|c| char::is_ascii_alphanumeric(&c) || c == '_') {}

            let value = parser.source[start.index..parser.location.index].to_owned();
            Ok(Some(Ident {
                value,
                location: start,
                length: parser.location.index - start.index,
            }))
        }).unwrap()
    }

    pub fn parse_path(&mut self) -> Result<Option<PathBuf>, ParseError> {
        self.inner.atomic(|parser| {
            let start = parser.location;
            if !(parser.take("..") || parser.take(".") || parser.take("~") || parser.peek("/")) {
                return Ok(None);
            }

            while parser.take("/") {
                if !parser.take(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
                    break;
                }
                while parser.take(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == ' ' || c == '.') {}
            }

            let contents = &parser.source[start.index..parser.location.index];
            let path = Path::new(contents).to_path_buf();
            Ok(Some(path))
        })
    }
}

pub struct Config {
    inner: Group,
    pub context: Rc<Context>,
    pub root: PathBuf,
}

impl Debug for Config {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("inner", &self.inner)
            .finish()
    }
}

impl Deref for Config {
    type Target = Group;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Config {
    pub fn from_directory(file: PathBuf) -> anyhow::Result<Self> {
        let file = file.canonicalize()
            .with_context(|| format!("unable to canonicalize config file directory: {}", file.display()))?;

        let config_file = file.join("project");
        let content = std::fs::read_to_string(config_file)
            .context("unable to read config file")?;
        let context = Context::new();
        let mut parser = ConfigParser::new(context, content.as_str());

        let mut out = Group::default();
        while !parser.inner.at_end() {
            let name = parser.parse_ident()
                .ok_or_else(|| ParseError::new(
                    "Expected field name",
                    parser.inner.location,
                ))?;
            let value = parser.parse_value()?;
            out.inner.insert(name, value);
        }

        Ok(Config {
            inner: out,
            context: parser.context,
            root: file,
        })
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

impl<'a> ConfigParser<'a> {
    pub fn parse_value(&mut self) -> Result<Value, ParseError> {
        if let Some(group) = self.parse_group()? {
            Ok(Value::Group(group))
        } else if let Some(string) = self.parse_string()? {
            Ok(Value::String(string))
        } else if let Some(int) = self.parse_int()? {
            Ok(Value::Int(int))
        } else if let Some(function) = self.parse_function()? {
            Ok(Value::Function(function))
        } else if let Some(path) = self.parse_path()? {
            Ok(Value::Path(path))
        } else {
            Err(ParseError::new(
                "Expected value: group, string, integer, function, or path",
                self.inner.location,
            ))
        }
    }
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

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Ident,
    pub args: Vec<Value>,
}

impl<'a> ConfigParser<'a> {
    pub fn parse_function(&mut self) -> Result<Option<Function>, ParseError> {
        let Some(name) = self.parse_ident() else {
            return Ok(None);
        };

        self.inner.expect('(')?;
        let args = self.parse_separated_terminated(')', ',', Self::parse_value)?;
        self.inner.expect(')')?;

        Ok(Some(Function {
            name,
            args,
        }))
    }
}

#[derive(Default, Debug, Clone)]
pub struct Group {
    context: Weak<Context>,
    inner: HashMap<Ident, Value>,
}

impl<'a> ConfigParser<'a> {
    pub fn parse_group(&mut self) -> Result<Option<Group>, ParseError> {
        if !self.inner.take('{') {
            return Ok(None);
        }

        let mut out = Group {
            inner: HashMap::default(),
            context: Rc::downgrade(&self.context),
        };

        while !self.inner.at_end() && !self.inner.peek('}') {
            let name = self.parse_ident()
                .ok_or_else(|| ParseError::new(
                    "Expected field name",
                    self.inner.location,
                ))?;
            let value = self.parse_value()?;
            out.inner.insert(name, value);
        }
        self.inner.expect('}')?;

        Ok(Some(out))
    }
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

    pub fn get_int<T>(&self, key: impl AsRef<str>) -> Result<T, EvaluationError>
        where T: TryFrom<i64>,
              T::Error: Display
    {
        let key = key.as_ref();
        let value = self.eval(key)?;
        let value = match value {
            Cow::Owned(Value::Int(value)) => value,
            Cow::Borrowed(Value::Int(value)) => *value,
            _ => return Err(EvaluationError::ExpectedValue {
                key: key.to_owned(),
                type_: "string".to_owned(),
            })
        };

        T::try_from(value)
            .map_err(|e| EvaluationError::EvaluationError {
                function: "<core>".to_owned(),
                message: format!("unable to convert i64 into {}: {}", type_name::<T>(), e),
            })
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
