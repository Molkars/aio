//! todo: better errors

use std::fmt::{Debug, Display};
use std::ops::Deref;
use std::str::FromStr;
use hashbrown::HashMap;
use crate::parser::{Ident, ParseError, ParsePrimitive, Parser};
use crate::pest::db::model::Model;
use crate::pest::db::query::Query;

pub struct QQLParser<'a> {
    inner: Parser<'a>,
}

impl<'a> Deref for QQLParser<'a> {
    type Target = Parser<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> QQLParser<'a> {
    pub fn new(s: &'a str) -> Self {
        QQLParser {
            inner: Parser::new(s)
                .with_whitespace(|parser| {
                    while parser.take(|c| char::is_ascii_whitespace(&c)) {}
                })
        }
    }

    pub fn parse(&mut self) -> Result<QQLFile, ParseError> {
        let mut out = QQLFile::default();
        while !self.inner.at_end() {
            if let Some(model) = self.parse_model()? {
                out.models.insert(model.name.clone(), model);
            } else if let Some(query) = self.parse_query()? {
                out.queries.insert(query.name.clone(), query);
            } else {
                return Err(ParseError::new(
                    "expected model or query",
                    self.location,
                ));
            }
        }
        Ok(out)
    }

    pub fn parse_number<T: TryFrom<u64>>(&mut self) -> Result<Option<T>, ParseError>
        where T::Error: Display
    {
        self.inner.atomic(|parser| {
            if !parser.take(|c| char::is_ascii_digit(&c)) {
                return Ok(None);
            }

            let start = parser.location;
            while parser.take(|c| char::is_ascii_digit(&c) || c == '_') {}
            let end = parser.location.index;
            let lex = parser.source[start.index..end]
                .replace('_', "")
                .parse::<u64>()
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
            if !parser.take(|c| char::is_ascii_alphabetic(&c) || c == '_') {
                return Ok(None);
            }

            while parser.take(|c| char::is_ascii_alphanumeric(&c) || c == '_') {
                // no-op
            }
            let value = parser.source[start.index..parser.location.index].to_string();
            Ok(Some(Ident {
                value,
                location: start,
                length: parser.location.index - start.index,
            }))
        }).unwrap()
    }

    pub fn expect_keyword(&mut self, keyword: &(impl PartialEq<str> + Debug + ?Sized)) -> Result<(), ParseError> {
        let ident = self.parse_ident()
            .ok_or_else(|| ParseError::new(format!("expected keyword {:?}", keyword), self.inner.location))?;

        if keyword.eq(ident.value.as_str()) {
            Ok(())
        } else {
            Err(ParseError::new_spanned(
                format!("expected keyword {:?}", keyword),
                ident.location,
                ident.length,
            ))
        }
    }

    pub fn take_keyword(&mut self, keyword: &(impl PartialEq<str> + Debug + ?Sized)) -> bool {
        let location = self.inner.location;
        match self.expect_keyword(keyword) {
            Ok(_) => true,
            Err(_) => {
                self.inner.location = location;
                false
            }
        }
    }

    pub fn take_keyword_insensitive(&mut self, keyword: &str) -> bool {
        let location = self.inner.location;
        let Some(ident) = self.parse_ident() else {
            return false;
        };
        if !ident.eq_ignore_ascii_case(keyword) {
            self.inner.location = location;
            false
        } else {
            true
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

    fn parse_separated<T>(
        &mut self,
        separator: impl ParsePrimitive + Copy,
        parse_fn: impl Fn(&mut Self) -> Result<T, ParseError>,
    ) -> Result<Vec<T>, ParseError> {
        let item = parse_fn(self)?;
        let mut out = vec![item];
        while self.inner.take(separator) {
            let item = parse_fn(self)?;
            out.push(item);
        }
        Ok(out)
    }

    #[inline]
    fn atomic<T>(&mut self, f: impl Fn(&mut Self) -> Result<T, ParseError>) -> Result<T, ParseError> {
        self.inner.atomic(|parser| {
            let mut inner = QQLParser { inner: parser.clone() };
            let value = f(&mut inner);
            parser.location = inner.location;
            value
        })
    }
}

#[derive(Default, Debug)]
pub struct QQLFile {
    pub models: HashMap<Ident, Model>,
    pub queries: HashMap<Ident, Query>,
}

impl FromStr for QQLFile {
    type Err = ParseError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        QQLParser::new(s).parse()
    }
}

pub mod model {
    use crate::parser::{Ident, ParseError};

    #[derive(Debug)]
    pub struct Model {
        pub name: Ident,
        pub fields: Vec<ModelField>,
    }

    impl<'a> super::QQLParser<'a> {
        pub fn parse_model(&mut self) -> Result<Option<Model>, ParseError> {
            if !self.take_keyword("model") {
                return Ok(None);
            }

            let Some(name) = self.parse_ident() else {
                return Err(ParseError::new("expected model name", self.inner.location));
            };

            self.inner.expect('{')?;
            let fields = self.parse_separated_terminated('}', ',', Self::parse_model_field)?;
            self.inner.expect('}')?;

            Ok(Some(Model {
                name,
                fields,
            }))
        }
    }

    #[derive(Debug)]
    pub struct ModelField {
        pub name: Ident,
        pub type_: FieldType,
    }

    impl<'a> super::QQLParser<'a> {
        pub fn parse_model_field(&mut self) -> Result<ModelField, ParseError> {
            let name = self.parse_ident()
                .ok_or_else(|| ParseError::new("expected field name", self.inner.location))?;
            self.inner.expect(":")?;
            let type_ = self.parse_field_type()?;

            Ok(ModelField { name, type_ })
        }
    }

    #[derive(Debug)]
    pub struct FieldType {
        pub name: Ident,
        pub arg: Option<u64>,
        pub optional: bool,
    }

    impl<'a> super::QQLParser<'a> {
        pub fn parse_field_type(&mut self) -> Result<FieldType, ParseError> {
            let name = self.parse_ident()
                .ok_or_else(|| ParseError::new("expected type name", self.inner.location))?;

            let arg = if self.inner.take('(') {
                let value = self.parse_number::<u64>()?
                    .ok_or_else(|| ParseError::new("expected type argument", self.inner.location))?;
                self.inner.expect(')')?;
                Some(value)
            } else {
                None
            };

            let optional = self.inner.take('?');

            Ok(FieldType {
                name,
                arg,
                optional,
            })
        }
    }
}

pub mod query {
    use crate::parser::{Ident, ParseError};
    use crate::pest::db::qql::Statement;

    #[derive(Debug)]
    pub struct Query {
        pub name: Ident,
        pub args: Vec<Ident>,
        pub statement: Statement,
    }

    impl<'a> super::QQLParser<'a> {
        pub fn parse_query(&mut self) -> Result<Option<Query>, ParseError> {
            if !self.take_keyword("query") {
                return Ok(None);
            }

            let name = self.parse_ident()
                .ok_or_else(|| ParseError::new("Expected query name", self.inner.location))?;
            self.inner.expect('(')?;
            let args = self.parse_separated_terminated(
                ')', ',',
                |parser| parser.parse_ident()
                    .ok_or_else(|| ParseError::new("expected identifier", parser.inner.location)),
            )?;
            self.inner.expect(')')?;
            self.inner.expect('{')?;
            let statement = self.parse_qql_statement()?;
            self.inner.expect('}')?;

            Ok(Some(Query {
                name,
                args,
                statement,
            }))
        }
    }
}

pub mod qql {
    use crate::parser::{Ident, ParseError};
    use crate::pest::db::QQLParser;

    #[derive(Debug)]
    pub struct Statement {
        pub action: Action,
        pub quantifier: Quantifier,
        pub selectors: Vec<Selector>,
        pub where_clause: Option<WhereClause>,
    }

    impl<'a> QQLParser<'a> {
        pub fn parse_qql_statement(&mut self) -> Result<Statement, ParseError> {
            let action = self.parse_qql_action()?;
            let quantifier = self.parse_qql_quantifier()?;
            let selectors = self.parse_separated(',', Self::parse_qql_selector)?;
            let where_clause = self.parse_qql_where_clause()?;

            Ok(Statement {
                action,
                quantifier,
                selectors,
                where_clause,
            })
        }
    }

    #[derive(Debug)]
    pub enum Action {
        Select,
        Update,
        Delete,
    }

    impl<'a> QQLParser<'a> {
        pub fn parse_qql_action(&mut self) -> Result<Action, ParseError> {
            let ident = self.parse_ident()
                .ok_or_else(|| ParseError::new("expected 'select' or 'update' or 'delete'", self.location))?;
            match ident.value.as_str() {
                s if s.eq_ignore_ascii_case("select") => Ok(Action::Select),
                s if s.eq_ignore_ascii_case("update") => Ok(Action::Update),
                s if s.eq_ignore_ascii_case("delete") => Ok(Action::Delete),
                _ => Err(ParseError::new_spanned(
                    "Expected 'select', 'update', or 'delete'",
                    ident.location,
                    ident.length,
                ))
            }
        }
    }

    #[derive(Debug)]
    pub enum Quantifier {
        One,
        All,
        Number(u64),
        Expr(Expr),
    }

    impl<'a> QQLParser<'a> {
        pub fn parse_qql_quantifier(&mut self) -> Result<Quantifier, ParseError> {
            const ERROR_MSG: &str = "expected 'ONE', 'ALL', a number, or an expression";
            if let Some(ident) = self.parse_ident() {
                return match ident.as_str() {
                    s if s.eq_ignore_ascii_case("one") => Ok(Quantifier::One),
                    s if s.eq_ignore_ascii_case("all") => Ok(Quantifier::All),
                    _ => Err(ParseError::new_spanned(ERROR_MSG, ident.location, ident.length))
                };
            }

            if let Some(number) = self.parse_number::<u64>()? {
                return Ok(Quantifier::Number(number));
            }

            if let Some(expr) = self.try_parse_qql_expression()? {
                return Ok(Quantifier::Expr(expr));
            }

            Err(ParseError::new(ERROR_MSG, self.location))
        }
    }

    #[derive(Debug)]
    pub struct Selector {
        pub name: Ident,
        pub fields: Vec<Ident>,
    }

    impl<'a> QQLParser<'a> {
        pub fn parse_qql_selector(&mut self) -> Result<Selector, ParseError> {
            let name = self.parse_ident()
                .ok_or_else(|| ParseError::new("expected model name", self.location))?;

            self.inner.expect('(')?;
            let fields = self.parse_separated_terminated(
                ')', ',',
                |parser| {
                    parser.parse_ident()
                        .ok_or_else(|| ParseError::new("expected field name", parser.location))
                })?;
            self.inner.expect(')')?;

            Ok(Selector {
                name,
                fields,
            })
        }
    }

    #[derive(Debug)]
    pub struct WhereClause {
        pub expr: Expr,
    }

    impl<'a> QQLParser<'a> {
        pub fn parse_qql_where_clause(&mut self) -> Result<Option<WhereClause>, ParseError> {
            if !self.take_keyword_insensitive("where") {
                return Ok(None);
            }

            let expr = self.parse_qql_expression()?;

            Ok(Some(WhereClause {
                expr,
            }))
        }
    }

    #[derive(Debug)]
    pub enum Expr {
        Binary(Box<Expr>, BinaryOp, Box<Expr>),
        Unary(UnaryOp, Box<Expr>),
        Number(u64),
        Interp(Ident),
        Field(Option<Ident>, Ident),
    }

    impl<'a> QQLParser<'a> {
        const PRIMARY_EXPR_ERROR: &'static str = "expected primary expression";

        #[inline]
        pub fn try_parse_qql_expression(&mut self) -> Result<Option<Expr>, ParseError> {
            let expr = self.parse_qql_expression_or();
            match expr {
                Ok(expr) => Ok(Some(expr)),
                Err(err) if err.message == Self::PRIMARY_EXPR_ERROR => Ok(None),
                Err(err) => Err(err),
            }
        }

        #[inline]
        pub fn parse_qql_expression(&mut self) -> Result<Expr, ParseError> {
            let expr = self.try_parse_qql_expression()?;
            expr.ok_or_else(|| ParseError::new(
                "expected qql expression",
                self.location,
            ))
        }

        pub fn parse_qql_expression_or(&mut self) -> Result<Expr, ParseError> {
            let mut expr = self.parse_qql_expression_and()?;
            while self.take_keyword_insensitive("or") {
                let rhs = self.parse_qql_expression_and()?;
                expr = Expr::Binary(Box::new(expr), BinaryOp::Or, Box::new(rhs));
            }
            Ok(expr)
        }

        pub fn parse_qql_expression_and(&mut self) -> Result<Expr, ParseError> {
            let mut expr = self.parse_qql_expression_eq()?;
            while self.take_keyword_insensitive("and") {
                let rhs = self.parse_qql_expression_eq()?;
                expr = Expr::Binary(Box::new(expr), BinaryOp::And, Box::new(rhs));
            }
            Ok(expr)
        }

        pub fn parse_qql_expression_eq(&mut self) -> Result<Expr, ParseError> {
            let mut expr = self.parse_qql_expression_ord()?;
            Ok(loop {
                let op = match () {
                    _ if self.inner.take("==") => BinaryOp::Eq,
                    _ if self.inner.take("!=") => BinaryOp::Ne,
                    _ => break expr,
                };
                let rhs = self.parse_qql_expression_ord()?;
                expr = Expr::Binary(Box::new(expr), op, Box::new(rhs));
            })
        }

        pub fn parse_qql_expression_ord(&mut self) -> Result<Expr, ParseError> {
            let mut expr = self.parse_qql_expression_term()?;
            Ok(loop {
                let op = match () {
                    _ if self.inner.take(">=") => BinaryOp::Ge,
                    _ if self.inner.take(">") => BinaryOp::Gt,
                    _ if self.inner.take("<=") => BinaryOp::Le,
                    _ if self.inner.take("<") => BinaryOp::Lt,
                    _ => break expr,
                };
                let rhs = self.parse_qql_expression_term()?;
                expr = Expr::Binary(Box::new(expr), op, Box::new(rhs));
            })
        }

        pub fn parse_qql_expression_term(&mut self) -> Result<Expr, ParseError> {
            let mut expr = self.parse_qql_expression_factor()?;
            Ok(loop {
                let op = match () {
                    _ if self.inner.take("+") => BinaryOp::Add,
                    _ if self.inner.take("-") => BinaryOp::Sub,
                    _ => break expr,
                };
                let rhs = self.parse_qql_expression_factor()?;

                expr = Expr::Binary(Box::new(expr), op, Box::new(rhs));
            })
        }

        pub fn parse_qql_expression_factor(&mut self) -> Result<Expr, ParseError> {
            let mut expr = self.parse_qql_expression_unary()?;
            Ok(loop {
                let op = match () {
                    _ if self.inner.take("*") => BinaryOp::Mul,
                    _ if self.inner.take("/") => BinaryOp::Div,
                    _ if self.inner.take("%") => BinaryOp::Rem,
                    _ => break expr,
                };
                let rhs = self.parse_qql_expression_unary()?;

                expr = Expr::Binary(Box::new(expr), op, Box::new(rhs));
            })
        }

        pub fn parse_qql_expression_unary(&mut self) -> Result<Expr, ParseError> {
            let op = if self.take_keyword("not") {
                Some(UnaryOp::Not)
            } else if self.inner.take('-') {
                Some(UnaryOp::Negative)
            } else {
                None
            };

            let rhs = self.parse_qql_expression_primary()?;
            Ok(match op {
                Some(op) => Expr::Unary(op, Box::new(rhs)),
                None => rhs
            })
        }

        pub fn parse_qql_expression_primary(&mut self) -> Result<Expr, ParseError> {
            if let Some(number) = self.parse_number::<u64>()? {
                return Ok(Expr::Number(number));
            }

            if let Some(ident) = self.parse_ident() {
                return if self.inner.take('.') {
                    let field_name = self.parse_ident()
                        .ok_or_else(|| ParseError::new("expected field name after model name", self.location))?;
                    Ok(Expr::Field(Some(ident), field_name))
                } else {
                    Ok(Expr::Field(None, ident))
                };
            }

            if let Some(ident) = self.atomic(|parser| {
                if !parser.inner.take('#') {
                    return Ok(None);
                }
                let ident = parser.parse_ident()
                    .ok_or_else(|| ParseError::new("expected argument name", parser.location))?;
                Ok(Some(ident))
            })? {
                return Ok(Expr::Interp(ident));
            }

            Err(ParseError::new("expected primary expression", self.location))
        }
    }

    #[derive(Debug)]
    pub enum BinaryOp {
        Mul,
        Div,
        Rem,
        Add,
        Sub,
        Lt,
        Le,
        Gt,
        Ge,
        Eq,
        Ne,
        And,
        Or,
    }

    #[derive(Debug)]
    pub enum UnaryOp {
        Not,
        Negative,
    }
}
