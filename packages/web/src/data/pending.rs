//! One-shot stash for a failed save.
//!
//! A form POST must not lose the user's text. Instead of re-rendering
//! the editor from the handler (the editor is a Dioxus page), the
//! failed submission is stashed under a random token and the browser
//! is redirected back to the editor with `?restore=<token>`; the page
//! reads the entry once and pre-fills the textarea with what the user
//! typed.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// A failed save, kept until the redirected editor page consumes it.
#[derive(Debug, Clone)]
pub struct PendingSave {
    pub locator: String,
    pub content: String,
    /// `stale` | `error`
    pub kind: String,
    pub message: String,
    /// Revision to send on the next save (the on-disk one for a
    /// conflict, so a second attempt is a deliberate overwrite).
    pub revision: String,
}

static PENDING: LazyLock<Mutex<HashMap<String, PendingSave>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Cap the stash so a runaway client cannot grow it without bound.
const MAX_PENDING: usize = 64;

pub fn stash(save: PendingSave) -> String {
    let token = ulid::Ulid::new().to_string();
    let mut map = PENDING.lock().expect("pending-save stash poisoned");
    if map.len() >= MAX_PENDING {
        map.clear();
    }
    map.insert(token.clone(), save);
    token
}

/// Take (and remove) the entry for `token`.
pub fn take(token: &str) -> Option<PendingSave> {
    PENDING
        .lock()
        .expect("pending-save stash poisoned")
        .remove(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stash_is_one_shot() {
        let token = stash(PendingSave {
            locator: "a.md".into(),
            content: "hello".into(),
            kind: "stale".into(),
            message: "changed".into(),
            revision: "r2".into(),
        });
        let first = take(&token).expect("first take");
        assert_eq!(first.content, "hello");
        assert!(take(&token).is_none(), "second take must be empty");
    }
}
