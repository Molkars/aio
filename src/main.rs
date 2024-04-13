#![allow(dead_code)]

use std::fmt::Debug;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path};
use anyhow::{anyhow, Context};
use crate::cli::{CLI, Command, DatabaseCommand};
use crate::config::Config;
use crate::db::migrate::{migrate_down, migrate_up};
use crate::db::validate::validate_database;
use crate::db::parser::QQLFile;

mod cli;
mod db;
mod config;
mod parser;

fn main() -> anyhow::Result<()> {
    let cli = <CLI as clap::Parser>::parse();

    match &cli.command {
        Command::Check { path } => check(path),
        Command::Build { path } => build(path),
        Command::Db { command: DatabaseCommand::Query { expression } } => {
            let path = std::env::current_dir()?;
            let config = Config::from_directory(path.to_path_buf())?;
            let db_context = db::Context::from_config(&config)?;

            let path = expression.split('.').collect::<Vec<_>>();
            let (query_expr, query_path) = path.split_last()
                .context("expected path to query: db.example.GetExample()")?;

            // todo: use the actual parser
            let query_name = query_expr.strip_suffix("()").unwrap();

            let mut file_path = db_context.path.clone();
            for link in query_path {
                file_path = file_path.join(link);
            }
            println!("reading file: {}", file_path.display());
            let content: QQLFile = std::fs::read_to_string(file_path)?.parse()?;
            let query = content.queries.get(query_name)
                .ok_or_else(|| anyhow!("no query named {:?} in {}", query_expr, query_path.join(".")))?;

            println!("{query:#?}");

            Ok(())
        }
        Command::Db { command: DatabaseCommand::Migrate { .. } } => {
            Ok(())
        }
    }
}

fn check(path: &Path) -> anyhow::Result<()> {
    let config = Config::from_directory(path.to_path_buf())?;
    let db_context = db::Context::from_config(&config)?;
    validate_database(&db_context)?;

    Ok(())
}

fn build(path: &Path) -> anyhow::Result<()> {
    let config = Config::from_directory(path.to_path_buf())?;

    create_clean_target(path.join("build"))?;

    let db_context = db::Context::from_config(&config)?;
    validate_database(&db_context)?;
    migrate_down(&db_context)?;
    migrate_up(&db_context)?;

    Ok(())
}

fn create_clean_target(path: impl AsRef<Path>) -> anyhow::Result<()> {
    let target = path.as_ref();
    if target.exists() {
        if target.is_file() {
            std::fs::remove_file(&target)?;
        } else if target.is_dir() {
            std::fs::remove_dir_all(&target)?;
        } else {
            unreachable!("don't know how to remove {}", target.display());
        }
    }
    std::fs::create_dir(target)?;
    Ok(())
}

fn debug_write<T: Debug>(v: &T, f: impl AsRef<Path>) -> anyhow::Result<()> {
    use std::io::Write;
    let mut out = BufWriter::new(File::create(f)?);
    write!(&mut out, "{:#?}", v)?;
    Ok(())
}
