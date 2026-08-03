use crate::artifact::skill::{SkillExporter, SkillIr};
use crate::domain::{Resource, ResourceKind, ResourceRef};
use std::collections::BTreeMap;
use tempfile::tempdir;

#[test]
fn compile_skill_ir_and_export_skill_package() {
    let temp_dir = tempdir().unwrap();
    let export_path = temp_dir.path().join("my_skill");

    let res1 = Resource {
        r#ref: ResourceRef::parse("heading:01J00000000000000000000080").unwrap(),
        kind: ResourceKind::Heading,
        title: "Skill Core Directive".to_string(),
        revision: "rev1".to_string(),
        source_id: "native".to_string(),
        locator: "/path/skill.org".to_string(),
        properties: BTreeMap::new(),
        object_id: notez_core::domain::ObjectId::default(),
    };

    let resources = vec![res1];

    let skill_ir = SkillIr::compile("my_skill", "My Skill Description", &resources);
    assert_eq!(skill_ir.name, "my_skill");

    let _package = SkillExporter::export(&skill_ir, &export_path).unwrap();

    assert!(export_path.join("SKILL.md").exists());
    assert!(export_path.join("resources.json").exists());

    let skill_md_content = std::fs::read_to_string(export_path.join("SKILL.md")).unwrap();
    assert!(skill_md_content.contains("name: my_skill"));
    assert!(skill_md_content.contains("My Skill Description"));
}
