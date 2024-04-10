//! todo: better errors

use std::str::FromStr;
use anyhow::{anyhow, bail};
use hashbrown::HashMap;
use pest::iterators::{Pair, Pairs};
use pest::Parser;
use pest_derive::Parser;
use crate::pest::db::model::Model;
use crate::pest::db::query::Query;

#[derive(Parser)]
#[grammar = "src/pest/db.pest"]
pub struct QQLParser;

#[derive(Default, Debug)]
pub struct QQLFile {
    pub models: HashMap<String, Model>,
    pub queries: HashMap<String, Query>,
}

impl FromStr for QQLFile {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pairs = QQLParser::parse(Rule::file, s)?;

        let mut out = QQLFile::default();
        for pair in pairs {
            if let Rule::EOI = pair.as_rule() {
                break;
            };

            match pair.as_rule() {
                Rule::model => {
                    let model = Model::try_from(pair.into_inner())?;
                    out.models.insert(model.name.clone(), model);
                }
                Rule::query => {
                    let query = Query::try_from(pair.into_inner())?;
                    out.queries.insert(query.name.clone(), query);
                }
                _ => unreachable!()
            };
        }

        Ok(out)
    }
}

trait NextResult {
    type Output;

    fn nextr(&mut self) -> anyhow::Result<Self::Output>;
}

impl<T: Iterator> NextResult for T {
    type Output = T::Item;

    #[inline]
    fn nextr(&mut self) -> anyhow::Result<Self::Output> {
        Iterator::next(self).ok_or_else(|| anyhow!("expected item"))
    }
}

trait PairsPlus {
    fn expect(&mut self, rule: Rule) -> anyhow::Result<Pair<'_, Rule>>;
    fn take_rule(&mut self, rule: Rule) -> Option<Pair<'_, Rule>>;
}

impl PairsPlus for Pairs<'_, Rule> {
    fn expect(&mut self, rule: Rule) -> anyhow::Result<Pair<'_, Rule>> {
        let next = self.nextr()?;
        if next.as_rule() != rule {
            bail!("expected {:?}, instead got {:?}", rule, next.as_rule());
        }
        Ok(next)
    }

    fn take_rule(&mut self, rule: Rule) -> Option<Pair<'_, Rule>> {
        let next = self.peek().filter(|inner| inner.as_rule() == rule)?;
        self.next();
        Some(next)
    }
}

pub mod model {
    use pest::iterators::{Pair, Pairs};
    use crate::pest::db::{NextResult, Rule};

    #[derive(Debug)]
    pub struct Model {
        pub name: String,
        pub fields: Vec<ModelField>,
    }

    impl TryFrom<Pairs<'_, Rule>> for Model {
        type Error = anyhow::Error;

        fn try_from(mut value: Pairs<'_, Rule>) -> Result<Self, Self::Error> {
            let name = value.nextr()?.as_str().to_string();
            let field_list = value.nextr()?.into_inner();
            let fields = field_list
                .map(|field| ModelField::try_from(field.into_inner()))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(Self {
                name,
                fields,
            })
        }
    }

    #[derive(Debug)]
    pub struct ModelField {
        pub name: String,
        pub type_: FieldType,
    }

    impl TryFrom<Pairs<'_, Rule>> for ModelField {
        type Error = anyhow::Error;

        fn try_from(mut value: Pairs<'_, Rule>) -> Result<Self, Self::Error> {
            let name = value.nextr()?.as_str().to_string();
            let type_ = FieldType::try_from(value.nextr()?.into_inner())?;
            Ok(Self { name, type_ })
        }
    }

    #[derive(Debug)]
    pub struct FieldType {
        pub name: String,
        pub arg: Option<u64>,
        pub optional: bool,
    }

    impl TryFrom<Pairs<'_, Rule>> for FieldType {
        type Error = anyhow::Error;

        fn try_from(mut value: Pairs<'_, Rule>) -> Result<Self, Self::Error> {
            let name = value.nextr()?.as_str().to_string();
            let mut arg = None;
            let mut optional = false;
            if let Some(pair) = value.next() {
                match pair.as_rule() {
                    Rule::type_arg => {
                        arg = Some(pair.into_inner().nextr()?.as_str().parse()?);
                        optional = value.next().is_some();
                    }
                    Rule::type_optional => {
                        optional = true;
                    }
                    _ => {}
                };
            }
            Ok(Self {
                name,
                arg,
                optional,
            })
        }
    }
}

pub mod query {
    use pest::iterators::{Pair, Pairs};
    use crate::pest::db::{PairsPlus, Rule};
    use crate::pest::db::qql::Statement;

    #[derive(Debug)]
    pub struct Query {
        pub name: String,
        pub args: Vec<String>,
        pub statement: Statement,
    }

    impl TryFrom<Pairs<'_, Rule>> for Query {
        type Error = anyhow::Error;

        fn try_from(mut value: Pairs<'_, Rule>) -> Result<Self, Self::Error> {
            let name = value.expect(Rule::ident)?.as_str().to_owned();
            let args = value.take_rule(Rule::ident_list)
                .map(Pair::into_inner)
                .map(|idents| {
                    idents
                        .map(|item| item.as_str().to_owned())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(Vec::new);

            let statement = value
                .expect(Rule::qql_statement)?
                .into_inner()
                .try_into()?;

            Ok(Self {
                name,
                args,
                statement,
            })
        }
    }
}

pub mod qql {
    use std::str::FromStr;
    use anyhow::{anyhow, bail};
    use pest::iterators::{Pair, Pairs};
    use crate::debug_write;
    use crate::pest::db::{NextResult, PairsPlus, Rule};

    #[derive(Debug)]
    pub struct Statement {
        pub action: Action,
        pub quantifier: Quantifier,
        pub selectors: Vec<Selector>,
        pub where_clause: Option<WhereClause>,
    }

    impl TryFrom<Pairs<'_, Rule>> for Statement {
        type Error = anyhow::Error;

        fn try_from(mut pairs: Pairs<'_, Rule>) -> Result<Self, Self::Error> {
            let action = pairs.expect(Rule::qql_action)?
                .as_str()
                .parse()?;
            let quantifier = pairs.expect(Rule::qql_quantifier)?
                .try_into()?;
            let selectors = pairs.expect(Rule::qql_selector_list)?
                .into_inner()
                .map(|pair| Selector::try_from(pair.into_inner()))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let where_clause = pairs.take_rule(Rule::qql_where_clause)
                .map(Pair::into_inner)
                .map(WhereClause::try_from)
                .transpose()?;

            Ok(Self {
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

    impl FromStr for Action {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(if s.eq_ignore_ascii_case("select") {
                Self::Select
            } else if s.eq_ignore_ascii_case("update") {
                Self::Update
            } else if s.eq_ignore_ascii_case("delete") {
                Self::Delete
            } else {
                bail!("expected 'select', 'update', or 'delete', got {:?}", s)
            })
        }
    }

    #[derive(Debug)]
    pub enum Quantifier {
        One,
        All,
        Expr(Expr),
    }

    impl TryFrom<Pair<'_, Rule>> for Quantifier {
        type Error = anyhow::Error;

        fn try_from(mut value: Pair<'_, Rule>) -> Result<Self, Self::Error> {
            Ok(if let Rule::qql_expr = value.as_rule() {
                let pairs = value.into_inner();
                Quantifier::Expr(Expr::try_from(pairs)?)
            } else {
                match value.as_str() {
                    s if s.eq_ignore_ascii_case("one") => Quantifier::One,
                    s if s.eq_ignore_ascii_case("all") => Quantifier::All,
                    _ => bail!("invalid quantifier, expected 'one', 'all', or an expression. Instead got {:?}", value.as_str()),
                }
            })
        }
    }

    #[derive(Debug)]
    pub struct Selector {
        pub name: String,
        pub fields: Vec<String>,
    }

    impl TryFrom<Pairs<'_, Rule>> for Selector {
        type Error = anyhow::Error;

        fn try_from(mut pairs: Pairs<'_, Rule>) -> Result<Self, Self::Error> {
            let name = pairs.expect(Rule::ident)?.as_str().to_owned();
            let fields = pairs.expect(Rule::ident_list)?
                .into_inner()
                .map(|ident| ident.as_str().to_owned())
                .collect::<Vec<_>>();
            Ok(Self { name, fields })
        }
    }

    #[derive(Debug)]
    pub struct WhereClause {
        pub expr: Expr,
    }

    impl TryFrom<Pairs<'_, Rule>> for WhereClause {
        type Error = anyhow::Error;

        fn try_from(mut pairs: Pairs<'_, Rule>) -> Result<Self, Self::Error> {
            let expr = pairs.expect(Rule::qql_expr)?
                .into_inner()
                .try_into()?;
            Ok(Self {
                expr
            })
        }
    }

    #[derive(Debug)]
    pub enum Expr {
        Binary(Box<Expr>, BinaryOp, Box<Expr>),
        Unary(UnaryOp, Box<Expr>),
        Number(u64),
        Field(Option<String>, String),
        Ident(String),
    }

    impl TryFrom<Pairs<'_, Rule>> for Expr {
        type Error = anyhow::Error;

        fn try_from(mut pairs: Pairs<'_, Rule>) -> Result<Self, Self::Error> {
            fn inner(pair: Pair<'_, Rule>) -> anyhow::Result<Expr> {
                Ok(match pair.as_rule() {
                    Rule::qql_expr_or | Rule::qql_expr_and | Rule::qql_expr_eq | Rule::qql_expr_ord | Rule::qql_expr_term | Rule::qql_expr_factor => {
                        let mut pairs = pair.into_inner();
                        let mut expr = inner(pairs.nextr()?)?;
                        while pairs.peek().is_some() {
                            let op = pairs.nextr()?.as_str().parse()?;
                            let right = inner(pairs.nextr()?)?;
                            expr = Expr::Binary(Box::new(expr), op, Box::new(right));
                        }
                        expr
                    }
                    Rule::qql_expr_unary => {
                        let mut pairs = pair.into_inner();
                        let op = pairs.take_rule(Rule::qql_expr_unary_op);
                        if let Some(op) = op {
                            let op = op.as_str().parse()?;
                            let right = inner(pairs.nextr()?)?;
                            Expr::Unary(op, Box::new(right))
                        } else {
                            inner(pairs.nextr()?)?
                        }
                    }
                    Rule::qql_expr_number => {
                        Expr::Number(pair.as_str().parse()?)
                    }
                    Rule::qql_expr_field => {
                        let mut pairs = pair.into_inner();
                        let first = pairs.nextr()?.as_str().to_owned();
                        let second = pairs.next().map(|p| p.as_str().to_owned());
                        match second {
                            Some(field) => Expr::Field(Some(first), field),
                            None => Expr::Field(None, first),
                        }
                    }
                    Rule::qql_expr_ident => {
                        Expr::Ident(pair.into_inner().nextr()?.as_str().to_owned())
                    }
                    rule => unreachable!("{:?}", rule),
                })
            }

            let mut out = None;
            while pairs.peek().is_some() {
                let pair = pairs.next().unwrap();
                let right = inner(pair)?;
                out = Some(match out {
                    Some(left) => Expr::Binary(Box::new(left), BinaryOp::Or, Box::new(right)),
                    None => right,
                });
            }
            out.ok_or_else(|| anyhow!("expected expression"))
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

    impl FromStr for BinaryOp {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(match s {
                "*" => Self::Mul,
                "/" => Self::Div,
                "%" => Self::Rem,
                "+" => Self::And,
                "-" => Self::Sub,
                "<" => Self::Lt,
                "<=" => Self::Le,
                ">" => Self::Gt,
                ">=" => Self::Ge,
                "==" => Self::Eq,
                "!=" => Self::Ne,
                s if s.eq_ignore_ascii_case("and") => Self::And,
                s if s.eq_ignore_ascii_case("or") => Self::Or,
                s => bail!("invalid binary operator, expected '+', '-', '*', '/', '%', '<', '<=', '>', '>=', 'and', or 'or'. Instead found {:?}", s)
            })
        }
    }

    #[derive(Debug)]
    pub enum UnaryOp {
        Not,
        Negative,
    }

    impl FromStr for UnaryOp {
        type Err = anyhow::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(match s {
                s if s.eq_ignore_ascii_case("not") => Self::Not,
                "-" => Self::Negative,
                _ => bail!("invalid unary operator, expected 'not' or '-'")
            })
        }
    }
}
