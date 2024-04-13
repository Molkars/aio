#![allow(dead_code)]

use std::fmt::Debug;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path};
use anyhow::{anyhow, Context};
use crate::cli::{CLI, Command, DatabaseCommand, DatabaseMigrationCommand};
use crate::config::Config;

mod cli;
mod config;
mod db;
mod parser;
mod util;

fn main() -> anyhow::Result<()> {
    let mut cli = <CLI as clap::Parser>::parse();
    cli.path = cli.path.canonicalize()?;

    match &cli.command {
        Command::Check { } => check(&cli.path),
        Command::Build { } => build(&cli.path),
        Command::Db { command: DatabaseCommand::Query { expression } } => {
            let config = Config::from_directory(cli.path.clone())?;
            let db_context = db::Context::from_config(&config)?;

            let path = expression.split('.').collect::<Vec<_>>();
            let (query_expr, query_path) = path.split_last()
                .context("expected path to query: db.example.GetExample()")?;

            let mut file_path = db_context.path.clone();
            for link in query_path {
                file_path = file_path.join(link);
            }
            let file: db::parser::QQLFile = std::fs::read_to_string(file_path)?.parse()?;
            db::validate::validate_file(&db_context, &file)?;

            // todo: use the actual parser
            let query_name = query_expr.strip_suffix("()").unwrap();
            let query = file.queries.get(query_name)
                .ok_or_else(|| anyhow!("no query named {:?} in {}", query_expr, query_path.join(".")))?;

            println!("{query:#?}");

            Ok(())
        }
        Command::Db { command: DatabaseCommand::Migrate { command } } => {
            let config = Config::from_directory(cli.path.clone())?;
            let db_context = db::Context::from_config(&config)?;
            db::validate::validate_database(&db_context)?;
            match command {
                DatabaseMigrationCommand::Up { .. } => {
                    db::migrate::migrate_up(&db_context)?;
                    Ok(())
                }
                DatabaseMigrationCommand::Down { .. } => {
                    db::migrate::migrate_down(&db_context)?;
                    Ok(())
                }
            }
        }
    }
}

fn check(path: &Path) -> anyhow::Result<()> {
    let config = Config::from_directory(path.to_path_buf())?;
    let db_context = db::Context::from_config(&config)?;
    db::validate::validate_database(&db_context)?;

    Ok(())
}

fn build(path: &Path) -> anyhow::Result<()> {
    let config = Config::from_directory(path.to_path_buf())?;

    create_clean_target(path.join("build"))?;

    let db_context = db::Context::from_config(&config)?;
    db::validate::validate_database(&db_context)?;
    // db::cache::cache(&db_context)?;
    db::migrate::migrate_down(&db_context)?;
    db::migrate::migrate_up(&db_context)?;

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
