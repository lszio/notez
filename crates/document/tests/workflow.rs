use document::workflow::{TodoStateKind, WorkflowProfile};

#[test]
fn parse_workflow_profile_and_apply_transition() {
    let spec = "TODO(t) NEXT(n) PEND(p) WAIT(w@/!) | DONE(d!) QUIT(q@)";
    let profile = WorkflowProfile::parse(spec).unwrap();

    assert_eq!(profile.active_states.len(), 4);
    assert_eq!(profile.done_states.len(), 2);

    let next_state = profile.find_state("NEXT").unwrap();
    assert_eq!(next_state.kind, TodoStateKind::Active);

    let done_state = profile.find_state("DONE").unwrap();
    assert_eq!(done_state.kind, TodoStateKind::Done);

    let transition = profile
        .transition("NEXT", "DONE", "2026-07-22 Wed 14:00")
        .unwrap();

    assert_eq!(transition.to_state, "DONE");
    assert_eq!(
        transition.closed_timestamp,
        Some("2026-07-22 Wed 14:00".to_string())
    );
    assert!(transition
        .logbook_entry
        .contains("- State \"DONE\"       from \"NEXT\""));
}
