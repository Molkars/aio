use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;

mod ident;

pub use ident::Ident;

#[derive(Clone)]
pub struct Parser<'a> {
    pub source: &'a str,
    pub location: Location,
    pub whitespace: Option<Rc<dyn for<'b> Fn(&mut Parser<'b>)>>,
    // comment: Option<Rc<dyn for<'b> Fn(&mut Parser<'b>)>>,
}

impl<'a> Parser<'a> {
    pub fn new(s: &'a str) -> Self {
        Self {
            source: s,
            location: Location {
                index: 0,
                line: 1,
                column: 1,
            },
            whitespace: None,
        }
    }

    pub fn with_whitespace(mut self, whitespace: impl Fn(&mut Parser) + 'static) -> Self {
        self.whitespace = Some(Rc::new(whitespace));
        self
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub location: Location,
    pub length: usize,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "parse error occurred at {}:{} ({})", self.location.line, self.location.column, self.location.index)?;
        writeln!(f, "  {}", self.message)?;
        Ok(())
    }
}

impl Error for ParseError {}

impl ParseError {
    #[inline]
    pub fn new(message: impl Into<String>, location: Location) -> Self {
        Self {
            message: message.into(),
            location,
            length: 1,
        }
    }

    #[inline]
    pub fn new_spanned(message: impl Into<String>, location: Location, length: usize) -> Self {
        Self {
            message: message.into(),
            location,
            length,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Location {
    pub index: usize,
    pub line: usize,
    pub column: usize,
}

impl Location {
    pub fn advance(&mut self, c: char) {
        self.index += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
    }
}

impl<'a> Parser<'a> {
    pub fn remaining(&self) -> &str {
        &self.source[self.location.index..]
    }

    /// parse while being mindful of whitespace
    pub fn atomic<T>(
        &mut self,
        parse_fn: impl FnOnce(&mut Parser<'a>) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        self.whitespace();
        let mut parser = self.clone();
        parser.whitespace = None;
        let value = parse_fn(&mut parser);
        self.location = parser.location;
        value
    }

    pub fn at_end(&mut self) -> bool {
        if let Some(whitespace) = self.whitespace.clone() {
            whitespace(self);
        }
        self.location.index >= self.source.len()
    }

    fn whitespace(&mut self) {
        if let Some(whitespace) = self.whitespace.clone() {
            let mut parser = self.clone();
            parser.whitespace = None;
            whitespace(&mut parser);
            self.location = parser.location;
        }
    }

    #[inline]
    pub fn peek<P: ParsePrimitive>(&mut self, p: P) -> bool {
        self.whitespace();
        p.peek(self)
    }

    #[inline]
    pub fn take<P: ParsePrimitive>(&mut self, p: P) -> bool {
        self.whitespace();
        p.take(self)
    }

    #[inline]
    pub fn expect<P: ParsePrimitive + Copy + Debug>(&mut self, p: P) -> Result<(), ParseError> {
        self.whitespace();
        if p.take(self) {
            Ok(())
        } else {
            Err(ParseError::new_spanned(format!("Expected {:?}", p), self.location, p.len()))
        }
    }
}

pub trait ParsePrimitive {
    fn peek(self, parser: &Parser) -> bool;
    fn take(self, parser: &mut Parser) -> bool;
    fn len(&self) -> usize;
}

impl ParsePrimitive for char {
    #[inline]
    fn peek(self, parser: &Parser) -> bool {
        let c = parser.source[parser.location.index..].chars().next();
        c == Some(self)
    }

    fn take(self, parser: &mut Parser) -> bool {
        if self.peek(parser) {
            parser.location.advance(self);
            true
        } else {
            false
        }
    }

    fn len(&self) -> usize {
        1
    }
}

impl ParsePrimitive for &'_ str {
    #[inline]
    fn peek(self, parser: &Parser) -> bool {
        parser.source[parser.location.index..].starts_with(self)
    }

    fn take(self, parser: &mut Parser) -> bool {
        let cursor = &parser.source[parser.location.index..];
        if self.len() > cursor.len() {
            return false;
        }

        let mut location = parser.location;
        for (a, b) in cursor.chars().zip(self.chars()) {
            if a != b {
                return false;
            }
            location.advance(a);
        }
        parser.location = location;

        true
    }

    fn len(&self) -> usize {
        str::len(self)
    }
}

impl<T: FnOnce(char) -> bool> ParsePrimitive for T {
    fn peek(self, parser: &Parser) -> bool {
        parser.source[parser.location.index..]
            .chars()
            .next()
            .is_some_and(self)
    }

    fn take(self, parser: &mut Parser) -> bool {
        parser.source[parser.location.index..]
            .chars()
            .next()
            .filter(|c| self(*c))
            .inspect(|c| parser.location.advance(*c))
            .is_some()
    }

    fn len(&self) -> usize {
        1
    }
}

