use domain::{Projection, ResourceKind, ResourceRef, Selector};

#[test]
fn resource_refs_and_selectors_compose() {
    let id = ResourceRef::parse("heading:01J00000000000000000000000").unwrap();
    assert_eq!(id.kind(), ResourceKind::Heading);
    let selector = Selector::kind(ResourceKind::Heading).with_title_contains("sync");
    assert_eq!(selector.kind, Some(ResourceKind::Heading));
    assert_eq!(selector.title_contains.as_deref(), Some("sync"));
    assert_eq!(
        Projection::summary().fields,
        vec!["ref", "title", "revision"]
    );
}
#[test]
fn invalid_resource_ref_parsing() {
    assert!(ResourceRef::parse("invalid:01J00000000000000000000000").is_err());
    assert!(ResourceRef::parse("heading:not-a-valid-ulid").is_err());
    assert!(ResourceRef::parse("malformed_ref_no_colon").is_err());
}
