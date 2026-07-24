use application::ApplicationService;
use domain::{LinkTarget, ResolutionStatus};
use std::fs;
use std::path::Path;
use storage::SqliteProjection;

#[test]
fn test_all_link_forms_resolution() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path();
    
    // Create an org file with various links
    let org_content = r#"
#+title: Link Test
#+ID: 01J000000000000000000000A1

* Heading 1
:PROPERTIES:
:ID: 01J000000000000000000000H1
:END:

See [[id:01J000000000000000000000H1][Self heading]].
See [[file:other.md::#some-section]].
See [[https://example.com]].
See [[Other Note]].
"#;
    fs::write(root.join("test.org"), org_content).unwrap();
    
    // Create a markdown file with various links
    let md_content = r#"---
title: Other Note
id: 01J000000000000000000000M1
---

# some-section <!-- id: 01J000000000000000000000H2 -->

See [[Link Test]] and [Org doc](test.org) and [[id:01J000000000000000000000A1]].
"#;
    fs::write(root.join("other.md"), md_content).unwrap();
    
    let db_path = root.join(".notez/index.sqlite");
    fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    let store = SqliteProjection::open(&db_path).unwrap();
    let mut service = ApplicationService::new(store);
    
    service.scan_native(root).unwrap();
    
    let doc1_ref = domain::ResourceRef::parse("document:01J000000000000000000000A1").unwrap();
    let heading1_ref = domain::ResourceRef::parse("heading:01J000000000000000000000H1").unwrap();
    let occurrences1 = service.query_link_occurrences(&heading1_ref).unwrap();
    assert_eq!(occurrences1.len(), 4);
    
    // Test resolution
    let _relations1 = service.query_resolved_relations(&heading1_ref).unwrap();
    
    let page = service.query(&domain::Selector::new()).unwrap();
    for res in page.items {
        println!("RES: {} - {}", res.r#ref, res.title);
        let occs = service.query_link_occurrences(&res.r#ref).unwrap();
        for occ in occs {
            println!("  OCC: {:?}", occ.target);
        }
    }
    let heading2_ref = domain::ResourceRef::parse("heading:01J000000000000000000000H2").unwrap();
    let occurrences2 = service.query_link_occurrences(&heading2_ref).unwrap();
    assert_eq!(occurrences2.len(), 3);
    
    let relations2 = service.query_resolved_relations(&heading2_ref).unwrap();
    // Verify some link targets from markdown
    assert!(matches!(&occurrences2[0].target, LinkTarget::Title { title, .. } if title == "Link Test"));
    assert!(matches!(&occurrences2[1].target, LinkTarget::File { path, .. } if path == "test.org"));
    assert!(matches!(&occurrences2[2].target, LinkTarget::Id { value, .. } if value == "01J000000000000000000000A1"));
    
    // Verify resolution statuses
    // [[id:01J000000000000000000000A1]] should be Resolved
    let resolved_id = relations2.iter().find(|r| matches!(&r.target, LinkTarget::Id { .. })).unwrap();
    assert_eq!(resolved_id.status, ResolutionStatus::Resolved);
    assert_eq!(resolved_id.target_ref, doc1_ref);
    
    // [[Link Test]] should be Resolved
    let resolved_title = relations2.iter().find(|r| matches!(&r.target, LinkTarget::Title { .. })).unwrap();
    assert_eq!(resolved_title.status, ResolutionStatus::Resolved);
    assert_eq!(resolved_title.target_ref, doc1_ref);
}
