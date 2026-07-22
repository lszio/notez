use artifact::extractor::{Extractor, ImageMetadataExtractor, SegmentSlicer, TextExtractor};

#[test]
fn text_extractor_and_segment_slicing() {
    let text = "First sentence of the note content. Second sentence providing more details. Third sentence summarizing the findings.";

    let extractor = TextExtractor;
    let extracted = extractor.extract(text.as_bytes(), "text/plain").unwrap();

    assert_eq!(extracted.text, text);

    let slicer = SegmentSlicer::new(50, 10);
    let segments = slicer.slice("att_123", &extracted.text);

    assert!(segments.len() >= 2);
    assert_eq!(segments[0].attachment_ref, "att_123");
    assert_eq!(segments[0].offset_start, 0);
    assert!(segments[0].text.len() <= 50);
}

#[test]
fn image_metadata_extractor() {
    let png_bytes = vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89,
    ];

    let extractor = ImageMetadataExtractor;
    let extracted = extractor.extract(&png_bytes, "image/png").unwrap();

    assert!(extracted.metadata.contains_key("dimensions"));
}
