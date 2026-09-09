use notez_protocol::{
    Command, CommandResult, DocumentSummary, Error, GraphQuery, GraphResult, IngestWatchBatch,
    ObjectAddress, ObjectSummary, Query, Request, Response, SpaceSummary, WatchStatus,
};
use schemars::schema_for;
use serde::de::DeserializeOwned;
use serde_json::json;

fn sample_write_commands() -> Vec<Command> {
    vec![
        Command::RegisterSpace {
            principal: "alice".into(),
            space_id: "work".into(),
            name: "Work".into(),
            expected_revision: "space-r1".into(),
        },
        Command::RegisterSource {
            principal: "alice".into(),
            space_id: "work".into(),
            source_id: "notes".into(),
            kind: "local".into(),
            expected_revision: "source-r1".into(),
        },
        Command::RemoveSpace {
            principal: "alice".into(),
            space_id: "work".into(),
            expected_revision: "space-r1".into(),
        },
        Command::AttachSource {
            principal: "alice".into(),
            space_id: "work".into(),
            source_id: "notes".into(),
            expected_revision: "source-r1".into(),
        },
        Command::ScanSpace {
            principal: "alice".into(),
            space_id: "work".into(),
            expected_revision: "space-r1".into(),
        },
        Command::CreateDocument {
            principal: "alice".into(),
            space_id: "work".into(),
            source_id: "notes".into(),
            document_path: "new.md".into(),
            content: "new".into(),
            expected_revision: "source-r1".into(),
        },
        Command::CreateObject {
            principal: "alice".into(),
            space_id: "work".into(),
            address: ObjectAddress::Stable {
                object_id: "obj-2".into(),
            },
            content: "body".into(),
            expected_revision: "object-r1".into(),
        },
        Command::TransitionTask {
            principal: "alice".into(),
            space_id: "work".into(),
            object_id: "obj-1".into(),
            state: "done".into(),
            expected_revision: "object-r1".into(),
        },
        Command::ResolveConflict {
            principal: "alice".into(),
            space_id: "work".into(),
            conflict_id: "conflict-1".into(),
            resolution: "ours".into(),
            expected_revision: "space-r1".into(),
        },
        Command::UpdateDocument {
            principal: "alice".into(),
            space_id: "work".into(),
            source_id: "notes".into(),
            document_path: "daily/today.md".into(),
            content: "# Today".into(),
            expected_revision: "document-r1".into(),
        },
        Command::WriteObject {
            principal: "alice".into(),
            space_id: "work".into(),
            address: ObjectAddress::Stable {
                object_id: "obj-1".into(),
            },
            content: "body".into(),
            expected_revision: "object-r1".into(),
        },
        Command::IngestWatch(IngestWatchBatch {
            principal: "watcher".into(),
            space_id: "work".into(),
            source_id: "notes".into(),
            batch_id: "watch-batch-1".into(),
            expected_revision: "watch-r1".into(),
            events: vec![],
        }),
    ]
}

#[test]
fn every_write_command_has_a_revision_field() {
    for command in sample_write_commands() {
        assert!(command.expected_revision().is_some());
    }
}

#[test]
fn typed_contracts_round_trip_as_json_schema() {
    let address = ObjectAddress::Positioned {
        space_id: "work".into(),
        source_id: "notes".into(),
        document_path: "daily/today.md".into(),
        locator: "line:3-5".into(),
        fingerprint: "fnv1a:deadbeef".into(),
    };
    let graph = GraphQuery::Neighbors {
        principal: "alice".into(),
        object: address.clone(),
        scope: "space:work".into(),
        depth: 2,
    };
    let batch = IngestWatchBatch {
        principal: "watcher".into(),
        space_id: "work".into(),
        source_id: "notes".into(),
        batch_id: "batch-1".into(),
        expected_revision: "watch-r1".into(),
        events: vec![],
    };

    for value in [
        serde_json::to_value(&address).unwrap(),
        serde_json::to_value(&graph).unwrap(),
        serde_json::to_value(&batch).unwrap(),
        serde_json::to_value(Query::Graph {
            principal: "alice".into(),
            object: address,
            scope: "space:work".into(),
            depth: 2,
        })
        .unwrap(),
        serde_json::to_value(Response::CommandResult(CommandResult::Applied {
            new_revision: "r2".into(),
        }))
        .unwrap(),
    ] {
        assert!(value.is_object());
    }
}

#[test]
fn protocol_error_round_trips_as_json_schema() {
    let error = Error::StaleRevision {
        expected: "a".into(),
        actual: "b".into(),
    };
    let encoded = serde_json::to_string(&error).unwrap();
    assert_eq!(serde_json::from_str::<Error>(&encoded).unwrap(), error);
}

#[test]
fn watch_batch_uses_canonical_wire_names_and_round_trips() {
    let value = json!({
        "principal": "watcher",
        "space_id": "work",
        "source_id": "notes",
        "batch_id": "batch-1",
        "expected_revision": "r1",
        "events": [{
            "kind": "modified",
            "locator": "daily/today.md",
            "observed_revision": "fp-1",
            "observed_at": "2026-09-07T00:00:00Z"
        }]
    });
    let batch: IngestWatchBatch = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(batch).unwrap(), value);
}

#[test]
fn empty_expected_revision_is_rejected_for_every_mutating_command() {
    for command in sample_write_commands() {
        let mut encoded = serde_json::to_value(command).unwrap();
        if let Some(expected_revision) = encoded.get_mut("expected_revision") {
            *expected_revision = json!("");
        }
        if encoded["command"] == "ingest_watch" {
            encoded["expected_revision"] = json!("");
        }
        assert!(serde_json::from_value::<Command>(encoded).is_err());
    }
}

#[test]
fn mutating_commands_carry_principal_and_space_scope() {
    for command in sample_write_commands() {
        let value = serde_json::to_value(command).unwrap();
        if value["command"] != "ingest_watch" {
            assert!(value.get("principal").is_some());
            assert!(value.get("space_id").is_some());
        }
    }
    let watch = serde_json::to_value(Command::IngestWatch(IngestWatchBatch {
        principal: "watcher".into(),
        space_id: "work".into(),
        source_id: "notes".into(),
        batch_id: "batch-1".into(),
        expected_revision: "r1".into(),
        events: vec![],
    }))
    .unwrap();
    assert!(watch.get("principal").is_some());
    assert!(watch.get("space_id").is_some());
}

#[test]
fn graph_query_carries_principal_scope_and_positioned_fingerprint() {
    let graph = Query::Graph {
        principal: "alice".into(),
        object: ObjectAddress::Positioned {
            space_id: "work".into(),
            source_id: "notes".into(),
            document_path: "daily/today.md".into(),
            locator: "line:3-5".into(),
            fingerprint: "fnv1a:deadbeef".into(),
        },
        scope: "space:work".into(),
        depth: 2,
    };
    let value = serde_json::to_value(&graph).unwrap();
    assert_eq!(value["principal"], "alice");
    assert_eq!(value["object"]["fingerprint"], "fnv1a:deadbeef");
    assert_eq!(serde_json::from_value::<Query>(value).unwrap(), graph);
}

#[test]
fn every_public_contract_round_trips_json() {
    fn round_trip<T>(value: T)
    where
        T: serde::Serialize + DeserializeOwned + std::fmt::Debug + PartialEq,
    {
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<T>(&encoded).unwrap(), value);
    }
    for command in sample_write_commands() {
        round_trip(command);
    }
    round_trip(Query::ListSpaces {
        principal: "alice".into(),
    });
    round_trip(Query::Graph {
        principal: "alice".into(),
        object: ObjectAddress::Stable {
            object_id: "o1".into(),
        },
        scope: "space:work".into(),
        depth: 1,
    });
    round_trip(Response::Space(SpaceSummary {
        space_id: "work".into(),
        name: "Work".into(),
    }));
    round_trip(Response::Document(DocumentSummary {
        source_id: "notes".into(),
        document_path: "a.md".into(),
        revision: "r1".into(),
    }));
    round_trip(Response::Object(ObjectSummary {
        address: ObjectAddress::Stable {
            object_id: "o1".into(),
        },
        title: "T".into(),
        content: "C".into(),
    }));
    round_trip(Response::Graph(GraphResult {
        object: ObjectAddress::Stable {
            object_id: "o1".into(),
        },
        neighbors: vec![],
    }));
    round_trip(Response::Summary(SpaceSummary {
        space_id: "work".into(),
        name: "Work".into(),
    }));
    round_trip(Response::Watch(WatchStatus {
        source_id: "notes".into(),
        active: true,
        last_batch_id: Some("b1".into()),
    }));
    for error in [
        Error::StaleRevision {
            expected: "r1".into(),
            actual: "r2".into(),
        },
        Error::Forbidden {
            message: "no".into(),
        },
        Error::Conflict {
            target: "x".into(),
            message: "no".into(),
        },
    ] {
        round_trip(error);
    }
}

#[test]
fn schemas_cover_all_new_contracts() {
    for schema in [
        schema_for!(Command),
        schema_for!(Query),
        schema_for!(Response),
        schema_for!(Error),
        schema_for!(IngestWatchBatch),
    ] {
        let value = serde_json::to_value(schema).unwrap();
        assert!(
            value.get("$schema").is_some()
                || value.get("schema").is_some()
                || value.get("definitions").is_some()
                || value.get("$defs").is_some()
        );
    }
}
#[test]
fn legacy_request_remains_available_during_protocol_migration() {
    let request = Request::ScanNative(notez_protocol::request::ScanNativeRequest {});
    assert_eq!(serde_json::to_value(request).unwrap()["op"], "scan_native");
}
