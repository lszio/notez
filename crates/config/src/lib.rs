pub mod discovery;
pub mod defaults;
pub mod merge;
pub mod migrate;
pub mod model;
pub use merge::load_runtime_config;
pub use discovery::{select_space, ConfigPaths, SelectedSpace, SpaceSelector};

pub use model::{
    ConfigError, GlobalConfig, Preferences, RuntimeConfig, SpaceConfig, SpaceIdentity,
    SpaceRegistration, SpaceSourceConfig, WorkflowConfig,
};
