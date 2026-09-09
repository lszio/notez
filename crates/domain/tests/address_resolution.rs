use notez_domain::{
    BlockLocator, ContentFingerprint, ObjectAddress, PositionedRef, SourceId, SourcePath, SpaceId,
};

#[test]
fn positioned_address_keeps_source_relative_path_and_fingerprint() {
    let address = ObjectAddress::Positioned(PositionedRef {
        space_id: SpaceId::new("work"),
        source_id: SourceId::new("notes"),
        document_path: SourcePath::parse("journal/today.md").unwrap(),
        locator: BlockLocator::line_span(4, 7),
        fingerprint: ContentFingerprint::from_bytes(b"hello"),
    });

    assert_eq!(address.source_id(), Some(&SourceId::new("notes")));
    assert_eq!(address.document_path().unwrap().as_str(), "journal/today.md");
}

#[test]
fn source_path_rejects_escape_and_absolute_paths() {
    assert!(SourcePath::parse("../secret.md").is_err());
    assert!(SourcePath::parse("/etc/passwd").is_err());
    assert!(SourcePath::parse(r"C:\\notes\\today.md").is_err());
}
