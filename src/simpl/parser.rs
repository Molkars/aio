use std::fmt::{Debug, Display};
use std::mem::swap;
use std::path::{Path, PathBuf};
use hashbrown::HashSet;
use crate::parser::{Ident, ParseError, ParsePrimitive, Parser, Section};

pub struct SimplParser<'a> {
    inner: Parser<'a>,

    imported_names: HashSet<Ident>,
}

#[derive(Default)]
pub struct SimplFile {
    imports: Vec<Import>,
    statements: Vec<Stmt>,
}

impl<'a> SimplParser<'a> {
    pub fn whitespace(parser: &mut Parser) {
        while parser.take(|c: char| c.is_ascii_whitespace()) {}
    }

    pub fn new(s: &'a str) -> Self {
        Self {
            inner: Parser::new(s)
                .with_whitespace(Self::whitespace),
            imported_names: HashSet::default(),
        }
    }

    pub fn parse_file(&mut self) -> Result<SimplFile, ParseError> {
        let mut out = SimplFile::default();

        while !self.inner.at_end() {
            if let Some(import) = self.parse_import()? {
                out.imports.push(import);
            } else {
                return Err(ParseError::new(
                    "expected import or statement",
                    self.inner.location,
                ));
            }
        }

        Ok(out)
    }

    pub fn parse_number<T: TryFrom<u64>>(&mut self) -> Result<Option<T>, ParseError>
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

    fn parse_until<T>(
        &mut self,
        terminal: impl ParsePrimitive + Copy,
        parse_fn: impl Fn(&mut Self) -> Result<T, ParseError>,
    ) -> Result<Vec<T>, ParseError> {
        let mut out = Vec::new();
        while !self.inner.at_end() && !self.inner.peek(terminal) {
            let item = parse_fn(self)?;
            out.push(item);
        }
        Ok(out)
    }

    #[inline]
    fn atomic<T>(&mut self, f: impl Fn(&mut Self) -> Result<T, ParseError>) -> Result<T, ParseError> {
        let whitespace = self.inner.whitespace.take();
        let value = f(self);
        self.inner.whitespace = whitespace;
        value
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

pub struct Import {
    pub path: Vec<Ident>,
    pub items: HashSet<Ident>,
}

impl<'a> SimplParser<'a> {
    pub fn parse_import(&mut self) -> Result<Option<Import>, ParseError> {
        if !self.take_keyword("use") {
            return Ok(None);
        }

        self.atomic(|parser| {
            let mut name = parser.parse_ident()
                .ok_or_else(|| ParseError::new(
                    "expected identifier",
                    parser.inner.location,
                ))?;

            let mut path = Vec::new();
            let mut items = HashSet::new();
            while parser.inner.take('.') {
                if parser.inner.take('{') {
                    let inner = parser.parse_separated_terminated(
                        '}', ',',
                        |parser| parser.parse_ident()
                            .ok_or_else(|| ParseError::new(
                                "expected imported item name",
                                parser.inner.location,
                            )),
                    )?;
                    parser.inner.expect('}')?;

                    for ident in inner {
                        if items.contains(&ident) {
                            return Err(ParseError::new_spanned(
                                format!("duplicate import item {:?}", ident),
                                ident.location,
                                ident.length,
                            ));
                        }

                        if parser.imported_names.contains(&ident) {
                            return Err(ParseError::new_spanned(
                                format!("item with name already imported! {:?}", ident),
                                ident.location,
                                ident.length,
                            ));
                        }

                        items.insert(ident.clone());
                        parser.imported_names.insert(ident);
                    }

                    break;
                }

                let mut link = parser.parse_ident()
                    .ok_or_else(|| ParseError::new(
                        "expected identifier",
                        parser.inner.location,
                    ))?;
                swap(&mut name, &mut link);
                path.push(link);
            }

            if items.is_empty() {
                items.insert(name);
            } else {
                path.push(name);
            }

            Ok(Some(Import {
                path,
                items,
            }))
        })
    }
}

pub enum Stmt {
    Expr(Expr),
}

impl<'a> SimplParser<'a> {
    pub fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        self.try_parse_stmt()?
            .ok_or_else(|| ParseError::new(
                "Expected statement",
                self.inner.location,
            ))
    }

    pub fn try_parse_stmt(&mut self) -> Result<Option<Stmt>, ParseError> {
        if let Some(expr) = self.try_parse_expression()? {
            Ok(Some(Stmt::Expr(expr)))
        } else {
            Ok(None)
        }
    }
}

pub enum Expr {
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    If(If),
    Ident(Ident),
    Number(u64),
    String(String),
    Decimal(f64),
    Bool(bool),
    Path(PathBuf),
    Group(Box<Expr>),
    Block(Block),
}

pub struct Block {
    pub name: Ident,
    pub inner: Section,
}

pub struct If {
    pub condition: Box<Expr>,
    pub then: Vec<Stmt>,
    pub otherwise: Option<Else>,
}

pub enum Else {
    If(Box<If>),
    Block(Vec<Stmt>),
}

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
    Assign,
}

pub enum UnaryOp {
    Not,
    Negative,
    Call(Vec<Expr>),
    Index(Box<Expr>),
    Access(Ident),
    Unwrap,
}

impl<'a> SimplParser<'a> {
    const PRIMARY_EXPR_ERROR: &'static str = "expected primary expression";

    #[inline]
    pub fn try_parse_expression(&mut self) -> Result<Option<Expr>, ParseError> {
        let expr = self.parse_expression_or();
        match expr {
            Ok(expr) => Ok(Some(expr)),
            Err(err) if err.message == Self::PRIMARY_EXPR_ERROR => Ok(None),
            Err(err) => Err(err),
        }
    }

    #[inline]
    pub fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        let expr = self.try_parse_expression()?;
        expr.ok_or_else(|| ParseError::new(
            "expected qql expression",
            self.inner.location,
        ))
    }

    pub fn parse_expression_or(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_expression_and()?;
        while self.take_keyword_insensitive("or") {
            let rhs = self.parse_expression_and()?;
            expr = Expr::Binary(BinaryOp::Or, Box::new(expr), Box::new(rhs));
        }
        Ok(expr)
    }

    pub fn parse_expression_and(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_expression_eq()?;
        while self.take_keyword_insensitive("and") {
            let rhs = self.parse_expression_eq()?;
            expr = Expr::Binary(BinaryOp::And, Box::new(expr), Box::new(rhs));
        }
        Ok(expr)
    }

    pub fn parse_expression_eq(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_expression_ord()?;
        Ok(loop {
            let op = match () {
                _ if self.inner.take("==") => BinaryOp::Eq,
                _ if self.inner.take("!=") => BinaryOp::Ne,
                _ => break expr,
            };
            let rhs = self.parse_expression_ord()?;
            expr = Expr::Binary(op, Box::new(expr), Box::new(rhs));
        })
    }

    pub fn parse_expression_ord(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_expression_term()?;
        Ok(loop {
            let op = match () {
                _ if self.inner.take(">=") => BinaryOp::Ge,
                _ if self.inner.take(">") => BinaryOp::Gt,
                _ if self.inner.take("<=") => BinaryOp::Le,
                _ if self.inner.take("<") => BinaryOp::Lt,
                _ => break expr,
            };
            let rhs = self.parse_expression_term()?;
            expr = Expr::Binary(op, Box::new(expr), Box::new(rhs));
        })
    }

    pub fn parse_expression_term(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_expression_factor()?;
        Ok(loop {
            let op = match () {
                _ if self.inner.take("+") => BinaryOp::Add,
                _ if self.inner.take("-") => BinaryOp::Sub,
                _ => break expr,
            };
            let rhs = self.parse_expression_factor()?;

            expr = Expr::Binary(op, Box::new(expr), Box::new(rhs));
        })
    }

    pub fn parse_expression_factor(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_expression_unary()?;
        Ok(loop {
            let op = match () {
                _ if self.inner.take("*") => BinaryOp::Mul,
                _ if self.inner.take("/") => BinaryOp::Div,
                _ if self.inner.take("%") => BinaryOp::Rem,
                _ => break expr,
            };
            let rhs = self.parse_expression_unary()?;

            expr = Expr::Binary(op, Box::new(expr), Box::new(rhs));
        })
    }

    pub fn parse_expression_unary(&mut self) -> Result<Expr, ParseError> {
        let op = if self.take_keyword("not") {
            Some(UnaryOp::Not)
        } else if self.inner.take('-') {
            Some(UnaryOp::Negative)
        } else {
            None
        };

        let rhs = self.parse_expression_postfix()?;
        Ok(match op {
            Some(op) => Expr::Unary(op, Box::new(rhs)),
            None => rhs
        })
    }

    pub fn parse_expression_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_expression_primary()?;

        Ok(loop {
            if self.inner.take('.') {
                let property = self.parse_ident()
                    .ok_or_else(|| ParseError::new("expected identifier", self.inner.location))?;
                expr = Expr::Unary(UnaryOp::Access(property), Box::new(expr));
            } else if self.inner.take('(') {
                let args = self.parse_separated_terminated(')', ',', Self::parse_expression)?;
                self.inner.expect(')')?;
                expr = Expr::Unary(UnaryOp::Call(args), Box::new(expr));
            } else if self.inner.take('[') {
                let index = self.parse_expression()?;
                self.inner.expect(']')?;
                expr = Expr::Unary(UnaryOp::Index(Box::new(index)), Box::new(expr));
            } else if self.inner.take('!') {
                expr = Expr::Unary(UnaryOp::Unwrap, Box::new(expr));
            } else {
                break expr;
            }
        })
    }

    pub fn parse_expression_primary(&mut self) -> Result<Expr, ParseError> {
        return if let Some(ident) = self.parse_ident() {
            if ident == "true" {
                Ok(Expr::Bool(true))
            } else if ident == "false" {
                Ok(Expr::Bool(false))
            } else if ident == "if" {
                fn if_body(parser: &mut SimplParser) -> Result<If, ParseError> {
                    let condition = parser.parse_expression()?;
                    parser.inner.expect('{')?;
                    let then = parser.parse_until('}', SimplParser::parse_statement)?;
                    parser.inner.expect('}')?;

                    let otherwise = if parser.take_keyword("else") {
                        let else_ = if parser.take_keyword("if") {
                            let body = if_body(parser)?;
                            Else::If(Box::new(body))
                        } else {
                            parser.inner.expect('{')?;
                            let body = parser.parse_until('}', SimplParser::parse_statement)?;
                            parser.inner.expect('}')?;
                            Else::Block(body)
                        };
                        Some(else_)
                    } else {
                        None
                    };

                    Ok(If {
                        condition: Box::new(condition),
                        then,
                        otherwise,
                    })
                }

                if_body(self).map(Expr::If)
            } else if self.inner.take('{') {
                let start = self.inner.location;
                let inner = self.atomic(|parser| {
                    while parser.inner.take(|c| c != '}') {}
                    let end = parser.inner.location;
                    parser.inner.expect('}')?;

                    let content = parser.inner.source[start.index..end.index].to_owned();
                    Ok(Section {
                        content,
                        location: start,
                        length: end.index - start.index,
                    })
                })?;
                Ok(Expr::Block(Block {
                    name: ident,
                    inner,
                }))
            } else {
                Ok(Expr::Ident(ident))
            }
        } else if let Some(number) = self.parse_number::<u64>()? {
            // todo: decimals lol
            Ok(Expr::Number(number))
        } else if let Some(path) = self.parse_path()? {
            Ok(Expr::Path(path))
        } else if self.inner.take('(') {
            let inner = self.parse_expression()?;
            self.inner.expect(')')?;
            Ok(Expr::Group(Box::new(inner)))
        } else {
            Err(ParseError::new("expected primary expression", self.inner.location))
        };
    }
}