use clap::{Args, Parser, Subcommand, ValueEnum};
use domain::ResourceKind;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "notez", version, about = "Notez CLI")]
pub struct Cli {
    #[arg(long, global = true, default_value = ".")]
    pub space: PathBuf,

    #[arg(long, global = true)]
    pub db: Option<PathBuf>,

    #[arg(long, global = true, default_value_t = false)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Scan native Org space
    Scan,

    /// Resolve a query string (ID, ref, locator, title)
    Resolve {
        query: String,
    },

    /// Query resources from the space
    Query(QueryArgs),

    /// Read details of a specific resource ref
    Read {
        r_ref: String,
    },

    /// Space administrative commands
    Space(SpaceSubcommand),
}

#[derive(Args, Debug)]
pub struct QueryArgs {
    #[arg(long)]
    pub kind: Option<CliResourceKind>,

    #[arg(long)]
    pub title_contains: Option<String>,

    #[arg(long)]
    pub exact_ref: Option<String>,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliResourceKind {
    Document,
    Heading,
}

impl From<CliResourceKind> for ResourceKind {
    fn from(k: CliResourceKind) -> Self {
        match k {
            CliResourceKind::Document => ResourceKind::Document,
            CliResourceKind::Heading => ResourceKind::Heading,
        }
    }
}

#[derive(Args, Debug)]
pub struct SpaceSubcommand {
    #[command(subcommand)]
    pub command: SpaceCommands,
}

#[derive(Subcommand, Debug)]
pub enum SpaceCommands {
    /// Rebuild SQLite projection for the space
    Rebuild,
}
