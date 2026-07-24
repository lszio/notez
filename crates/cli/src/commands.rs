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
    Resolve { query: String },

    /// Query resources from the space
    Query(QueryArgs),

    /// Read details of a specific resource ref
    Read { r_ref: String },

    /// Inspect resource or rule traces
    Inspect {
        r_ref: String,
        #[arg(long, default_value_t = false)]
        rules: bool,
    },

    /// Agenda view of scheduled/deadline items (Legacy alias for `task agenda`)
    Agenda,

    /// Link commands (list, resolve, diagnose)
    Link(LinkSubcommand),

    /// Task mutation operations
    Task(TaskSubcommand),
    /// Source management commands (federation)
    Source(SourceSubcommand),
    /// Attachment operations and extraction jobs
    Attachment(AttachmentSubcommand),
    /// Community management commands
    Community(CommunitySubcommand),

    /// Derive recipe artifacts (summary, llms.txt, context-pack, skill-ir)
    Derive(DeriveArgs),

    /// Agent Skill export commands
    Skill(SkillSubcommand),
    // Job is now merged into Task

    /// Derived artifact management
    Artifact(ArtifactSubcommand),

    /// MCP stdio server commands
    Mcp(McpSubcommand),

    /// Space administrative commands
    Space(SpaceSubcommand),

    /// Synchronization commands (folder push/pull/conflicts)
    Sync(SyncSubcommand),
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
    Attachment,
    Block,
}
impl From<CliResourceKind> for ResourceKind {
    fn from(k: CliResourceKind) -> Self {
        match k {
            CliResourceKind::Document => ResourceKind::Document,
            CliResourceKind::Heading => ResourceKind::Heading,
            CliResourceKind::Attachment => ResourceKind::Attachment,
            CliResourceKind::Block => ResourceKind::Block,
        }
    }
}

#[derive(Args, Debug)]
pub struct LinkSubcommand {
    #[command(subcommand)]
    pub command: LinkCommands,
}

#[derive(Subcommand, Debug)]
pub enum LinkCommands {
    /// List link occurrences for a resource
    List {
        r_ref: String,
    },
    /// List resolved relations for a resource
    Resolved {
        r_ref: String,
    },
}

#[derive(Args, Debug)]
pub struct SpaceSubcommand {
    #[command(subcommand)]
    pub command: SpaceCommands,
}

#[derive(Args, Debug)]
pub struct McpSubcommand {
    #[command(subcommand)]
    pub command: McpCommands,
}

#[derive(Subcommand, Debug)]
pub enum McpCommands {
    /// Start MCP stdio server
    Serve,
}
#[derive(Args, Debug)]
pub struct TaskSubcommand {
    #[command(subcommand)]
    pub command: Option<TaskCommands>,
}

#[derive(Subcommand, Debug)]
pub enum TaskCommands {
    /// Transition task state (e.g., TODO -> DONE)
    Transition {
        r_ref: String,
        #[arg(long)]
        to: String,
        #[arg(long, default_value = "2026-07-22 Wed 16:00")]
        timestamp: String,
    },
    
    /// List all tasks grouped by status
    List,

    /// Agenda view of scheduled, deadline, or actionable items
    Agenda,

    /// Overview of PARA (Projects, Areas, Resources, Archives) structures
    Para,

    /// List background system tasks/jobs
    Jobs,
}
#[derive(Args, Debug)]
pub struct AttachmentSubcommand {
    #[command(subcommand)]
    pub command: AttachmentCommands,
}

#[derive(Subcommand, Debug)]
pub enum AttachmentCommands {
    /// Add an attachment file to space
    Add {
        #[arg(long)]
        path: PathBuf,

        #[arg(long, default_value = "application/octet-stream")]
        mime: String,
    },

    /// Run text/metadata extraction on an attachment
    Extract { r_ref: String },

    /// Query extracted text segments for an attachment
    Segments { r_ref: String },
}
#[derive(Args, Debug)]
pub struct SourceSubcommand {
    #[command(subcommand)]
    pub command: SourceCommands,
}
#[derive(Args, Debug)]
pub struct SyncSubcommand {
    #[command(subcommand)]
    pub command: SyncCommands,
}

#[derive(Subcommand, Debug)]
pub enum SyncCommands {
    /// Push space changes to shared folder
    Push {
        #[arg(long, default_value = "default_actor")]
        actor: String,

        #[arg(long)]
        folder: PathBuf,
    },

    /// Pull changes from shared folder into space
    Pull {
        #[arg(long, default_value = "default_actor")]
        actor: String,

        #[arg(long)]
        folder: PathBuf,
    },

    /// List active sync conflicts
    Conflicts,

    /// Trigger relay transport sync
    Relay {
        #[arg(long)]
        id: String,
    },
}
#[derive(Subcommand, Debug)]
pub enum SpaceCommands {
    /// Rebuild SQLite projection for the space
    Rebuild,

    /// Run space integrity diagnostics (space doctor)
    Doctor,
}


#[derive(Args, Debug)]
pub struct ArtifactSubcommand {
    #[command(subcommand)]
    pub command: ArtifactCommands,
}

#[derive(Subcommand, Debug)]
pub enum ArtifactCommands {
    /// Check artifact freshness against source files
    Stale,
}
#[derive(Args, Debug)]
pub struct CommunitySubcommand {
    #[command(subcommand)]
    pub command: CommunityCommands,
}

#[derive(Subcommand, Debug)]
pub enum CommunityCommands {
    /// Create a community
    Create {
        #[arg(long)]
        id: String,

        #[arg(long)]
        name: String,

        #[arg(long)]
        kind: Option<CliResourceKind>,

        #[arg(long)]
        title_contains: Option<String>,
    },

    /// List configured communities
    List,
}

#[derive(Args, Debug)]
pub struct DeriveArgs {
    #[arg(long)]
    pub community: String,

    #[arg(long)]
    pub recipe: String,
}

#[derive(Args, Debug)]
pub struct SkillSubcommand {
    #[command(subcommand)]
    pub command: SkillCommands,
}

#[derive(Subcommand, Debug)]
pub enum SkillCommands {
    /// Export SKILL.md package
    Export {
        #[arg(long)]
        community: String,

        #[arg(long, default_value = "Exported Skill Package")]
        description: String,

        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum SourceCommands {
    /// Add an external source
    Add {
        #[arg(long)]
        id: String,

        #[arg(long)]
        kind: CliSourceKind,

        #[arg(long)]
        path: PathBuf,

        #[arg(long, default_value_t = false)]
        read_only: bool,
    },

    /// List configured sources
    List,

    /// Sync / scan federated sources
    Sync,

    /// Trigger writeback mutation to a writable external source
    Writeback {
        #[arg(long)]
        id: String,

        #[arg(long)]
        r_ref: String,

        #[arg(long)]
        payload: String,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliSourceKind {
    Native,
    Git,
    Obsidian,
    Anytype,
}

impl From<CliSourceKind> for source::SourceKind {
    fn from(k: CliSourceKind) -> Self {
        match k {
            CliSourceKind::Native => source::SourceKind::Native,
            CliSourceKind::Git => source::SourceKind::Git,
            CliSourceKind::Obsidian => source::SourceKind::Obsidian,
            CliSourceKind::Anytype => source::SourceKind::Anytype,
        }
    }
}
