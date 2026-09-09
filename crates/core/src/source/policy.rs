//! `SourcePolicy` — the single decision surface for "which files
//! belong to this Source".
//!
//! Built from the `[scan]` section of `notez.toml`
//! ([`crate::config::model::ScanConfig`]), one policy instance is
//! consulted by every entry point that touches source files: the
//! native scanner, the web files panel, attachment previews, and
//! dynamic-block context building. Filter rules must never be
//! re-implemented per surface.
//!
//! Decision order (first match wins):
//!
//! 1. any hidden path component → [`IgnoreReason::Hidden`];
//! 2. symlink while `follow_symlinks = false` → [`IgnoreReason::Symlink`];
//! 3. file larger than `max_file_size` → [`IgnoreReason::TooLarge`];
//! 4. matches an `exclude` glob → [`IgnoreReason::Excluded`];
//! 5. does not match any `include` glob → [`IgnoreReason::NotIncluded`];
//! 6. otherwise allowed.
//!
//! Directory pruning uses [`SourcePolicy::hides_dir`], which applies
//! only the structural rules (hidden/excluded/symlink) — `include`
//! matches files, so it must never prune a directory.

use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Path;

use crate::config::model::ScanConfig;

/// Why a path was rejected by the policy. Stable machine names are
/// part of the scan report vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IgnoreReason {
    /// Any path component starts with `.`.
    Hidden,
    /// Matches an `exclude` glob (or a legacy `exclude_paths` prefix).
    Excluded,
    /// Matches no `include` glob.
    NotIncluded,
    /// Larger than `max_file_size`.
    TooLarge,
    /// Symlink while `follow_symlinks` is off.
    Symlink,
}

impl IgnoreReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            IgnoreReason::Hidden => "hidden",
            IgnoreReason::Excluded => "excluded",
            IgnoreReason::NotIncluded => "not_included",
            IgnoreReason::TooLarge => "too_large",
            IgnoreReason::Symlink => "symlink",
        }
    }
}

/// Aggregate ignore counts for one scan pass, keyed by reason. Part
/// of the scan report so "file did not show up" is explainable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IgnoreCounts {
    pub hidden: u32,
    pub excluded: u32,
    pub not_included: u32,
    pub too_large: u32,
    pub symlink: u32,
}

impl IgnoreCounts {
    /// Record one rejection.
    pub fn record(&mut self, reason: IgnoreReason) {
        match reason {
            IgnoreReason::Hidden => self.hidden += 1,
            IgnoreReason::Excluded => self.excluded += 1,
            IgnoreReason::NotIncluded => self.not_included += 1,
            IgnoreReason::TooLarge => self.too_large += 1,
            IgnoreReason::Symlink => self.symlink += 1,
        }
    }

    /// Total rejections across all reasons.
    pub fn total(&self) -> u32 {
        self.hidden + self.excluded + self.not_included + self.too_large + self.symlink
    }
}

/// Outcome of a policy decision for one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Ignore(IgnoreReason),
}

/// Compiled scan policy for one Source.
#[derive(Debug, Clone)]
pub struct SourcePolicy {
    include: GlobSet,
    exclude: GlobSet,
    /// Legacy absolute-path excludes (`SourceConfig::exclude_paths`),
    /// matched as prefixes for backwards compatibility.
    legacy_exclude_prefixes: Vec<std::path::PathBuf>,
    follow_symlinks: bool,
    max_file_size: u64,
}

impl SourcePolicy {
    /// Whether the policy follows symlinks (walk configuration).
    pub fn follow_symlinks(&self) -> bool {
        self.follow_symlinks
    }

    /// Compile a policy from a `[scan]` config plus the legacy
    /// `exclude_paths` prefixes. Invalid glob patterns fall back to
    /// the built-in default for that list (config validation should
    /// reject them earlier; the scanner must stay usable).
    pub fn from_parts(scan: &ScanConfig, legacy_exclude_prefixes: Vec<std::path::PathBuf>) -> Self {
        let include = compile_globs(&scan.include).unwrap_or_else(|| {
            compile_globs(&ScanConfig::default().include).expect("default include globs compile")
        });
        let exclude = compile_globs(&scan.exclude).unwrap_or_else(|| {
            compile_globs(&ScanConfig::default().exclude).expect("default exclude globs compile")
        });
        Self {
            include,
            exclude,
            legacy_exclude_prefixes,
            follow_symlinks: scan.follow_symlinks,
            max_file_size: scan.max_file_size,
        }
    }

    /// Policy for a root without a parsed config: the built-in
    /// defaults (standard note formats, VCS/build/dependency/source
    /// exclusions).
    pub fn default_policy() -> Self {
        Self::from_parts(&ScanConfig::default(), Vec::new())
    }

    /// Policy for a root directory: parse `<root>/notez.toml` when it
    /// exists and use its `[scan]` section; any missing file or parse
    /// failure falls back to the built-in defaults (a corrupt config
    /// must not take the whole surface down).
    pub fn load_for_root(root: &Path) -> Self {
        let notez_toml = root.join("notez.toml");
        let Ok(text) = std::fs::read_to_string(&notez_toml) else {
            return Self::default_policy();
        };
        match crate::config::model::SourceConfig::parse(&text) {
            Ok(config) => Self::from_parts(&config.scan, Vec::new()),
            Err(_) => Self::default_policy(),
        }
    }

    /// Structural rules only, used to prune directory subtrees:
    /// hidden directories and excluded prefixes are skipped entirely,
    /// symlinks are skipped unless following. `include` is a
    /// file-level rule and must never prune directories.
    pub fn hides_dir(&self, rel_dir: &Path, is_symlink: bool) -> bool {
        if is_hidden(rel_dir) {
            return true;
        }
        if is_symlink && !self.follow_symlinks {
            return true;
        }
        self.legacy_prefix_hit(rel_dir)
    }

    /// Full decision for one file. `rel` is the source-relative path
    /// with `/` separators (the locator form).
    pub fn decide(&self, rel: &Path, is_symlink: bool, size: u64) -> Decision {
        if let Some(ignore) = self.structural_decision(rel, is_symlink) {
            return ignore;
        }
        if size > self.max_file_size {
            return Decision::Ignore(IgnoreReason::TooLarge);
        }
        let candidate = rel.to_string_lossy().replace('\\', "/");
        if !self.include.is_match(&candidate) {
            return Decision::Ignore(IgnoreReason::NotIncluded);
        }
        Decision::Allow
    }

    /// Decision for a file that belongs to the Source whether or not
    /// it is indexed. Identical to [`Self::decide`] except that the
    /// `include` globs and the size cap are not applied: they select
    /// the *documents* the indexer parses, while attachments (PDF,
    /// images, Office, archives…) are Source files too and must be
    /// listable and previewable. Callers that read a file apply their
    /// own size limit.
    pub fn decide_file(&self, rel: &Path, is_symlink: bool) -> Decision {
        self.structural_decision(rel, is_symlink)
            .unwrap_or(Decision::Allow)
    }

    /// Hidden / symlink / exclude rules shared by every decision.
    fn structural_decision(&self, rel: &Path, is_symlink: bool) -> Option<Decision> {
        if is_hidden(rel) {
            return Some(Decision::Ignore(IgnoreReason::Hidden));
        }
        if is_symlink && !self.follow_symlinks {
            return Some(Decision::Ignore(IgnoreReason::Symlink));
        }
        if self.legacy_prefix_hit(rel) {
            return Some(Decision::Ignore(IgnoreReason::Excluded));
        }
        let candidate = rel.to_string_lossy().replace('\\', "/");
        if self.exclude.is_match(&candidate) {
            return Some(Decision::Ignore(IgnoreReason::Excluded));
        }
        None
    }

    fn legacy_prefix_hit(&self, rel: &Path) -> bool {
        self.legacy_exclude_prefixes.iter().any(|prefix| rel.starts_with(prefix))
    }
}

fn is_hidden(rel: &Path) -> bool {
    rel.components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
}

fn compile_globs(patterns: &[String]) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        match Glob::new(pattern) {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(_) => return None,
        }
    }
    builder.build().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn policy(scan: ScanConfig) -> SourcePolicy {
        SourcePolicy::from_parts(&scan, Vec::new())
    }

    #[test]
    fn default_policy_accepts_notes_and_rejects_code_and_build() {
        let p = SourcePolicy::default_policy();
        assert_eq!(
            p.decide(Path::new("notes/hello.md"), false, 10),
            Decision::Allow
        );
        assert_eq!(
            p.decide(Path::new("docs/deep/note.org"), false, 10),
            Decision::Allow
        );
        assert_eq!(
            p.decide(Path::new("src/main.rs"), false, 10),
            Decision::Ignore(IgnoreReason::Excluded)
        );
        assert_eq!(
            p.decide(Path::new("target/debug/lib.rlib"), false, 10),
            Decision::Ignore(IgnoreReason::Excluded)
        );
        assert_eq!(
            p.decide(Path::new("node_modules/pkg/index.js"), false, 10),
            Decision::Ignore(IgnoreReason::Excluded)
        );
        assert_eq!(
            p.decide(Path::new("notes/script.ts"), false, 10),
            Decision::Ignore(IgnoreReason::Excluded)
        );
        // A data file that is neither excluded nor included.
        assert_eq!(
            p.decide(Path::new("data/dump.bin"), false, 10),
            Decision::Ignore(IgnoreReason::NotIncluded)
        );
    }

    #[test]
    fn decide_file_keeps_attachments_but_applies_structural_rules() {
        let p = SourcePolicy::default_policy();
        // Not a document, so the indexer skips it…
        assert_eq!(
            p.decide(Path::new("assets/diagram.png"), false, 10),
            Decision::Ignore(IgnoreReason::NotIncluded)
        );
        // …but it is still a Source file the UI must list and preview.
        assert_eq!(p.decide_file(Path::new("assets/diagram.png"), false), Decision::Allow);
        assert_eq!(
            p.decide_file(Path::new("参考材料/样张.pdf"), false),
            Decision::Allow
        );
        // Structural rules still apply.
        assert_eq!(
            p.decide_file(Path::new(".git/config"), false),
            Decision::Ignore(IgnoreReason::Hidden)
        );
        assert_eq!(
            p.decide_file(Path::new("target/debug/web"), false),
            Decision::Ignore(IgnoreReason::Excluded)
        );
        assert_eq!(
            p.decide_file(Path::new("link.bin"), true),
            Decision::Ignore(IgnoreReason::Symlink)
        );
    }

    #[test]
    fn hidden_paths_are_rejected_and_pruned() {
        let p = SourcePolicy::default_policy();
        assert_eq!(
            p.decide(Path::new(".notez/index.sqlite"), false, 10),
            Decision::Ignore(IgnoreReason::Hidden)
        );
        assert_eq!(
            p.decide(Path::new("notes/.hidden.md"), false, 10),
            Decision::Ignore(IgnoreReason::Hidden)
        );
        assert!(p.hides_dir(Path::new(".git"), false));
        assert!(!p.hides_dir(Path::new("notes"), false));
    }

    #[test]
    fn symlinks_rejected_unless_following() {
        let p = SourcePolicy::default_policy();
        assert_eq!(
            p.decide(Path::new("notes/link.md"), true, 10),
            Decision::Ignore(IgnoreReason::Symlink)
        );
        let mut scan = ScanConfig::default();
        scan.follow_symlinks = true;
        let p = policy(scan);
        assert_eq!(p.decide(Path::new("notes/link.md"), true, 10), Decision::Allow);
    }

    #[test]
    fn oversized_files_are_rejected_before_include() {
        let p = SourcePolicy::default_policy();
        assert_eq!(
            p.decide(Path::new("notes/huge.md"), false, u64::MAX),
            Decision::Ignore(IgnoreReason::TooLarge)
        );
    }

    #[test]
    fn custom_include_narrows_the_set() {
        let mut scan = ScanConfig::default();
        scan.include = vec!["**/*.org".to_string()];
        let p = policy(scan);
        assert_eq!(p.decide(Path::new("a.org"), false, 1), Decision::Allow);
        assert_eq!(
            p.decide(Path::new("a.md"), false, 1),
            Decision::Ignore(IgnoreReason::NotIncluded)
        );
    }

    #[test]
    fn invalid_globs_fall_back_to_defaults() {
        let mut scan = ScanConfig::default();
        scan.include = vec!["[unclosed".to_string()];
        let p = policy(scan);
        assert_eq!(p.decide(Path::new("notes/a.md"), false, 1), Decision::Allow);
    }

    #[test]
    fn legacy_exclude_prefixes_still_apply() {
        let p = SourcePolicy::from_parts(&ScanConfig::default(), vec![PathBuf::from("archive")]);
        assert_eq!(
            p.decide(Path::new("archive/old.md"), false, 1),
            Decision::Ignore(IgnoreReason::Excluded)
        );
    }

    #[test]
    fn counts_record_and_total() {
        let mut c = IgnoreCounts::default();
        c.record(IgnoreReason::Hidden);
        c.record(IgnoreReason::Hidden);
        c.record(IgnoreReason::TooLarge);
        assert_eq!(c.hidden, 2);
        assert_eq!(c.total(), 3);
    }
}
