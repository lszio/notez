use crate::sync::merge::{HeadsTracker, MergeResult, ThreeWayMerger};
use tempfile::tempdir;

#[test]
fn test_three_way_merge_and_heads_tracking() {
    let temp = tempdir().unwrap();
    let heads = HeadsTracker::new(temp.path());

    heads.set_head("actor_a", "snap_100").unwrap();
    assert_eq!(
        heads.get_head("actor_a").unwrap().as_deref(),
        Some("snap_100")
    );

    let base = "line1\nline2\n";
    let mine = "line1\nline2\n";
    let theirs = "line1\nline2\nline3\n";

    let res_ff = ThreeWayMerger::merge("note.org", base, mine, theirs);
    match res_ff {
        MergeResult::Clean(merged) => assert_eq!(merged, theirs),
        other => panic!("expected clean fast-forward merge, got {other:?}"),
    }

    let base2 = "line1\nline2\nline3\n";
    let mine2 = "line1_edited\nline2\nline3\n";
    let theirs2 = "line1\nline2\nline3_edited\n";

    let res_clean = ThreeWayMerger::merge("note.org", base2, mine2, theirs2);
    match res_clean {
        MergeResult::Clean(merged) => {
            assert!(merged.contains("line1_edited"));
            assert!(merged.contains("line3_edited"));
        }
        other => panic!("expected clean 3-way merge, got {other:?}"),
    }

    let base3 = "line1\nline2\n";
    let mine3 = "line1_mine\nline2\n";
    let theirs3 = "line1_theirs\nline2\n";

    let res_conflict = ThreeWayMerger::merge("note.org", base3, mine3, theirs3);
    match res_conflict {
        MergeResult::Conflict { conflict } => {
            assert_eq!(conflict.logical_path, "note.org");
            assert!(conflict.conflict_text.contains("<<<<<<< mine"));
        }
        other => panic!("expected conflict, got {other:?}"),
    }
}
