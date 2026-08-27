//! Web pages and reusable UI components.

pub mod activity;
pub mod detail;
pub mod graph;
pub mod home;
pub mod layout;
pub mod list;
pub mod palette;
pub mod panel_files;
pub mod panel_graph;
pub mod panel_props;
pub mod panel_tree;
pub mod preview;
pub mod sidebar;
pub mod space_home;
pub mod ui;

pub use activity::ActivityPage;
pub use detail::DetailPage;
pub use graph::GraphPage;
pub use home::HomePage;
pub use layout::use_space_layout;
pub use list::ListPage;
pub use palette::{CommandPalette, SearchTrigger};
pub use panel_files::FilesPanel;
pub use panel_graph::GraphPanel;
pub use panel_props::PropertiesPanel;
pub use panel_tree::TreePanel;
pub use preview::PreviewPage;
pub use sidebar::{SpaceDropdown, SpaceSidebar};
pub use space_home::SpaceHome;
pub use ui::{Breadcrumb, BreadcrumbSegment, KindIcon};
