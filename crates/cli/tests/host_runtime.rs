use std::sync::Arc;

use notez_cli::host::host_runtime;
use notez_core::application::WatchService;

#[test]
fn host_runtime_reuses_one_watcher_across_runtime_clones() {
    let watch = WatchService::new();
    let runtime = host_runtime(watch.clone());
    let clone = runtime.clone();

    assert!(Arc::ptr_eq(&watch, &runtime.watch()));
    assert!(Arc::ptr_eq(&runtime.watch(), &clone.watch()));
}
