use crate::application::ApplicationService;
use crate::domain::ResourceRef;
use std::fs;
use crate::storage::SqliteProjection;

#[test]
fn agenda_views_task_transitions_and_para_overview() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_root = temp_dir.path();
    let file = source_root.join("tasks.org");
    let content = r#"#+title: Tasks Space
#+ID: 01J00000000000000000000600

* NEXT Important design review
:PROPERTIES:
:ID: 01J00000000000000000000601
:SCHEDULED: <2026-07-22 Wed>
:TYPE: project
:END:

* TODO Secondary task
:PROPERTIES:
:ID: 01J00000000000000000000602
:DEADLINE: <2026-07-25 Sat>
:END:
"#;
    fs::write(&file, content).unwrap();

    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(source_root).unwrap();

    let agenda = service.agenda().unwrap();
    assert_eq!(agenda.items.len(), 2);
    assert_eq!(agenda.items[0].title, "Important design review");

    let r_ref = ResourceRef::parse("heading:01J00000000000000000000601").unwrap();
    let transition_res = service
        .transition_task(&r_ref, "DONE", "2026-07-22 Wed 16:00")
        .unwrap();

    assert_eq!(transition_res.to_state, "DONE");
    assert!(transition_res.closed_timestamp.is_some());

    let updated = service.read(&r_ref).unwrap().unwrap();
    assert_eq!(
        updated.properties.get("TODO").map(|s| s.as_str()),
        Some("DONE")
    );

    let para = service.para_overview().unwrap();
    assert_eq!(para.projects.len(), 1);
    assert_eq!(para.projects[0].resource.r#ref, r_ref);
}
