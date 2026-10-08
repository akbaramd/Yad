use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "yad",
    version,
    about = "Project memory and structured records for humans and agents"
)]
pub struct Cli {
    #[arg(long, global = true, help = "Emit machine-readable JSON")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Init(InitArgs),
    Status,
    Sync,
    Remember(RememberArgs),
    Search(SearchArgs),
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
    Adr {
        #[command(subcommand)]
        command: AdrCommand,
    },
    Validate,
    Index {
        #[command(subcommand)]
        command: IndexCommand,
    },
    Infra {
        #[command(subcommand)]
        command: InfraCommand,
    },
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
    Space {
        #[command(subcommand)]
        command: SpaceCommand,
    },
    Doctor,
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
}

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Debug, Args)]
pub struct RememberArgs {
    pub text: String,

    #[arg(long, default_value = "note")]
    pub kind: String,

    #[arg(long, default_value = "general")]
    pub space: String,

    #[arg(long)]
    pub subject: Option<String>,

    #[arg(long, default_value_t = 3)]
    pub importance: u8,

    #[arg(long)]
    pub source: Option<String>,

    #[arg(long = "tag")]
    pub tags: Vec<String>,
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    pub query: String,

    #[arg(long, default_value_t = 10)]
    pub limit: usize,

    #[arg(long)]
    pub space: Option<String>,

    #[arg(long)]
    pub kind: Option<String>,

    #[arg(
        long,
        help = "Include archived, superseded, deprecated, and rejected history"
    )]
    pub include_history: bool,
}

#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    List {
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        space: Option<String>,
        #[arg(long)]
        status: Option<String>,
    },
    Get {
        id: String,
    },
    Archive {
        id: String,
    },
    Supersede {
        id: String,
        text: String,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        subject: Option<String>,
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        importance: Option<u8>,
    },
}

#[derive(Debug, Subcommand)]
pub enum AdrCommand {
    New {
        title: String,
        #[arg(long, default_value = "general")]
        space: String,
        #[arg(long)]
        context: Option<String>,
        #[arg(long)]
        decision: Option<String>,
        #[arg(long)]
        consequences: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    List {
        #[arg(long)]
        space: Option<String>,
        #[arg(long)]
        status: Option<String>,
    },
    Show {
        id: String,
    },
    Validate {
        id: Option<String>,
    },
    Accept {
        id: String,
    },
    Reject {
        id: String,
    },
    Deprecate {
        id: String,
    },
    Supersede {
        id: String,
        #[arg(long)]
        by: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum IndexCommand {
    Rebuild,
    Status,
}

#[derive(Debug, Subcommand)]
pub enum InfraCommand {
    Up,
    Down,
    Status,
}

#[derive(Debug, Subcommand)]
pub enum SchemaCommand {
    List,
    Show { id: String },
    Verify,
}
#[derive(Debug, Subcommand)]
pub enum ModelCommand {
    Status,
    Install {
        #[arg(long)]
        from: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum SpaceCommand {
    Add {
        id: String,
        #[arg(long)]
        name: Option<String>,
    },
    List,
    Tree,
    Show {
        id: String,
    },
}
