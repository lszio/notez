use notez_domain::{GraphScope, ObjectId, Relation, RelationKind, RelationScope, SpaceId};

#[test]
fn global_relations_are_not_duplicated_by_space_membership() {
    let relation = Relation::global(
        ObjectId::new("a"),
        ObjectId::new("b"),
        RelationKind::LinksTo,
    );

    assert_eq!(relation.scope, RelationScope::Global);
}

#[test]
fn cross_space_graph_scope_is_explicit() {
    assert!(GraphScope::CrossSpace(vec![SpaceId::new("a"), SpaceId::new("b")]).is_cross_space());
}
