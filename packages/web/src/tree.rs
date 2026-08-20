//! Tree and source-file aggregation built from the projection store.
//!
//! The web client renders a left column with two stacked panels:
//!
//! 1. A directory tree — one folder per intermediate path that
//!    appears in any resource's `locator`. Folders carry the count
//!    of resources that live under them; the leaves are themselves
//!    source/attachment resources.
//! 2. A flat "source files" list — every org/md/attachment file in
//!    the space, sorted by display path. The user uses this to
//!    preview an attachment without going through the index page.
//!
//! Both views are derived from a single `Selector::new()` query so
//! they always agree with the rest of the UI.

use std::collections::BTreeMap;
use std::path::Path;

use notez_core::application::ApplicationFacade;
use notez_core::domain::{Resource, ResourceKind, Selector};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub depth: usize,
    /// Resources that live directly under this folder (its own leaves
    /// plus everything under nested folders).
    pub count: usize,
    pub children: Vec<TreeChild>,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TreeChild {
    Folder(TreeNode),
    Leaf {
        name: String,
        path: String,
        kind: String,
        title: String,
        ref_str: String,
    },
}

/// Flat list of every source/attachment file in the space.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceFileRow {
    pub ref_str: String,
    pub kind: String,
    pub title: String,
    pub display_path: String,
    pub ext: String,
    pub size: u64,
}

/// One hit for the command-palette search.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SearchHit {
    pub ref_str: String,
    pub kind: String,
    pub title: String,
    pub locator: String,
    pub snippet: String,
    pub match_field: String,
}

/// `index.org`-style landing entry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndexEntryDto {
    pub ref_str: String,
    pub title: String,
    pub locator: String,
}

/// Aggregated counts of resource kinds in the space.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KindCounts {
    pub document: usize,
    pub heading: usize,
    pub attachment: usize,
    pub block: usize,
}

impl KindCounts {
    pub fn total(&self) -> usize {
        self.document + self.heading + self.attachment + self.block
    }
}

// ---- public entry points ----

pub fn build_tree<S>(
    facade: &ApplicationFacade<S>,
) -> Result<TreeNode, notez_core::application::ApplicationError>
where
    S: notez_core::domain::ProjectionStore,
{
    let page = facade.query(&Selector::new())?;
    Ok(build_tree_from(&page.items))
}
/// Build a tree that merges the indexed resources with the loose
/// files on disk. Loose files get an empty `ref_str`; the panel
/// renders them via the preview route.
pub fn build_tree_with_disk<S>(
    facade: &ApplicationFacade<S>,
    space_root: &Path,
) -> Result<TreeNode, notez_core::application::ApplicationError>
where
    S: notez_core::domain::ProjectionStore,
{
    let page = facade.query(&Selector::new())?;
    let loose = build_filesystem_listing(space_root);
    Ok(build_tree_from_with_disk(&page.items, &loose))
}

pub fn build_source_files<S>(
    facade: &ApplicationFacade<S>,
    space_root: &Path,
) -> Result<Vec<SourceFileRow>, notez_core::application::ApplicationError>
where
    S: notez_core::domain::ProjectionStore,
{
    let page = facade.query(&Selector::new())?;
    Ok(build_source_files_from(&page.items, space_root))
}

pub fn build_kind_counts<S>(
    facade: &ApplicationFacade<S>,
) -> Result<KindCounts, notez_core::application::ApplicationError>
where
    S: notez_core::domain::ProjectionStore,
{
    let page = facade.query(&Selector::new())?;
    Ok(build_kind_counts_from(&page.items))
}

pub fn build_index_entry<S>(
    facade: &ApplicationFacade<S>,
) -> Result<Option<IndexEntryDto>, notez_core::application::ApplicationError>
where
    S: notez_core::domain::ProjectionStore,
{
    let page = facade.query(&Selector::new())?;
    Ok(build_index_entry_from(&page.items))
}

pub fn build_search<S>(
    facade: &ApplicationFacade<S>,
    space_root: &Path,
    q: &str,
) -> Result<Vec<SearchHit>, notez_core::application::ApplicationError>
where
    S: notez_core::domain::ProjectionStore,
{
    let page = facade.query(&Selector::new())?;
    Ok(build_search_from(&page.items, space_root, q))
}
/// Walk `space_root` on disk and return every non-hidden file as
/// a `SourceFileRow`. This is the "everything on disk" view the
/// files panel and the per-space home page show; it surfaces
/// loose attachments the projection has not indexed yet.
pub fn build_filesystem_listing(space_root: &Path) -> Vec<SourceFileRow> {
    const MAX_FILES: usize = 2000;
    let mut out: Vec<SourceFileRow> = Vec::new();
    walk_dir(space_root, space_root, &mut out);
    out.sort_by(|a, b| a.display_path.to_lowercase().cmp(&b.display_path.to_lowercase()));
    if out.len() > MAX_FILES {
        out.truncate(MAX_FILES);
    }
    out
}

fn walk_dir(root: &Path, dir: &Path, out: &mut Vec<SourceFileRow>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            walk_dir(root, &path, out);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let rel = match path.strip_prefix(root) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push(SourceFileRow {
            ref_str: String::new(),
            kind: "attachment".to_string(),
            title: name_str.into_owned(),
            display_path: rel,
            ext,
            size,
        });
    }
}

// ---- pure projections from Resource slices ----

/// Same as `build_tree_from` but also adds the loose files on
/// disk (from `build_filesystem_listing`) as leaves, deduplicating
/// against the indexed set. Indexed files keep their `ref_str`;
/// loose files get an empty `ref_str` (the panel renders them via
/// the preview route).
pub fn build_tree_from_with_disk(resources: &[Resource], loose: &[SourceFileRow]) -> TreeNode {
    let loose_set: std::collections::HashSet<String> = loose.iter().map(|f| f.display_path.clone()).collect();
    // Filter indexed resources to only those that have a real file
    // on disk (avoids showing deleted files in the tree).
    let mut resources_vec: Vec<Resource> = resources
        .iter()
        .filter(|r| {
            r.kind != ResourceKind::Heading
                && r.kind != ResourceKind::Block
                && !r.locator.is_empty()
                && loose_set.contains(&r.locator)
        })
        .cloned()
        .collect();
    // Add loose files that aren't already indexed.
    for f in loose {
        if !loose_set.contains(&f.display_path) {
            // already filtered above, but keep the dedup logic safe
        }
    }
    // Build tree from the filtered indexed set first.
    let mut tree = build_tree_from(&resources_vec);
    // Add loose-file leaves that don't have an indexed counterpart.
    let indexed_locators: std::collections::HashSet<String> =
        resources_vec.iter().map(|r| r.locator.clone()).collect();
    let mut by_folder: BTreeMap<String, Vec<LeafRaw>> = BTreeMap::new();
    for f in loose {
        if indexed_locators.contains(&f.display_path) {
            continue;
        }
        let folder = parent_folder(&f.display_path);
        by_folder.entry(folder).or_default().push(LeafRaw {
            name: f.title.clone(),
            path: f.display_path.clone(),
            kind: f.kind.clone(),
            title: f.title.clone(),
            ref_str: f.ref_str.clone(),
        });
    }
    if !by_folder.is_empty() {
        // Walk the tree and inject the loose-file leaves under the
        // right folder, creating folders as needed.
        inject_loose(&mut tree, &by_folder);
    }
    let _ = resources_vec;
    tree
}

fn inject_loose(tree: &mut TreeNode, leaves: &BTreeMap<String, Vec<LeafRaw>>) {
    for (folder_path, leafs) in leaves.iter() {
        ensure_folder_with_leaves(tree, folder_path, leafs);
    }
    // Re-sort children after injection.
    tree.children.sort_by(|a, b| match (a, b) {
        (TreeChild::Folder(_), TreeChild::Leaf { .. }) => std::cmp::Ordering::Less,
        (TreeChild::Leaf { .. }, TreeChild::Folder(_)) => std::cmp::Ordering::Greater,
        (TreeChild::Folder(a), TreeChild::Folder(b)) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        (TreeChild::Leaf { name: a, .. }, TreeChild::Leaf { name: b, .. }) => a.to_lowercase().cmp(&b.to_lowercase()),
    });
    // Recount.
    let _ = propagate_count(tree);
}

fn ensure_folder_with_leaves(
    tree: &mut TreeNode,
    folder_path: &str,
    leafs: &[LeafRaw],
) {
    if folder_path.is_empty() {
        for l in leafs {
            tree.children.push(TreeChild::Leaf {
                name: l.name.clone(),
                path: l.path.clone(),
                kind: l.kind.clone(),
                title: l.title.clone(),
                ref_str: l.ref_str.clone(),
            });
        }
        return;
    }
    let parts: Vec<&str> = folder_path.split('/').collect();
    let mut current_path = String::new();
    let mut cursor = tree;
    for (i, part) in parts.iter().enumerate() {
        current_path = if i == 0 {
            part.to_string()
        } else {
            format!("{}/{}", current_path, part)
        };
        let pos = cursor.children.iter().position(|c| match c {
            TreeChild::Folder(f) => f.path == current_path || f.name == *part,
            _ => false,
        });
        let idx = match pos {
            Some(i) => i,
            None => {
                cursor.children.push(TreeChild::Folder(TreeNode {
                    name: part.to_string(),
                    path: current_path.clone(),
                    depth: i + 1,
                    count: 0,
                    children: Vec::new(),
                }));
                cursor.children.len() - 1
            }
        };
        if let TreeChild::Folder(f) = &mut cursor.children[idx] {
            cursor = f;
        } else {
            return;
        }
    }
    for l in leafs {
        cursor.children.push(TreeChild::Leaf {
            name: l.name.clone(),
            path: l.path.clone(),
            kind: l.kind.clone(),
            title: l.title.clone(),
            ref_str: l.ref_str.clone(),
        });
    }
}
pub fn build_tree_from(resources: &[Resource]) -> TreeNode {
    // 1. Bucket leaves under their parent folder.
    let mut leaves: BTreeMap<String, Vec<LeafRaw>> = BTreeMap::new();
    let mut folder_paths: BTreeMap<String, usize> = BTreeMap::new();
    folder_paths.insert(String::new(), 0);

    for res in resources {
        // Skip headings: they belong to their parent document and are not
        // separate files. They still count toward the folder's total.
        let folder_path = parent_folder(&res.locator);
        *folder_paths.entry(folder_path.clone()).or_insert(0) += 1;
        // Ensure every ancestor folder exists in the folder set with at
        // least its current count.
        for ancestor in ancestor_paths(&folder_path) {
            folder_paths.entry(ancestor).or_insert(0);
        }
        if res.kind == ResourceKind::Heading || res.kind == ResourceKind::Block {
            continue;
        }
        leaves.entry(folder_path).or_default().push(LeafRaw {
            name: file_name_of(&res.locator),
            path: res.locator.clone(),
            kind: res.kind.as_str().to_string(),
            title: res.title.clone(),
            ref_str: res.r#ref.to_string(),
        });
    }

    // 2. Build the tree bottom-up by walking folder paths in depth order
    //    (deepest first). Each folder collects leaves attached to itself
    //    and any subfolders built previously.
    let mut folders: BTreeMap<String, TreeNode> = BTreeMap::new();
    // Pre-populate empty nodes for every folder so children can resolve.
    for path in folder_paths.keys() {
        folders.insert(
            path.clone(),
            TreeNode {
                name: if path.is_empty() { "(root)".into() } else { leaf_name(path) },
                path: path.clone(),
                depth: depth_of(path),
                count: 0,
                children: Vec::new(),
            },
        );
    }

    // Walk from deepest to shallowest so we attach leaves / subfolders
    // before their parents consume them.
    let mut paths: Vec<String> = folders.keys().cloned().collect();
    paths.sort_by(|a, b| b.len().cmp(&a.len()));
    for p in paths.iter() {
        if let Some(folder_leaves) = leaves.remove(p) {
            let node = folders.get_mut(p).expect("folder exists");
            for l in folder_leaves {
                node.children.push(TreeChild::Leaf {
                    name: l.name,
                    path: l.path,
                    kind: l.kind,
                    title: l.title,
                    ref_str: l.ref_str,
                });
            }
            // Sort leaves: directories before files, then alphabetical.
            node.children.sort_by(|a, b| match (a, b) {
                (TreeChild::Folder(_), TreeChild::Leaf { .. }) => std::cmp::Ordering::Less,
                (TreeChild::Leaf { .. }, TreeChild::Folder(_)) => std::cmp::Ordering::Greater,
                (TreeChild::Folder(a), TreeChild::Folder(b)) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                (TreeChild::Leaf { name: a, .. }, TreeChild::Leaf { name: b, .. }) => a.to_lowercase().cmp(&b.to_lowercase()),
            });
        }
    }

    // 3. Walk top-down, attaching subfolders as children of their parents
    //    and propagating counts.
    let mut root = folders.remove("").unwrap_or(TreeNode {
        name: "(root)".into(),
        path: String::new(),
        depth: 0,
        count: 0,
        children: Vec::new(),
    });

    // Sort folders globally so we attach parents first.
    let mut remaining: Vec<String> = folders.keys().cloned().collect();
    remaining.sort_by(|a, b| b.len().cmp(&a.len()));
    for folder_path in remaining {
        let parent_path = parent_folder(&folder_path);
        let folder_node = folders.remove(&folder_path).unwrap();
        let node = if parent_path.is_empty() {
            &mut root
        } else {
            folders.get_mut(&parent_path).expect("parent exists")
        };
        node.children.push(TreeChild::Folder(folder_node));
    }
    // Sort root children the same way.
    root.children.sort_by(|a, b| match (a, b) {
        (TreeChild::Folder(_), TreeChild::Leaf { .. }) => std::cmp::Ordering::Less,
        (TreeChild::Leaf { .. }, TreeChild::Folder(_)) => std::cmp::Ordering::Greater,
        (TreeChild::Folder(a), TreeChild::Folder(b)) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        (TreeChild::Leaf { name: a, .. }, TreeChild::Leaf { name: b, .. }) => a.to_lowercase().cmp(&b.to_lowercase()),
    });

    // 4. Recompute counts bottom-up.
    propagate_count(&mut root);
    root
}

fn propagate_count(node: &mut TreeNode) -> usize {
    let mut total = 0;
    for child in node.children.iter_mut() {
        match child {
            TreeChild::Folder(f) => {
                total += propagate_count(f);
            }
            TreeChild::Leaf { .. } => total += 1,
        }
    }
    node.count = total;
    total
}

#[derive(Clone)]
struct LeafRaw {
    name: String,
    path: String,
    kind: String,
    title: String,
    ref_str: String,
}

pub fn build_source_files_from(resources: &[Resource], space_root: &Path) -> Vec<SourceFileRow> {
    let mut out: Vec<SourceFileRow> = resources
        .iter()
        .filter(|r| r.kind != ResourceKind::Heading && r.kind != ResourceKind::Block)
        .map(|r| {
            let ext = std::path::Path::new(&r.locator)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let size = space_root.join(&r.locator).metadata().map(|m| m.len()).unwrap_or(0);
            SourceFileRow {
                ref_str: r.r#ref.to_string(),
                kind: r.kind.as_str().to_string(),
                title: r.title.clone(),
                display_path: r.locator.clone(),
                ext,
                size,
            }
        })
        .collect();
    out.sort_by(|a, b| a.display_path.to_lowercase().cmp(&b.display_path.to_lowercase()));
    out
}

pub fn build_kind_counts_from(resources: &[Resource]) -> KindCounts {
    let mut c = KindCounts::default();
    for r in resources {
        match r.kind {
            ResourceKind::Document => c.document += 1,
            ResourceKind::Heading => c.heading += 1,
            ResourceKind::Attachment => c.attachment += 1,
            ResourceKind::Block => c.block += 1,
        }
    }
    c
}

pub fn build_index_entry_from(resources: &[Resource]) -> Option<IndexEntryDto> {
    let mut candidates: Vec<&Resource> = resources
        .iter()
        .filter(|r| r.kind == ResourceKind::Document)
        .collect();
    let match_name = |r: &&Resource, name: &str| {
        std::path::Path::new(&r.locator)
            .file_name()
            .and_then(|s| s.to_str())
            == Some(name)
    };
    let order = ["index.org", "index.md", "README.org", "README.md"];
    for name in order {
        if let Some(r) = candidates.iter().find(|r| match_name(r, name)) {
            return Some(IndexEntryDto {
                ref_str: r.r#ref.to_string(),
                title: r.title.clone(),
                locator: r.locator.clone(),
            });
        }
    }
    // Fallback: any document at root whose title looks like an index.
    for r in candidates.drain(..) {
        let title = r.title.to_lowercase();
        if title.starts_with("index") || title.starts_with("readme") {
            return Some(IndexEntryDto {
                ref_str: r.r#ref.to_string(),
                title: r.title.clone(),
                locator: r.locator.clone(),
            });
        }
    }
    None
}

pub fn build_search_from(resources: &[Resource], space_root: &Path, q: &str) -> Vec<SearchHit> {
    const MAX_FILE_BYTES: u64 = 256 * 1024;
    const MAX_HITS: usize = 80;
    const SNIPPET_CHARS: usize = 80;

    let terms: Vec<String> = q
        .split_whitespace()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    if terms.is_empty() {
        return Vec::new();
    }

    let mut hits: Vec<SearchHit> = Vec::new();
    for r in resources.iter() {
        if hits.len() >= MAX_HITS {
            break;
        }
        let title_lc = r.title.to_lowercase();
        let locator_lc = r.locator.to_lowercase();
        let title_hit = terms.iter().any(|t| title_lc.contains(t));
        let locator_hit = terms.iter().any(|t| locator_lc.contains(t));
        if title_hit || locator_hit {
            hits.push(SearchHit {
                ref_str: r.r#ref.to_string(),
                kind: r.kind.as_str().to_string(),
                title: r.title.clone(),
                locator: r.locator.clone(),
                snippet: snippet_at(&r.locator, locator_lc.find(&terms[0]).unwrap_or(0), SNIPPET_CHARS),
                match_field: if title_hit { "title".into() } else { "filename".into() },
            });
            continue;
        }
        // Body search: only for files we can read.
        let file_path = space_root.join(&r.locator);
        let meta = match std::fs::metadata(&file_path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.len() > MAX_FILE_BYTES || meta.len() == 0 {
            continue;
        }
        let bytes = match std::fs::read(&file_path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let body = match std::str::from_utf8(&bytes) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let body_lc = body.to_lowercase();
        if !terms.iter().all(|t| body_lc.contains(t)) {
            continue;
        }
        let first = terms
            .iter()
            .filter_map(|t| body_lc.find(t))
            .min()
            .unwrap_or(0);
        hits.push(SearchHit {
            ref_str: r.r#ref.to_string(),
            kind: r.kind.as_str().to_string(),
            title: r.title.clone(),
            locator: r.locator.clone(),
            snippet: snippet_at(body, first, SNIPPET_CHARS),
            match_field: "body".into(),
        });
    }

    hits
}

fn snippet_at(body: &str, byte_offset: usize, chars: usize) -> String {
    let start = body[..byte_offset]
        .char_indices()
        .rev()
        .nth(chars / 2)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let end = body[byte_offset..]
        .char_indices()
        .nth(chars / 2)
        .map(|(i, _)| byte_offset + i)
        .unwrap_or(body.len());
    let mut s: String = body[start..end].chars().take(chars).collect();
    if start > 0 {
        s = format!("…{s}");
    }
    if end < body.len() {
        s.push('…');
    }
    s
}

fn parent_folder(locator: &str) -> String {
    parent_folder_opt(locator).unwrap_or_default()
}

fn parent_folder_opt(locator: &str) -> Option<String> {
    let p = std::path::Path::new(locator);
    p.parent().and_then(|p| p.to_str()).map(|s| s.replace('\\', "/"))
}

fn ancestor_paths(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = parent_folder_opt(path);
    while let Some(p) = cur {
        if !p.is_empty() {
            out.push(p.clone());
        }
        cur = parent_folder_opt(&p);
    }
    out
}

fn leaf_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

fn file_name_of(locator: &str) -> String {
    leaf_name(locator)
}

fn depth_of(path: &str) -> usize {
    if path.is_empty() {
        0
    } else {
        path.matches('/').count() + path.matches('\\').count() + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notez_core::domain::{ObjectId, Resource, ResourceRef};
    use ulid::Ulid;

    fn make_doc(locator: &str, title: &str) -> Resource {
        Resource {
            r#ref: ResourceRef::new(ResourceKind::Document, Ulid::new()),
            kind: ResourceKind::Document,
            title: title.into(),
            revision: "r1".into(),
            source_id: "native".into(),
            locator: locator.into(),
            properties: BTreeMap::new(),
            object_id: ObjectId::new(Ulid::new()),
        }
    }

    #[test]
    fn tree_groups_by_folder() {
        let resources = vec![
            make_doc("docs/a.org", "A"),
            make_doc("docs/sub/b.md", "B"),
            make_doc("c.org", "C"),
        ];
        let tree = build_tree_from(&resources);
        assert_eq!(tree.count, 3);
        // Root has at least one folder and one leaf.
        assert!(tree.children.iter().any(|c| matches!(c, TreeChild::Folder(_))));
        assert!(tree.children.iter().any(|c| matches!(c, TreeChild::Leaf { .. })));
    }

    #[test]
    fn tree_folder_count_includes_nested() {
        let resources = vec![
            make_doc("a/b/c.org", "C"),
            make_doc("a/d.org", "D"),
        ];
        let tree = build_tree_from(&resources);
        assert_eq!(tree.count, 2);
    }

    #[test]
    fn index_entry_prefers_index_org() {
        let resources = vec![
            make_doc("README.org", "README"),
            make_doc("index.org", "Index"),
        ];
        let entry = build_index_entry_from(&resources).expect("entry");
        assert_eq!(entry.locator, "index.org");
    }

    #[test]
    fn search_finds_body_match() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("note.org");
        std::fs::write(&file, "* heading\n\nbody mentioning needle here\n").unwrap();
        let r = make_doc("note.org", "Note");
        let hits = build_search_from(&[r], tmp.path(), "needle");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].match_field, "body");
    }

    #[test]
    fn search_finds_title_match() {
        let r = make_doc("foo.org", "My needle title");
        let hits = build_search_from(&[r], Path::new("/tmp"), "needle");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].match_field, "title");
    }
}