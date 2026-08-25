use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub use crate::domain::ConflictRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeResult {
    Clean(String),
    Conflict { conflict: ConflictRecord },
}

pub struct HeadsTracker {
    root: PathBuf,
}

impl HeadsTracker {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.join("heads"),
        }
    }

    pub fn set_head(&self, actor_id: &str, snapshot: &str) -> Result<(), io::Error> {
        if !self.root.exists() {
            fs::create_dir_all(&self.root)?;
        }
        let file = self.root.join(actor_id);
        fs::write(file, snapshot)?;
        Ok(())
    }

    pub fn get_head(&self, actor_id: &str) -> Result<Option<String>, io::Error> {
        let file = self.root.join(actor_id);
        if !file.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(file)?;
        Ok(Some(content.trim().to_string()))
    }
}

pub struct ThreeWayMerger;

impl ThreeWayMerger {
    pub fn merge(logical_path: &str, base: &str, mine: &str, theirs: &str) -> MergeResult {
        if mine == theirs {
            return MergeResult::Clean(mine.to_string());
        }
        if mine == base {
            return MergeResult::Clean(theirs.to_string());
        }
        if theirs == base {
            return MergeResult::Clean(mine.to_string());
        }

        let base_lines: Vec<&str> = base.lines().collect();
        let mine_lines: Vec<&str> = mine.lines().collect();
        let theirs_lines: Vec<&str> = theirs.lines().collect();

        let mut merged = Vec::new();
        let max_len = mine_lines.len().max(theirs_lines.len());
        let mut has_conflict = false;

        for i in 0..max_len {
            let b_line = base_lines.get(i).copied().unwrap_or("");
            let m_line = mine_lines.get(i).copied().unwrap_or("");
            let t_line = theirs_lines.get(i).copied().unwrap_or("");

            if m_line == t_line {
                merged.push(m_line.to_string());
            } else if m_line == b_line {
                merged.push(t_line.to_string());
            } else if t_line == b_line {
                merged.push(m_line.to_string());
            } else {
                has_conflict = true;
                merged.push(format!(
                    "<<<<<<< mine\n{m_line}\n=======\n{t_line}\n>>>>>>> theirs"
                ));
            }
        }

        let result_text = merged.join("\n") + "\n";

        if has_conflict {
            use sha2::{Digest, Sha256};
            let mine_hash = format!("{:x}", Sha256::digest(mine.as_bytes()));
            let theirs_hash = format!("{:x}", Sha256::digest(theirs.as_bytes()));

            MergeResult::Conflict {
                conflict: ConflictRecord::pending(
                    logical_path,
                    mine_hash,
                    theirs_hash,
                    result_text,
                ),
            }
        } else {
            MergeResult::Clean(result_text)
        }
    }
}
