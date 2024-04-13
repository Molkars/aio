use std::path::PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser, Clone)]
pub struct CLI {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Clone)]
pub enum Command {
    Check {
        path: PathBuf,
    },
    Build {
        path: PathBuf,
    },
    Db {
        #[command(subcommand)]
        command: DatabaseCommand,
    },
}

#[derive(Subcommand, Clone)]
pub enum DatabaseCommand {
    Query {
        expression: String,
    },
    Migrate  {
        #[command(subcommand)]
        command: DatabaseMigrationCommand
    },
}

#[derive(Subcommand, Clone)]
pub enum DatabaseMigrationCommand {
    Up {

    },
    Down {

    }
}