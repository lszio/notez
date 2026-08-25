use crate::domain::community::Community;
use crate::domain::{Resource, ResourceKind, ResourceRef, Selector};
use std::collections::BTreeMap;

#[test]
fn community_creation_and_member_filtering() {
    let mut selector = Selector::kind(ResourceKind::Heading);
    selector.title_contains = Some("sync".to_string());

    let pinned_ref = ResourceRef::parse("heading:01J00000000000000000000010").unwrap();
    let excluded_ref = ResourceRef::parse("heading:01J00000000000000000000020").unwrap();

    let community = Community {
        id: "comm_devsync".to_string(),
        name: "DevSync Community".to_string(),
        selector,
        pinned_members: vec![pinned_ref],
        excluded_members: vec![excluded_ref],
    };

    let res_matching = Resource {
        r#ref: ResourceRef::parse("heading:01J00000000000000000000030").unwrap(),
        kind: ResourceKind::Heading,
        title: "Architecture sync".to_string(),
        revision: "rev1".to_string(),
        source_id: "native".to_string(),
        locator: "/path.org".to_string(),
        properties: BTreeMap::new(),
        object_id: notez_core::domain::ObjectIdentity::default(),
        primary_source_id: String::new(),
    };

    let res_excluded = Resource {
        r#ref: excluded_ref,
        kind: ResourceKind::Heading,
        title: "Excluded sync".to_string(),
        revision: "rev1".to_string(),
        source_id: "native".to_string(),
        locator: "/path2.org".to_string(),
        properties: BTreeMap::new(),
        object_id: notez_core::domain::ObjectIdentity::default(),
        primary_source_id: String::new(),
    };

    let res_pinned = Resource {
        r#ref: pinned_ref,
        kind: ResourceKind::Heading,
        title: "Pinned Item".to_string(),
        revision: "rev1".to_string(),
        source_id: "native".to_string(),
        locator: "/path3.org".to_string(),
        properties: BTreeMap::new(),
        object_id: notez_core::domain::ObjectIdentity::default(),
        primary_source_id: String::new(),
    };

    let resources = vec![res_matching.clone(), res_excluded, res_pinned.clone()];

    let members = community.filter_members(&resources);
    assert_eq!(members.len(), 2);
    let member_refs: Vec<ResourceRef> = members.iter().map(|r| r.r#ref).collect();
    assert!(member_refs.contains(&res_matching.r#ref));
    assert!(member_refs.contains(&pinned_ref));
    assert!(!member_refs.contains(&excluded_ref));
}
