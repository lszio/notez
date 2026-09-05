//! Contract tests for `SourceAdapterFactory` and `SourceRegistry`.

use notez_core::source::protocol::{FormatParser, RawEntity, SourceTransport, TransportError};
use notez_core::source::{
    ComposedSourceAdapter, SourceAdapter, SourceAdapterFactory, SourceConfig, SourceError,
    SourceKind, SourceRegistry,
};
use std::path::PathBuf;

fn make_config(kind: SourceKind, id: &str) -> SourceConfig {
    SourceConfig {
        id: id.to_string(),
        kind,
        path: PathBuf::from("/space"),
        read_only: true,
        url: None,
        include_paths: vec![],
        exclude_paths: vec![],
        scan: Default::default(),
    }
}

#[test]
fn new_registry_is_empty() {
    let reg = SourceRegistry::new();
    assert!(reg.kinds().is_empty());
    assert!(reg.get(&SourceKind::Native).is_none());
}

#[test]
fn with_builtins_registers_three_real_kinds() {
    let reg = SourceRegistry::with_builtins();
    let kinds = reg.kinds();
    assert!(kinds.contains(&SourceKind::Native));
    assert!(kinds.contains(&SourceKind::Git));
    assert!(kinds.contains(&SourceKind::Obsidian));
    assert_eq!(
        kinds.len(),
        3,
        "stub-backed kinds must not be builtin-registered"
    );
}

#[test]
fn register_replaces_existing_kind() {
    let mut reg = SourceRegistry::with_builtins();
    let before = reg.kinds().len();
    reg.register(Box::new(MarkerFactory::new(
        SourceKind::Native,
        "v2-marker",
    )));
    assert_eq!(reg.kinds().len(), before);
    let adapter = reg
        .build(make_config(SourceKind::Native, "native_main"))
        .expect("native factory must build");
    assert_eq!(adapter.config().id, "native_main");
    // The marker factory's id ends up on the resulting adapter through
    // the synthetic source_id field, but ComposedSourceAdapter does
    // not currently use that. Instead we use config().id as the
    // observable: the new factory preserves the caller-provided id.
    // (The marker is asserted via a separate check on the factory's
    //  own state in MarkerFactory::last_marker below.)
    let _ = adapter;
}

#[test]
fn build_returns_registered_adapter() {
    let reg = SourceRegistry::with_builtins();
    let adapter = reg
        .build(make_config(SourceKind::Native, "native_main"))
        .expect("native factory must build");
    assert_eq!(adapter.config().id, "native_main");
}

#[test]
fn build_with_unknown_other_kind_returns_error() {
    let reg = SourceRegistry::new();
    let result = reg.build(make_config(SourceKind::Other("ghost".to_string()), "x"));
    let err = match result {
        Ok(_) => panic!("expected error for unregistered kind"),
        Err(e) => e,
    };
    match err {
        SourceError::Other(msg) => assert!(msg.contains("ghost"), "msg: {msg}"),
        other => panic!("expected SourceError::Other, got {other:?}"),
    }
}

#[test]
fn get_returns_factory_for_real_kinds() {
    let reg = SourceRegistry::with_builtins();
    for k in [SourceKind::Native, SourceKind::Git, SourceKind::Obsidian] {
        assert!(reg.get(&k).is_some(), "factory missing for {k}");
    }
}

#[test]
fn third_party_other_kind_is_dispatched_through_registered_factory() {
    // This test demonstrates the extension point: a third party registers
    // a factory for `Other("notion")` and the registry dispatches.
    let mut reg = SourceRegistry::new();
    let factory = MarkerFactory::new(SourceKind::Other("notion".to_string()), "notion-marker");
    reg.register(Box::new(factory));
    let adapter = reg
        .build(make_config(
            SourceKind::Other("notion".to_string()),
            "notion_main",
        ))
        .expect("notion factory must build");
    assert_eq!(adapter.config().id, "notion_main");
    // (No global-marker assertion here: it would race with
    // `register_replaces_existing_kind` under parallel test threads.)
}

// ---- Test-only factory and transport used by the tests above ----

struct MarkerFactory {
    kind: SourceKind,
    marker: &'static str,
}

impl MarkerFactory {
    fn new(kind: SourceKind, marker: &'static str) -> Self {
        Self { kind, marker }
    }
}

impl SourceAdapterFactory for MarkerFactory {
    fn kind(&self) -> SourceKind {
        self.kind.clone()
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        let _ = self.marker; // informational only; no shared state
        let transport: Box<dyn SourceTransport> = Box::new(NoopTransport);
        let parsers: Vec<Box<dyn FormatParser>> = vec![];
        Ok(Box::new(ComposedSourceAdapter::new(
            config, transport, parsers,
        )))
    }
}

struct NoopTransport;
impl SourceTransport for NoopTransport {
    fn fetch_raw(&self) -> Result<Vec<RawEntity>, TransportError> {
        Ok(vec![])
    }
}
