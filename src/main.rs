use std::fmt::Debug;
use std::fs::File;
use std::io::BufWriter;
use std::os::windows::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use crate::cli::{CLI, Command};
use crate::config::project::ProjectConfig;
use crate::pest::db::QQLFile;

mod cli;
mod pest;
mod db;
mod config;

fn main() -> anyhow::Result<()> {
    let cli = <CLI as clap::Parser>::parse();

    match &cli.command {
        Command::Check { path } => check(path),
        Command::Build { path } => build(path),
    }
}

fn check(path: &Path) -> anyhow::Result<()> {
    let project = std::fs::read_to_string(path.join("config"))?;
    let project = ProjectConfig::from_str(project.as_str())?;

    let db = path.join("db");
    let db_context = db::Context::default();
    for file in db.read_dir()? {
        let file = file?;

        let file_type = file.file_type()?;
        if !(file_type.is_file() || file_type.is_symlink_file()) {
            continue;
        }

        let content = std::fs::read_to_string(file.path())?;
        let qql_ast: QQLFile = content.parse()?;
        db::validate::validate(&db_context, &qql_ast)?;
    }

    Ok(())
}

fn build(path: &Path) -> anyhow::Result<()> {
    let project = std::fs::read_to_string(path.join("config"))?;
    let project = ProjectConfig::from_str(project.as_str())?;

    let target = path.join("build");
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

    let db = path.join("db");
    let db_context = db::Context::from_config(&project)?;
    for file in db.read_dir()? {
        let file = file?;

        let file_type = file.file_type()?;
        if !(file_type.is_file() || file_type.is_symlink_file()) {
            continue;
        }

        let content = std::fs::read_to_string(file.path())?;
        let qql_ast: QQLFile = content.parse()?;
        db::validate::validate(&db_context, &qql_ast)?;
    }

    Ok(())
}

fn debug_write<T: Debug>(v: &T, f: impl AsRef<Path>) -> anyhow::Result<()> {
    use std::io::Write;
    let mut out = BufWriter::new(File::create(f)?);
    write!(&mut out, "{:#?}", v)?;
    Ok(())
}
