use crate::domain::schema::{
    PropertyType, SchemaField, Trait, TypeDefinition, TypeRegistry, ValidationError,
};
use crate::domain::{Resource, ResourceKind, ResourceRef};
use std::collections::BTreeMap;

#[test]
fn schema_definition_and_validation() {
    let mut registry = TypeRegistry::new();

    let mut project_fields = BTreeMap::new();
    project_fields.insert(
        "area".to_string(),
        SchemaField {
            property_type: PropertyType::Ref,
            required: true,
            target_types: vec!["area".to_string()],
        },
    );
    project_fields.insert(
        "effort".to_string(),
        SchemaField {
            property_type: PropertyType::Duration,
            required: false,
            target_types: vec![],
        },
    );

    let project_def = TypeDefinition {
        name: "project".to_string(),
        traits: vec![
            Trait::Taskable,
            Trait::Schedulable,
            Trait::ParaItem,
            Trait::Summarizable,
        ],
        fields: project_fields,
    };

    registry.register(project_def);

    let valid_ref = ResourceRef::parse("heading:01J00000000000000000000001").unwrap();
    let mut valid_props = BTreeMap::new();
    valid_props.insert("TYPE".to_string(), "project".to_string());
    valid_props.insert(
        "area".to_string(),
        "heading:01J00000000000000000000002".to_string(),
    );
    valid_props.insert("effort".to_string(), "2h".to_string());

    let valid_res = Resource {
        r#ref: valid_ref,
        kind: ResourceKind::Heading,
        title: "Project Alpha".to_string(),
        revision: "rev1".to_string(),
        source_id: "native".to_string(),
        locator: "/path/alpha.org".to_string(),
        properties: valid_props,
    };

    assert!(registry.validate(&valid_res).is_ok());

    let mut invalid_props = BTreeMap::new();
    invalid_props.insert("TYPE".to_string(), "project".to_string());

    let invalid_res = Resource {
        r#ref: valid_ref,
        kind: ResourceKind::Heading,
        title: "Invalid Project".to_string(),
        revision: "rev1".to_string(),
        source_id: "native".to_string(),
        locator: "/path/invalid.org".to_string(),
        properties: invalid_props,
    };

    let err = registry.validate(&invalid_res).unwrap_err();
    assert!(matches!(err, ValidationError::MissingRequiredField { field } if field == "area"));
}
