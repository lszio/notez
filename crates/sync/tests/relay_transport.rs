use sync::manifest::Manifest;
use sync::relay::{P2pTransport, RelayTransport, SyncEvent, SyncTransport, TransportRegistry};
use tempfile::tempdir;

#[test]
fn relay_transport_exchange_manifests_and_objects() {
    let temp = tempdir().unwrap();
    let mut relay = RelayTransport::new(temp.path());

    let manifest = Manifest {
        space_id: "space_relay".to_string(),
        actor_id: "actor_relay".to_string(),
        parent_snapshots: vec![],
        logical_path: "relay_note.org".to_string(),
        content_hash: "relay_hash_abc".to_string(),
        properties: Default::default(),
    };
    relay.push_manifest(&manifest).unwrap();

    let payload = b"Relay object payload bytes";
    relay.push_object(&manifest.content_hash, payload).unwrap();

    let pulled_manifest = relay.pull_manifest("relay_note.org").unwrap().unwrap();
    assert_eq!(pulled_manifest.content_hash, "relay_hash_abc");

    let pulled_obj = relay.pull_object(&manifest.content_hash).unwrap().unwrap();
    assert_eq!(pulled_obj, payload);

    let events = relay.drain_events();
    assert!(
        events.iter().any(
            |e| matches!(e, SyncEvent::ManifestPushed(m) if m.logical_path == "relay_note.org")
        )
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SyncEvent::ObjectPushed(h) if h == "relay_hash_abc"))
    );
}

#[test]
fn p2p_transport_provides_object_lookups() {
    let mut p2p = P2pTransport::new();
    p2p.push_object("k1", b"data1").unwrap();
    let v = p2p.pull_object("k1").unwrap();
    assert_eq!(v.as_deref(), Some(b"data1".as_ref()));
}

#[test]
fn transport_registry_tracks_named_transports() {
    let temp = tempdir().unwrap();
    let mut registry = TransportRegistry::new(temp.path());
    registry.register("relay", RelayTransport::new(temp.path()));
    registry.register("p2p", P2pTransport::new());
    assert_eq!(registry.transports.len(), 2);
    assert!(registry.transports.contains_key("relay"));
    assert!(registry.transports.contains_key("p2p"));
}
