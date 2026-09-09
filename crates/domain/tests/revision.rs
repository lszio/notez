use notez_domain::Revision;

#[test]
fn revision_preserves_its_wire_value() {
    let revision = Revision::new("sha256:abc123");
    assert_eq!(revision.as_str(), "sha256:abc123");
}
