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
    }
}
