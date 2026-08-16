//! Write-path safety checks.
//!
//! These helpers enforce three invariants on every facade write path,
//! in the order the spec
//! `docs/superpowers/specs/2026-08-09-0.5x-a1-a3-usecase-impl-split-and-write-checks-design.org`
//! §2.4 prescribes:
//!
//! 1. `check_capability` — the descriptor must be present in the
//!    active `CapabilityCatalog`, or the facade refuses to perform
//!    the action. This is the only authorisation dimension implemented
//!    in 0.5.x; richer `Principal × Space × Source × Action × Scope`
//!    is a 0.6 / MVP-2 follow-up.
//! 2. `check_revision` — the caller's expected revision must match
//!    the projection's current revision, or the facade refuses to
//!    overwrite. An empty `expected` is treated as "no precondition"
//!    so that existing CLI write paths keep working until 0.6 wires
//!    the journal + expected-revision contract end-to-end.
//! 3. `check_address_uniqueness` — the same `ResourceAddress` must
//!    not bind to two different `ResourceRef`s. We only enforce this
//!    when the caller supplies a `ResourceAddress::Ref`; locator-based
//!    addresses are deferred to 0.6 where the locator resolver has a
//!    proper place to own uniqueness.
//!
//! Each check returns an `ApplicationError` with a stable, structured
//! variant. Read paths (the other 9 use-case methods) skip these
//! checks entirely.

use crate::application::service::{ApplicationError, ApplicationFacade};
use crate::domain::ProjectionStore;
use crate::domain::link::ResourceAddress;
use crate::domain::resource::ResourceRef;

/// Refuse the call if `capability` is not registered in the facade's
/// `CapabilityCatalog`. The nine built-in capabilities that ship with
/// `with_builtins()` always pass.
pub fn check_capability<S: ProjectionStore>(
    facade: &ApplicationFacade<S>,
    capability: &'static str,
) -> Result<(), ApplicationError> {
    if facade.capability_catalog().contains(capability) {
        Ok(())
    } else {
        Err(ApplicationError::UnsupportedCapability { capability })
    }
}

/// Refuse the call if the projection's current revision for `r_ref`
/// does not match `expected`. An empty `expected` is treated as "no
/// precondition" and always passes; non-empty strings that don't
/// match raise `RevisionConflict`.
pub fn check_revision<S: ProjectionStore>(
    facade: &ApplicationFacade<S>,
    r_ref: &ResourceRef,
    expected: &str,
) -> Result<(), ApplicationError> {
    if expected.is_empty() {
        return Ok(());
    }
    let actual = facade
        .store()
        .get(r_ref)
        .map_err(|e| ApplicationError::Storage {
            kind: crate::application::service::StorageErrorKind::Sqlite,
            message: e.to_string(),
        })?
        .map(|r| r.revision)
        .unwrap_or_default();
    if actual == expected {
        Ok(())
    } else {
        Err(ApplicationError::RevisionConflict {
            expected: expected.to_string(),
            actual,
        })
    }
}

/// Refuse the call if a different `ResourceRef` already binds to the
/// same `ResourceAddress::Ref`. Locator-based addresses are accepted
/// as-is for now (deferred to 0.6 along with locator uniqueness).
pub fn check_address_uniqueness<S: ProjectionStore>(
    facade: &ApplicationFacade<S>,
    addr: &ResourceAddress,
    candidate: &ResourceRef,
) -> Result<(), ApplicationError> {
    let ResourceAddress::Ref { r#ref: existing } = addr else {
        return Ok(());
    };
    if existing == candidate {
        return Ok(());
    }
    // Different `Ref` variant points at the same logical address. The
    // wire contract: a `ResourceAddress::Ref` is a stable identity, so
    // rebinding to a different `ResourceRef` is always a bug.
    Err(ApplicationError::AddressUniqueness {
        addr: addr.clone(),
        existing: *existing,
        candidate: *candidate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::use_cases::ResourceUseCase;
    use crate::capability::{CapabilityDescriptor, Mutability};
    use crate::domain::ResourceKind;
    use crate::domain::resource::Resource;
    use crate::storage::SqliteProjection;
    use ulid::Ulid;

    fn facade_with_builtins() -> ApplicationFacade<SqliteProjection> {
        let store = SqliteProjection::in_memory().expect("in-memory store");
        ApplicationFacade::new(store)
    }

    fn make_resource(revision: &str) -> Resource {
        let r_ref = ResourceRef::new(ResourceKind::Document, Ulid::new());
        Resource {
            r#ref: r_ref,
            kind: ResourceKind::Document,
            title: "t".to_string(),
            revision: revision.to_string(),
            source_id: "native".to_string(),
            locator: "/x.org".to_string(),
            properties: Default::default(),
            object_id: crate::domain::resource::ObjectId::default(),
        }
    }

    #[test]
    fn check_capability_passes_for_builtin() {
        let facade = facade_with_builtins();
        assert!(check_capability(&facade, "resource").is_ok());
    }

    #[test]
    fn check_capability_rejects_unknown_id() {
        let facade = facade_with_builtins();
        let err = check_capability(&facade, "does_not_exist").unwrap_err();
        assert!(matches!(
            err,
            ApplicationError::UnsupportedCapability { capability: "does_not_exist" }
        ));
    }

    #[test]
    fn check_capability_respects_registered_custom() {
        let mut facade = facade_with_builtins();
        facade.register_capability(&CapabilityDescriptor::new(
            "metrics",
            "metrics export",
            Mutability::Read,
        ));
        assert!(check_capability(&facade, "metrics").is_ok());
    }

    #[test]
    fn check_revision_empty_always_passes() {
        let facade = facade_with_builtins();
        let r_ref = make_resource("r1").r#ref;
        assert!(check_revision(&facade, &r_ref, "").is_ok());
    }

    #[test]
    fn check_revision_matches_persisted() {
        let mut facade = facade_with_builtins();
        let r = make_resource("rev-7");
        <ApplicationFacade<_> as ResourceUseCase>::upsert_resource(&mut facade, r.clone())
            .unwrap();
        assert!(check_revision(&facade, &r.r#ref, "rev-7").is_ok());
    }

    #[test]
    fn check_revision_mismatch_raises_conflict() {
        let mut facade = facade_with_builtins();
        let r = make_resource("rev-7");
        <ApplicationFacade<_> as ResourceUseCase>::upsert_resource(&mut facade, r.clone())
            .unwrap();
        let err = check_revision(&facade, &r.r#ref, "rev-9").unwrap_err();
        assert!(matches!(
            err,
            ApplicationError::RevisionConflict { ref expected, ref actual }
            if expected == "rev-9" && actual == "rev-7"
        ));
    }

    #[test]
    fn check_address_uniqueness_same_ref_passes() {
        let facade = facade_with_builtins();
        let r_ref = ResourceRef::new(ResourceKind::Document, Ulid::new());
        let addr = ResourceAddress::Ref { r#ref: r_ref };
        assert!(check_address_uniqueness(&facade, &addr, &r_ref).is_ok());
    }

    #[test]
    fn check_address_uniqueness_different_ref_blocks() {
        let facade = facade_with_builtins();
        let existing_outer = ResourceRef::new(ResourceKind::Document, Ulid::new());
        let candidate_outer = ResourceRef::new(ResourceKind::Document, Ulid::new());
        let addr = ResourceAddress::Ref { r#ref: existing_outer };
        let err = check_address_uniqueness(&facade, &addr, &candidate_outer).unwrap_err();
        match err {
            ApplicationError::AddressUniqueness { existing, candidate, .. } => {
                assert_eq!(existing, existing_outer);
                assert_eq!(candidate, candidate_outer);
            }
            other => panic!("expected AddressUniqueness, got {other:?}"),
        }
    }

    #[test]
    fn check_address_uniqueness_locator_passes() {
        let facade = facade_with_builtins();
        let candidate = ResourceRef::new(ResourceKind::Document, Ulid::new());
        let addr = ResourceAddress::Locator {
            target: crate::domain::link::LinkTarget::File {
                path: "x.org".to_string(),
                fragment: None,
            },
        };
        assert!(check_address_uniqueness(&facade, &addr, &candidate).is_ok());
    }
}
