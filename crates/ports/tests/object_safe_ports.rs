use notez_ports::{
    AuditLog, BlobStore, ChangeJournal, Clock, ProjectionReader, ProjectionWriter, SourceReader,
    SourceWriter, WatchIngestion,
};

fn assert_object_safe<T: ?Sized>() {}

#[test]
fn infrastructure_ports_are_object_safe() {
    assert_object_safe::<dyn SourceReader>();
    assert_object_safe::<dyn SourceWriter>();
    assert_object_safe::<dyn ProjectionReader>();
    assert_object_safe::<dyn ProjectionWriter>();
    assert_object_safe::<dyn ChangeJournal>();
    assert_object_safe::<dyn AuditLog>();
    assert_object_safe::<dyn WatchIngestion>();
    assert_object_safe::<dyn BlobStore>();
    assert_object_safe::<dyn Clock>();
    let _: Box<dyn Clock> = Box::new(notez_ports::SystemClock);
}

#[test]
fn source_write_outcomes_keep_concurrency_explicit() {
    use notez_ports::SourceWriteOutcome;

    assert!(matches!(
        SourceWriteOutcome::Applied {
            new_revision: "next".into()
        },
        SourceWriteOutcome::Applied { new_revision } if new_revision == "next"
    ));
    assert!(matches!(
        SourceWriteOutcome::Stale {
            current_revision: "current".into()
        },
        SourceWriteOutcome::Stale { current_revision } if current_revision == "current"
    ));
    assert!(matches!(
        SourceWriteOutcome::Unsupported,
        SourceWriteOutcome::Unsupported
    ));
}
