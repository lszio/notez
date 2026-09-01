//! Web pages and reusable UI components.

pub mod activity;
pub mod graph;
pub mod home;
pub mod journal;
pub mod layout;
pub mod list;
pub mod note;
pub mod palette;
pub mod panel_backlinks;
pub mod panel_files;
pub mod panel_graph;
pub mod panel_outline;
pub mod panel_props;
pub mod panel_tree;
pub mod preview;
pub mod sidebar;
pub mod space_home;
pub mod ui;

pub use activity::ActivityPage;
pub use graph::GraphPage;
pub use home::HomePage;
pub use journal::JournalPage;
pub use layout::use_space_layout;
pub use list::ListPage;
pub use note::NotePage;
pub use palette::{IntentPalette, SearchTrigger};
pub use panel_backlinks::BacklinksPanel;
pub use panel_files::FilesPanel;
pub use panel_graph::GraphPanel;
pub use panel_outline::OutlinePanel;
pub use panel_props::PropertiesPanel;
pub use panel_tree::TreePanel;
pub use preview::PreviewPage;
pub use sidebar::SpaceDropdown;
pub use space_home::SpaceHome;
pub use ui::{Breadcrumb, BreadcrumbSegment, KindIcon};
