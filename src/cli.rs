use std::path::PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser, Clone)]
pub struct CLI {
    #[command(subcommand)]
    pub command: Command,

    #[arg(short, long, global=true, default_value="./")]
    pub path: PathBuf,
}

#[derive(Subcommand, Clone)]
pub enum Command {
    Check {
    },
    Build {
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
    Up {},
    Down {}
}