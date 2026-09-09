//! Shared Notez UI primitives.

mod navbar;
pub use navbar::Navbar;

pub mod notez;
pub use notez::{NzBadge, NzButton, NzButtonGhost, NzCard, NzInput};

pub mod store;
pub use store::AppStore;

pub mod backend;
pub use backend::{Backend, ResourceRow, SpaceRow};

pub mod workspace;
pub use workspace::{build_tree, NzDocBody, NzShell, NzTree, TreeEntry, TreeFile};

/// The workspace stylesheet, shared by every surface.
///
/// Web serves it at `/app.css`; desktop and mobile inject it with
/// `document::Style` so all three render the same shell, tree and
/// document styles.
pub const WORKSPACE_CSS: &str = include_str!("../assets/workspace.css");
