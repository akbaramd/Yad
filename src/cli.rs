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
    Record {
        #[command(subcommand)]
        command: RecordCommand,
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
pub enum RecordCommand {
    /// Create a schema-backed formal record. Schema can be an id, abbreviation, prefix, or alias.
    New {
        schema: String,
        title: String,
        #[arg(long, default_value = "general")]
        space: String,
        /// Set a section as key=value. Prefix value with @ to read it from a UTF-8 file.
        #[arg(long = "section")]
        sections: Vec<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// List formal records across one or all schemas.
    List {
        #[arg(long)]
        schema: Option<String>,
        #[arg(long)]
        space: Option<String>,
        #[arg(long)]
        status: Option<String>,
    },
    Show {
        id: String,
    },
    /// Replace one schema section by key. Prefix value with @ to read it from a UTF-8 file.
    Set {
        id: String,
        section: String,
        value: String,
    },
    Validate {
        id: Option<String>,
        #[arg(long)]
        schema: Option<String>,
    },
    /// Move a record through a lifecycle transition defined by its schema.
    Transition {
        id: String,
        status: String,
    },
    /// Mark a record as superseded by another record of the same schema.
    Supersede {
        id: String,
        #[arg(long)]
        by: String,
    },
    /// Show the installed document types and what each one is for.
    Types,
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
    /// Install any missing built-in schemas and refresh schemas.lock.
    Upgrade {
        #[arg(long)]
        force: bool,
    },
    /// Import a project-specific schema and pin it in schemas.lock.
    Add {
        path: PathBuf,
        #[arg(long)]
        force: bool,
    },
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