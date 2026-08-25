//! Honesty contracts for `writeback_resource` and stub-backed sources.
//!
//! These tests pin the Phase-0 honesty fixes:
//!
//! 1. `writeback_resource` never falls back to the process working
//!    directory — it refuses without an explicit `SourceContext`.
//! 2. A stub-backed source that was opted into explicitly (e.g.
//!    `AnytypeFactory`) reports no capabilities, so a write attempt is
//!    refused with `ReadOnlySource` instead of reporting fake success.
//! 3. `relay_sync` stays an unsupported capability until the 0.6 Change
//!    delivery model lands.

use notez_core::application::ApplicationFacade;
use notez_core::application::context::SourceContext;
use notez_core::application::use_cases::SyncUseCase;
use notez_core::config::model::{
    SourceConfig as SpaceConfig, SourceIdentity, SourceInstanceConfig, WorkflowConfig,
};
use notez_core::source::SourceKind;
use notez_core::source::registry::AnytypeFactory;
use notez_core::storage::SqliteProjection;
use std::path::{Path, PathBuf};

fn space_config_with_stub_source(root: &Path) -> SpaceConfig {
    SpaceConfig {
        version: 2,
        source: SourceIdentity {
            name: "test-space".into(),
            database: PathBuf::from(".notez/index.sqlite"),
        },
        workflow: WorkflowConfig::default(),
        sources: vec![SourceInstanceConfig {
            id: "anysrc".into(),
            kind: SourceKind::Anytype,
            path: root.to_path_buf(),
            read_only: false,
            include_paths: vec![],
            exclude_paths: vec![],
        }],
        link_overrides: serde_json::Value::Null,
    }
}

#[test]
fn writeback_requires_explicit_source_context() {
    let store = SqliteProjection::in_memory().unwrap();
    let service = ApplicationFacade::new(store);

    let err = service
        .writeback_resource(
            "any_src",
            "heading:01J00000000000000000000999",
            "updated_title",
        )
        .expect_err("writeback must not fall back to the process cwd");
    assert!(
        err.to_string().contains("SourceContext"),
        "unexpected error: {err}"
    );
}

#[test]
fn writeback_against_opt_in_stub_source_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteProjection::in_memory().unwrap();
    let ctx = SourceContext::new(
        "test-space",
        dir.path().to_path_buf(),
        space_config_with_stub_source(dir.path()),
    );
    let mut facade = ApplicationFacade::with_source(store, ctx);

    // Opt-in the stub factory explicitly; `with_builtins()` must NOT have
    // registered it already.
    facade.register_source_factory(Box::new(AnytypeFactory));

    let err = facade
        .writeback_resource(
            "anysrc",
            "heading:01J00000000000000000000999",
            "updated_title",
        )
        .expect_err("stub adapter claims can_write=false; write must be refused");
    match err {
        notez_core::ApplicationError::ReadOnlySource { source_id } => {
            assert_eq!(source_id, "anysrc");
        }
        other => panic!("expected ReadOnlySource, got: {other:?}"),
    }
}

#[test]
fn relay_sync_reports_unsupported() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteProjection::in_memory().unwrap();
    let config = space_config_with_stub_source(dir.path());
    let service = ApplicationFacade::with_source(
        store,
        SourceContext::new("any_src", dir.path().to_path_buf(), config),
    );

    let err = <ApplicationFacade<_> as SyncUseCase>::relay_sync(&service)
        .expect_err("relay sync is not implemented yet");
    assert!(
        err.to_string().contains("not yet implemented"),
        "unexpected error: {err}"
    );
}
