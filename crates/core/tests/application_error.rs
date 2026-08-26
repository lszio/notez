//! Contract tests for the structured `ApplicationError` variants.

use notez_core::application::ApplicationError;
use notez_core::capability::Mutability;
use notez_core::domain::{ResourceKind, ResourceRef, ProjectionReader, ProjectionWrite};

#[test]
fn not_found_display_and_serde_round_trip() {
    let r_ref = ResourceRef::parse("heading:01J00000000000000000000F01").unwrap();
    let err = ApplicationError::NotFound {
        kind: ResourceKind::Heading,
        r_ref,
    };
    assert_eq!(
        err.to_string(),
        "resource not found: heading:01J00000000000000000000F01 (kind=heading)"
    );
    let s = serde_json::to_string(&err).unwrap();
    let back: ApplicationError = serde_json::from_str(&s).unwrap();
    assert_eq!(back, err);
}

#[test]
fn storage_display_and_serde_round_trip() {
    let err = ApplicationError::Storage {
        kind: notez_core::application::StorageErrorKind::Sqlite,
        message: "db busy".to_string(),
    };
    assert_eq!(err.to_string(), "storage error (sqlite): db busy");
    let s = serde_json::to_string(&err).unwrap();
    let back: ApplicationError = serde_json::from_str(&s).unwrap();
    assert_eq!(back, err);
}

#[test]
fn unsupported_capability_display() {
    let err = ApplicationError::UnsupportedCapability {
        capability: "space doctor",
    };
    assert_eq!(err.to_string(), "unsupported capability: space doctor");
}

#[test]
fn source_not_found_display() {
    let err = ApplicationError::SourceNotFound {
        source_id: "vault".to_string(),
    };
    assert_eq!(err.to_string(), "source not found: vault");
}

#[test]
fn read_only_source_display() {
    let err = ApplicationError::ReadOnlySource {
        source_id: "vault".to_string(),
    };
    assert_eq!(err.to_string(), "read-only source: vault");
}

#[test]
fn revision_conflict_display() {
    let err = ApplicationError::RevisionConflict {
        expected: "r1".to_string(),
        actual: "r2".to_string(),
    };
    assert_eq!(err.to_string(), "revision conflict: expected r1, actual r2");
}

#[test]
fn io_display_and_serde_round_trip() {
    let err = ApplicationError::Io {
        path: None,
        source: std::io::ErrorKind::NotFound,
    };
    assert!(err.to_string().starts_with("io error: "));
    let s = serde_json::to_string(&err).unwrap();
    let back: ApplicationError = serde_json::from_str(&s).unwrap();
    assert_eq!(back, err);
}

#[test]
fn document_display_org_variant() {
    let err = ApplicationError::Document {
        source: notez_core::application::DocumentErrorKind::Org(
            notez_core::document::OrgDocumentError::Other("parse failed".into()),
        ),
    };
    assert_eq!(err.to_string(), "document error (org): parse failed");
}
