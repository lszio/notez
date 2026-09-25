//! Pin that `PreviewContext` carries no concrete engine or storage type.
//! Preview rendering consumes only caller-provided data.

use notez_core::domain::{Resource, ResourceKind, ResourceRef};
use notez_preview::{
    default_catalog, MediaKind, PreviewCapabilities, PreviewContext, PreviewModel, Previewer,
    PreviewerCatalog, ViewerFormat,
};

fn make_ctx<'a>(
    bytes: &'static [u8],
    mime: &'static str,
    locator: &str,
    catalog: &'a PreviewerCatalog,
) -> PreviewContext<'a> {
    PreviewContext {
        resource: Resource {
            r#ref: ResourceRef::parse("document:01J000000000000000000000D1").unwrap(),
            kind: ResourceKind::Document,
            title: "T".to_string(),
            revision: "r1".to_string(),
            source_id: "native".to_string(),
            locator: locator.to_string(),
            properties: Default::default(),
            object_id: notez_core::domain::ObjectIdentity::default(),
            primary_source_id: String::new(),
        },
        bytes: Some(bytes::Bytes::from_static(bytes)),
        mime: Some(mime.to_string()),
        locator: std::path::PathBuf::from(locator),
        segments: vec![],
        siblings: vec![],
        catalog,
    }
}

#[test]
fn preview_context_type_does_not_carry_application_service_handle() {
    let catalog = default_catalog();
    let ctx = make_ctx(b"# Hello\n", "text/markdown", "/x.md", &catalog);
    let previewer = ctx.catalog.resolve(Some("fallback"), &ctx);
    assert!(previewer.is_some());
}

#[test]
fn render_with_default_catalog_needs_no_application_service() {
    let catalog = default_catalog();
    let ctx = make_ctx(b"* heading\n", "text/org", "/y.org", &catalog);
    let previewer = ctx
        .catalog
        .resolve(None, &ctx)
        .expect("at least one previewer must match a default context");
    let _model = previewer.render(&ctx).expect("render must succeed");
}

#[test]
fn media_matcher_claims_video_and_audio() {
    let catalog = default_catalog();
    let mp4 =
        catalog.resolve(None, &make_ctx(b"", "video/mp4", "/clips/clip.mp4", &catalog));
    let mp3 =
        catalog.resolve(None, &make_ctx(b"", "audio/mpeg", "/clips/clip.mp3", &catalog));
    assert!(mp4.is_some(), "mp4 must match the media previewer");
    assert_eq!(mp4.unwrap().id(), "media");
    assert!(mp3.is_some(), "mp3 must match the media previewer");

    // `render` must succeed without the byte payload: media never
    // inspects the body, it just hands the raw URL to the UI.
    let model = mp4.unwrap().render(&make_ctx(b"", "video/mp4", "/x.mp4", &catalog));
    match model {
        Ok(PreviewModel::Media { media: MediaKind::Video }) => {}
        other => panic!("expected Media/Video model, got {other:?}"),
    }
}

#[test]
fn media_falls_back_to_extension_when_mime_missing() {
    let catalog = default_catalog();
    let ctx = make_ctx(b"", "application/octet-stream", "/a/clip.mka", &catalog);
    let p = catalog.resolve(None, &ctx).expect("must match by extension");
    assert_eq!(p.id(), "media");
    let model = p.render(&ctx).expect("render");
    match model {
        PreviewModel::Media { media: MediaKind::Audio } => {}
        other => panic!("expected Media/Audio from .mka, got {other:?}"),
    }
}

#[test]
fn docx_and_pptx_render_viewer_shells_not_text() {
    let catalog = default_catalog();
    let docx =
        catalog.resolve(None, &make_ctx(b"zip", "docx mime", "/files/a.docx", &catalog));
    assert_eq!(docx.expect("docx resolves").id(), "docx");
    let docx_model = docx.unwrap().render(&make_ctx(b"zip", "docx", "/a.docx", &catalog));
    assert!(
        matches!(docx_model, Ok(PreviewModel::Viewer { format: ViewerFormat::Docx })),
        "docx now renders a Viewer shell, not paragraphs"
    );

    let pptx =
        catalog.resolve(None, &make_ctx(b"zip", "pptx mime", "/files/a.pptx", &catalog));
    assert_eq!(pptx.expect("pptx resolves").id(), "pptx");
    let pptx_model = pptx.unwrap().render(&make_ctx(b"zip", "pptx", "/a.pptx", &catalog));
    assert!(matches!(
        pptx_model,
        Ok(PreviewModel::Viewer { format: ViewerFormat::Pptx })
    ));
}

#[test]
fn office_advertise_can_edit_capability() {
    let catalog = default_catalog();
    let docx = catalog
        .resolve(None, &make_ctx(b"", "", "/a.docx", &catalog))
        .unwrap();
    let pptx = catalog
        .resolve(None, &make_ctx(b"", "", "/a.pptx", &catalog))
        .unwrap();
    assert_eq!(
        docx.capabilities(),
        PreviewCapabilities { can_edit: true },
        "docx must declare can_edit so the replace-file affordance shows"
    );
    assert_eq!(
        pptx.capabilities(),
        PreviewCapabilities { can_edit: true },
        "pptx must declare can_edit"
    );
}

#[test]
fn default_capabilities_disables_editing() {
    // Markdown is editable via the document editor path, so the
    // previewer must NOT declare can_edit (otherwise the page would
    // show both the textarea editor and the upload-replace form).
    let catalog = default_catalog();
    let md = catalog
        .resolve(None, &make_ctx(b"# H\n", "text/markdown", "/a.md", &catalog))
        .unwrap();
    assert_eq!(md.capabilities(), PreviewCapabilities::default());
}

#[test]
fn extension_previewers_can_register_into_the_catalog() {
    // Custom previewer that registers a new view on a fresh id —
    // mirrors how a third-party crate can plug in without changing
    // the core catalog.
    struct Hello;

    impl Previewer for Hello {
        fn id(&self) -> &'static str {
            "hello-extension"
        }
        fn matches(&self, ctx: &PreviewContext) -> bool {
            ctx.locator.to_string_lossy().ends_with(".hello")
        }
        fn render(&self, _ctx: &PreviewContext) -> Result<PreviewModel, notez_preview::PreviewError> {
            Ok(PreviewModel::Html {
                html: "<p>hi</p>".to_string(),
            })
        }
    }

    // Build a fresh catalog and demonstrate the extension contract:
    // custom previewers are appended; registration order matters; the
    // Fallback (last) only wins when no earlier previewer claims the
    // context. A third-party crate wires its previewers in via
    // `register` at composition time, before the Fallback is appended.
    let mut catalog = PreviewerCatalog::new();
    catalog.register(Hello);
    catalog.register(notez_preview::builders::org::OrgPreviewer);
    catalog.register(notez_preview::builders::fallback::FallbackPreviewer);

    let ctx = make_ctx(b"", "", "/notes/welcome.hello", &catalog);
    let p = catalog.resolve(None, &ctx).expect("extension must match");
    assert_eq!(p.id(), "hello-extension");
    let model = p.render(&ctx).expect("render");
    assert!(matches!(model, PreviewModel::Html { .. }));

    // The built-in Org previewer still claims its shapes.
    let org_ctx = make_ctx(b"* h\n", "text/org", "/y.org", &catalog);
    assert_eq!(
        catalog.resolve(None, &org_ctx).unwrap().id(),
        "org",
        "Org previewer still wins on .org documents"
    );
}

#[test]
fn html_and_media_models_serde_round_trip() {
    let html = PreviewModel::Html {
        html: "<b>hi</b>".to_string(),
    };
    let media = PreviewModel::Media {
        media: MediaKind::Video,
    };
    let viewer = PreviewModel::Viewer {
        format: ViewerFormat::Docx,
    };
    for m in [&html, &media, &viewer] {
        let s = serde_json::to_string(m).expect("serialize");
        let back: PreviewModel = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(back, *m);
    }
}
