use notez_core::domain::{
    derived_object_id, LinkTarget, ProjectionStore, RelationDirection, RelationType,
    ResolutionStatus, ResolvedRelation, Resource, ResourceKind, ResourceRef, ResourceRelation,
};
use notez_core::storage::SqliteProjection;
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn find_by_object_returns_matching_resources() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("proj.sqlite");
    let mut store = SqliteProjection::open(&db_path).unwrap();

    let object_id = derived_object_id("abc123", "notes/x.md", "h:0");
    let ref_a = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let res_a = Resource {
        r#ref: ref_a,
        kind: ResourceKind::Heading,
        title: "X".to_string(),
        revision: "r1".to_string(),
        source_id: "src_a".to_string(),
        locator: "notes/x.md".to_string(),
        properties: Default::default(),
        object_id,
    };
    store.upsert_resource(&res_a).unwrap();

    let found = store.find_by_object(object_id).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].r#ref, ref_a);
    assert_eq!(found[0].object_id, object_id);
}

#[test]
fn schema_v1_to_v2_migration_backfills_object_id_and_relation_fields() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("legacy.sqlite");

    // Hand-build a v1 schema
    {
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE resources (
                ref TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                revision TEXT NOT NULL,
                source_id TEXT NOT NULL,
                locator TEXT NOT NULL,
                properties_json TEXT NOT NULL
            );
            CREATE TABLE relations (
                source_ref TEXT NOT NULL,
                relation TEXT NOT NULL,
                target_ref TEXT NOT NULL,
                source_id TEXT NOT NULL
            );
            CREATE TABLE resolved_relations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_ref TEXT NOT NULL,
                target_ref TEXT NOT NULL,
                target_json TEXT NOT NULL,
                status TEXT NOT NULL,
                candidates_json TEXT NOT NULL DEFAULT '[]',
                source_id TEXT NOT NULL
            );
            ",
        )
        .unwrap();
        let ref_str = "heading:01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string();
        conn.execute(
            "INSERT INTO resources (ref, kind, title, revision, source_id, locator, properties_json)
             VALUES (?1, 'heading', 'X', 'r1', 'src_a', 'notes/x.md', '{}')",
            rusqlite::params![ref_str],
        )
        .unwrap();
    }

    // Open through SqliteProjection — should run the v1→v2 migration
    let _store = SqliteProjection::open(&db_path).unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let user_version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(user_version, 2, "user_version should be 2 after migration");

    let col_present: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('resources') WHERE name = 'object_id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(col_present, 1, "object_id column should be added by migration");

    for col in ["direction", "creator", "created_at", "evidence_json", "relation_type"] {
        let n: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM pragma_table_info('relations') WHERE name = '{col}'"
                ),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "{col} column should be present after migration");
    }
}

#[test]
fn relation_evidence_round_trip_through_replace_source() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("rel.sqlite");
    let mut store = SqliteProjection::open(&db_path).unwrap();

    let src_ref = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let tgt_ref = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap();
    let rel = ResourceRelation {
        source_ref: src_ref,
        relation: "references".to_string(),
        target_ref: tgt_ref,
        relation_type: RelationType::References,
        direction: RelationDirection::Forward,
        evidence_json: serde_json::json!({"source_id": "src_a", "span_line": 1}),
        created_at: "2026-08-03T00:00:00Z".to_string(),
        creator: "scan".to_string(),
    };
    store
        .replace_source("src_a", vec![], vec![rel], vec![])
        .unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let dir_str: String = conn
        .query_row(
            "SELECT direction FROM relations WHERE source_id = 'src_a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(dir_str, "forward");
    let creator: String = conn
        .query_row(
            "SELECT creator FROM relations WHERE source_id = 'src_a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(creator, "scan");
}

#[test]
fn resolved_relation_evidence_persists_with_direction_forward() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("resolved.sqlite");
    let mut store = SqliteProjection::open(&db_path).unwrap();

    let src_ref = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let tgt_ref = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap();
    let rel = ResolvedRelation {
        source_ref: src_ref,
        target_ref: tgt_ref,
        target: LinkTarget::id("01ARZ3NDEKTSV4RRFFQ69G5FAW", None),
        status: ResolutionStatus::Resolved,
        candidates: vec![],
        relation_type: RelationType::References,
        direction: RelationDirection::Forward,
        evidence_json: serde_json::json!({"source_id": "src_a", "span_line": 5, "rule": "default_profile:markdown"}),
        created_at: "2026-08-03T00:00:00Z".to_string(),
        creator: "scan".to_string(),
    };
    store
        .replace_resolved_relations("src_a", vec![rel])
        .unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let dir_str: String = conn
        .query_row(
            "SELECT direction FROM resolved_relations WHERE source_id = 'src_a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(dir_str, "forward");
    let creator: String = conn
        .query_row(
            "SELECT creator FROM resolved_relations WHERE source_id = 'src_a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(creator, "scan");
}
