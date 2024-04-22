use std::collections::LinkedList;
use std::fmt::{Debug, Display};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use hashbrown::HashSet;
use crate::parser::{Ident, ParseError, ParsePrimitive, Parser};

pub struct SimplParser<'a> {
    inner: Parser<'a>,
    imported_names: HashSet<Ident>,
}

pub trait KeywordParser {
    fn parse(&self, parser: &mut SimplParser) -> Result<Expr, ParseError>;
}

#[derive(Default, Debug)]
pub struct SimplFile {
    pub imports: Vec<Import>,
    pub statements: Vec<Expr>,
}

impl FromStr for SimplFile {
    type Err = ParseError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SimplParser::new(s).parse_file()
    }
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
            } else if let Some(expr) = self.try_parse_expression()? {
                out.statements.push(expr);
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

    fn parse_terminated<T>(
        &mut self,
        terminator: impl ParsePrimitive + Copy,
        parse_fn: impl Fn(&mut Self) -> Result<T, ParseError>,
    ) -> Result<Vec<T>, ParseError> {
        let mut out = Vec::new();
        while !self.inner.at_end() && !self.inner.peek(terminator) {
            out.push(parse_fn(self)?);
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
    fn atomic<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, ParseError>) -> Result<T, ParseError> {
        self.inner.whitespace();
        let whitespace = self.inner.whitespace.take();
        let value = f(self);
        self.inner.whitespace = whitespace;
        value
    }

    #[inline]
    fn inline<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, ParseError>) -> Result<T, ParseError> {
        let ws = self.inner.whitespace.clone();
        self.inner.whitespace = Some(Rc::new(|parser| {
            while parser.take(|c: char| c != '\n' && c.is_ascii_whitespace()) {}
        }));
        let value = f(self);
        self.inner.whitespace = ws;
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
}

#[derive(Debug)]
pub struct Import {
    pub path: LinkedList<Ident>,
    pub file: Ident,
    pub uses: Option<HashSet<Ident>>,
}

impl<'a> SimplParser<'a> {
    pub fn parse_import(&mut self) -> Result<Option<Import>, ParseError> {
        if !self.inner.take("import") {
            return Ok(None);
        }

        let (path, file) = self.atomic(|parser| {
            let mut path = LinkedList::new();

            loop {
                let ident = parser.parse_ident()
                    .ok_or_else(|| ParseError::new(
                        "Expected path-link in import path",
                        parser.inner.location,
                    ))?;

                if parser.inner.take('/') {
                    path.push_back(ident);
                    continue;
                }

                let mut file_path = String::new();
                if !parser.inner.take('.') {
                    file_path.push_str(&ident);
                } else {
                    file_path.push_str(&ident);
                    file_path.push('.');
                    loop {
                        let extension = parser.parse_ident()
                            .ok_or_else(|| ParseError::new(
                                "expected extension after '.' in import path",
                                parser.inner.location,
                            ))?;
                        file_path.push_str(&extension);

                        if !parser.inner.take('.') {
                            break;
                        }
                    }
                }

                let file = Ident {
                    value: file_path,
                    location: ident.location,
                    length: parser.inner.location.index - ident.location.index,
                };
                return Ok((path, file));
            }
        })?;

        let uses = if self.take_keyword("use") {
            let mut names = HashSet::new();

            let Some(ident) = self.parse_ident() else {
                return Err(ParseError::new(
                    "Expected item name after 'use'",
                    self.inner.location,
                ));
            };
            names.insert(ident);

            while self.inner.take(',') {
                let Some(ident) = self.parse_ident() else {
                    return Err(ParseError::new(
                        "Expected item name in 'use' list",
                        self.inner.location,
                    ));
                };
                names.insert(ident);
            }

            Some(names)
        } else {
            None
        };

        Ok(Some(Import {
            path,
            file,
            uses,
        }))
    }
}

// impl<'a> SimplParser<'a> {
//     pub fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
//         self.try_parse_statement()?
//             .ok_or_else(|| ParseError::new(
//                 "Expected statement",
//                 self.inner.location,
//             ))
//     }
//
//     pub fn try_parse_statement(&mut self) -> Result<Option<Stmt>, ParseError> {
//         if let Some(expr) = self.try_parse_expression()? {
//             Ok(Some(Stmt::Expr(expr)))
//         } else {
//             Ok(None)
//         }
//     }
// }
//
#[derive(Debug)]
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
    Block(Vec<Expr>),
    Html(Vec<Section>),
    Respond(Vec<(Ident, Expr)>),
}

#[derive(Debug)]
pub enum Section {
    Element(HtmlElement),
    Escaped(Expr),
    Unescaped(Expr),
    Text(String),
}

#[derive(Debug)]
pub struct HtmlElement {
    pub name: Ident,
    pub attributes: Vec<(Ident, Option<AttributeValue>)>,
    pub body: Option<Vec<Section>>,
}

#[derive(Debug)]
pub enum AttributeValue {
    String(String),
    Expr(Expr),
}

#[derive(Debug)]
pub struct If {
    pub condition: Box<Expr>,
    pub then: Vec<Expr>,
    pub otherwise: Option<Else>,
}

#[derive(Debug)]
pub enum Else {
    If(Box<If>),
    Block(Vec<Expr>),
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
    Assign,
}

#[derive(Debug)]
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
        let expr = self.parse_expression_assign();
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
            "expected expression",
            self.inner.location,
        ))
    }

    pub fn parse_expression_assign(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_expression_or()?;
        if self.inner.take('=') {
            let rhs = self.parse_expression_or()?;
            Ok(Expr::Binary(BinaryOp::Assign, Box::new(expr), Box::new(rhs)))
        } else {
            Ok(expr)
        }
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
                    .ok_or_else(|| ParseError::new("expected property", self.inner.location))?;
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
                self.parse_expression_if().map(Expr::If)
            } else {
                Ok(Expr::Ident(ident))
            }
        } else if let Some(number) = self.parse_number::<u64>()? {
            // todo: decimals lol
            Ok(Expr::Number(number))
        } else if let Some(path) = self.parse_path()? {
            Ok(Expr::Path(path))
        } else if self.inner.take('(') {
            if self.inner.peek('<') {
                let html = self.parse_html_block()?;
                self.inner.expect(')')?;
                Ok(Expr::Html(html))
            } else {
                let inner = self.parse_expression()?;
                self.inner.expect(')')?;
                Ok(Expr::Group(Box::new(inner)))
            }
        } else if self.inner.take('{') {
            let out = self.parse_terminated('}', Self::parse_expression)?;
            self.inner.expect('}')?;
            Ok(Expr::Block(out))
        } else if let Some(str) = self.parse_string()? {
            Ok(Expr::String(str))
        } else if self.inner.take("<>") {
            let html = self.parse_html_block()?;
            self.inner.expect("</>")?;
            Ok(Expr::Html(html))
        } else if let Some(html) = self.parse_html_element()? {
            Ok(Expr::Html(vec![Section::Element(html)]))
        } else {
            Err(ParseError::new("expected primary expression", self.inner.location))
        };
    }

    fn parse_expression_if(&mut self) -> Result<If, ParseError> {
        let condition = self.parse_expression()?;
        self.inner.expect('{')?;
        let then = self.parse_until('}', SimplParser::parse_expression)?;
        self.inner.expect('}')?;

        let otherwise = if self.take_keyword("else") {
            let else_ = if self.take_keyword("if") {
                let body = self.parse_expression_if()?;
                Else::If(Box::new(body))
            } else {
                self.inner.expect('{')?;
                let body = self.parse_until('}', SimplParser::parse_expression)?;
                self.inner.expect('}')?;
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

    fn parse_html_block(&mut self) -> Result<Vec<Section>, ParseError> {
        let mut out = Vec::new();
        loop {
            if let Some(elt) = self.parse_html_element()? {
                out.push(Section::Element(elt));
            } else if self.inner.take("{!") {
                let expression = self.parse_expression()?;
                out.push(Section::Unescaped(expression));
                self.inner.expect('}')?;
            } else if self.inner.take('{') {
                let expression = self.parse_expression()?;
                out.push(Section::Escaped(expression));
                self.inner.expect('}')?;
            } else {
                break;
            }
        }

        Ok(out)
    }

    fn parse_html_element(&mut self) -> Result<Option<HtmlElement>, ParseError> {
        if !self.inner.take('<') {
            return Ok(None);
        }

        let name = self.parse_ident()
            .ok_or_else(|| ParseError::new("Expected tag name after '<'", self.inner.location))?;

        let mut attributes = Vec::new();
        while let Some(attr_name) = self.parse_ident() {
            let value = if self.inner.take('=') {
                Some(self.parse_html_attribute_value()?)
            } else {
                None
            };
            attributes.push((attr_name, value));
        }

        let body = if self.inner.take("/>") {
            None
        } else {
            self.inner.expect(">")?;

            let mut body = Vec::new();
            while !self.inner.at_end() && !self.inner.peek("</") {
                if let Some(elt) = self.parse_html_element()? {
                    body.push(Section::Element(elt));
                } else if self.inner.take("{!") {
                    let expression = self.parse_expression()?;
                    body.push(Section::Unescaped(expression));
                    self.inner.expect('}')?;
                } else if self.inner.take('{') {
                    let expression = self.parse_expression()?;
                    body.push(Section::Escaped(expression));
                    self.inner.expect('}')?;
                } else {
                    let start = self.inner.location;
                    while self.inner.take(|c: char| c != '<' && c != '{') {}
                    let end = self.inner.location;
                    let content = &self.inner.source[start.index..end.index];
                    if !content.is_empty() {
                        body.push(Section::Text(content.to_owned()));
                    }
                }
            };

            self.inner.expect("</")?;
            self.expect_keyword(&name)?;
            self.inner.expect(">")?;
            Some(body)
        };

        Ok(Some(HtmlElement {
            name,
            attributes,
            body,
        }))
    }

    fn parse_html_attribute_value(&mut self) -> Result<AttributeValue, ParseError> {
        if let Some(value) = self.parse_ident() {
            Ok(AttributeValue::String(value.value))
        } else if self.inner.take('{') {
            let expr = self.parse_expression()?;
            self.inner.expect('}')?;
            Ok(AttributeValue::Expr(expr))
        } else {
            self.atomic(|parser| {
                let string_char = if parser.inner.take('"') {
                    '"'
                } else if parser.inner.take('\'') {
                    '\''
                } else {
                    return Err(ParseError::new(
                        "Expected identifier or string after attribute '='",
                        parser.inner.location,
                    ));
                };

                let mut out = String::new();
                while let Some(c) = parser.inner.peek_char() {
                    if c == string_char {
                        break;
                    }

                    let start = parser.inner.location;
                    parser.inner.location.advance(c);
                    if c != '&' {
                        out.push(c);
                        continue;
                    }

                    if parser.inner.take('#') {
                        if parser.inner.take('x') || parser.inner.take('X') {
                            let content = {
                                let start = parser.inner.location;
                                while parser.inner.take(|c: char| c != ';') {}
                                let end = parser.inner.location;
                                &parser.inner.source[start.index..end.index]
                            };
                            if !parser.inner.take(';') {
                                return Err(ParseError::new(
                                    "expected ';' after html character reference: like &#xFF80;",
                                    start,
                                ));
                            }
                            let end = parser.inner.location;

                            let value = u32::from_str_radix(content, 16)
                                .map_err(|e| ParseError::new_spanned(
                                    format!("expected hexadecimal code point at {:?}: {}", content, e),
                                    start,
                                    end.index - start.index,
                                ))?;
                            let _char = char::from_u32(value)
                                .ok_or_else(|| ParseError::new_spanned(
                                    format!("hexadecimal escape {:?} is not a valid codepoint", content),
                                    start,
                                    end.index - start.index,
                                ))?;
                        } else {
                            let content = {
                                let start = parser.inner.location;
                                while parser.inner.take(|c: char| c != ';') {}
                                &parser.inner.source[start.index..parser.inner.location.index]
                            };
                            if !parser.inner.take(';') {
                                return Err(ParseError::new(
                                    "expected ';' after html character reference: like &#1234;",
                                    start,
                                ));
                            }
                            let end = parser.inner.location;

                            let value = u32::from_str(content)
                                .map_err(|e| ParseError::new_spanned(
                                    format!("expected decimal code point at {:?}: {}", content, e),
                                    start,
                                    end.index - start.index,
                                ))?;
                            let _char = char::from_u32(value)
                                .ok_or_else(|| ParseError::new_spanned(
                                    format!("decimal escape {:?} is not a valid codepoint", content),
                                    start,
                                    end.index - start.index,
                                ))?;
                        }
                    } else {
                        let local_start = parser.inner.location;
                        while parser.inner.take(|c: char| c.is_ascii_alphabetic()) {}
                        let end = parser.inner.location;
                        if end.index - local_start.index == 0 {
                            return Err(ParseError::new(
                                "expected named character reference after the '&' character",
                                start,
                            ));
                        }

                        if !parser.inner.take(';') {
                            return Err(ParseError::new(
                                "expected ';' after html named character reference: like &apos;",
                                start,
                            ));
                        }
                    }

                    out.push_str(&parser.inner.source[start.index..parser.inner.location.index]);
                }
                parser.inner.expect(string_char)?;

                Ok(AttributeValue::String(out))
            })
        }
    }
}
