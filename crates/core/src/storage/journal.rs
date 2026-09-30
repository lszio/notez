//! SQLite-backed event journal and audit log.
//!
//! The journal is a single append-only table (`event_journal`) with a
//! monotonic `sequence` column. `audit_records` is a parallel table
//! the audit port writes through. Both adapters take a
//! [`std::sync::Mutex<Connection>`] so the underlying SQLite handle
//! stays exclusive to one thread at a time (rusqlite connections are
//! `!Send` by default).

use crate::domain::audit::{AuditError, AuditLog, AuditRecord};
use crate::domain::change::{Actor, Change, ChangeOp};
use crate::domain::change::ChangePrecondition as DomainPrecondition;
use crate::domain::journal::{ActivityRecord, JournalEntry, JournalError};
use crate::domain::resource::ResourceRef;
use rusqlite::{Connection, params};
use serde_json::Value;
use std::sync::Mutex;

pub struct SqliteEventJournal {
    conn: Mutex<Connection>,
}

impl SqliteEventJournal {
    pub fn new(conn: Mutex<Connection>) -> Self {
        Self { conn }
    }
}

impl crate::domain::journal::EventJournal for SqliteEventJournal {
    fn append(&self, change: &Change) -> Result<u64, JournalError> {
        let op_json = serde_json::to_string(&change.op).map_err(|e| JournalError::Schema(e.to_string()))?;
        let targets_json = serde_json::to_string(&change.targets.iter().map(|t| t.to_string()).collect::<Vec<_>>()).map_err(|e| JournalError::Schema(e.to_string()))?;
        let precondition_json = serde_json::to_string(&change.precondition).map_err(|e| JournalError::Schema(e.to_string()))?;
        let payload_json = serde_json::to_string(&change.payload).map_err(|e| JournalError::Schema(e.to_string()))?;
        let guard = self.conn.lock().map_err(|e| JournalError::Storage(e.to_string()))?;
        guard.execute(
            "INSERT OR IGNORE INTO event_journal
             (change_id, actor_principal, actor_space, actor_source, at_unix_millis, source_id,
              op_json, targets_json, expected_revision, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![change.id.to_string(), change.actor.principal, change.actor.space,
                change.actor.source, change.at_unix_millis, change.source_id, op_json,
                targets_json, precondition_json, payload_json],
        ).map_err(|e| JournalError::Storage(e.to_string()))?;
        let seq: i64 = guard.query_row(
            "SELECT sequence FROM event_journal WHERE change_id = ?1",
            params![change.id.to_string()], |r| r.get(0),
        ).map_err(|e| JournalError::Storage(e.to_string()))?;
        Ok(seq as u64)
    }

    fn since(&self, cursor: u64) -> Result<Vec<JournalEntry>, JournalError> {
        let guard = self.conn.lock().map_err(|e| JournalError::Storage(e.to_string()))?;
        let mut stmt = guard
            .prepare(
                "SELECT sequence, change_id, actor_principal, actor_space, actor_source,
                        at_unix_millis, source_id, op_json, targets_json,
                        expected_revision, payload_json
                 FROM event_journal
                 WHERE sequence >= ?1
                 ORDER BY sequence ASC",
            )
            .map_err(|e| JournalError::Storage(e.to_string()))?;
        let rows = stmt
            .query_map(params![cursor as i64], |row| {
                let seq: i64 = row.get(0)?;
                let change_id_str: String = row.get(1)?;
                let principal: String = row.get(2)?;
                let space: Option<String> = row.get(3)?;
                let source: Option<String> = row.get(4)?;
                let at_unix_millis: i64 = row.get(5)?;
                let source_id: String = row.get(6)?;
                let op_json: String = row.get(7)?;
                let targets_json: String = row.get(8)?;
                let precondition_json: Option<String> = row.get(9)?;
                let payload_json: String = row.get(10)?;
                Ok(JournalRow {
                    sequence: seq,
                    change_id: change_id_str,
                    actor: Actor {
                        principal,
                        space,
                        source,
                    },
                    at_unix_millis,
                    source_id,
                    op_json,
                    targets_json,
                    precondition_json,
                    payload_json,
                })
            })
            .map_err(|e| JournalError::Storage(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| JournalError::Storage(e.to_string()))?;
        rows.into_iter()
            .map(|r| r.into_entry())
            .collect::<Result<Vec<_>, _>>()
    }

    fn len(&self) -> Result<u64, JournalError> {
        let guard = self.conn.lock().map_err(|e| JournalError::Storage(e.to_string()))?;
        let n: i64 = guard.query_row("SELECT COUNT(*) FROM event_journal", [], |row| row.get(0))
            .map_err(|e| JournalError::Storage(e.to_string()))?;
        Ok(n as u64)
    }

    fn activity(&self, limit: usize) -> Result<Vec<ActivityRecord>, JournalError> {
        let entries = self.since(0)?;
        let guard = self.conn.lock().map_err(|e| JournalError::Storage(e.to_string()))?;
        entries.into_iter().rev().take(limit).map(|entry| {
            let audited: bool = guard.query_row(
                "SELECT EXISTS(SELECT 1 FROM audit_records WHERE change_id = ?1)",
                params![entry.change.id.to_string()],
                |row| row.get(0),
            ).map_err(|e| JournalError::Storage(e.to_string()))?;
            Ok(ActivityRecord { sequence: entry.sequence, change: entry.change, audited })
        }).collect()
    }
}

struct JournalRow {
    sequence: i64,
    change_id: String,
    actor: Actor,
    at_unix_millis: i64,
    source_id: String,
    op_json: String,
    targets_json: String,
    precondition_json: Option<String>,
    payload_json: String,
}

impl JournalRow {
    fn into_entry(self) -> Result<JournalEntry, JournalError> {
        let id = ulid::Ulid::from_string(&self.change_id)
            .map_err(|e| JournalError::Schema(format!("change_id: {e}")))?;
        let op: ChangeOp = serde_json::from_str(&self.op_json)
            .map_err(|e| JournalError::Schema(format!("op: {e}")))?;
        let target_strs: Vec<String> = serde_json::from_str(&self.targets_json)
            .map_err(|e| JournalError::Schema(format!("targets: {e}")))?;
        let mut targets = Vec::with_capacity(target_strs.len());
        for s in target_strs {
            targets.push(
                ResourceRef::parse(&s)
                    .map_err(|e| JournalError::Schema(format!("target {s}: {e}")))?,
            );
        }
        let payload: Value = serde_json::from_str(&self.payload_json)
            .map_err(|e| JournalError::Schema(format!("payload: {e}")))?;
        Ok(JournalEntry {
            sequence: self.sequence as u64,
            change: Change {
                id,
                actor: self.actor,
                at_unix_millis: self.at_unix_millis,
                source_id: self.source_id,
                op: op.clone(),
                targets,
                precondition: decode_precondition(self.precondition_json.as_deref(), &op)?,
                payload,
            },
        })
    }
}
fn decode_precondition(json: Option<&str>, _op: &ChangeOp) -> Result<DomainPrecondition, JournalError> {
    let Some(json) = json else {
        return Ok(DomainPrecondition::UnconditionalObservation);
    };
    if let Ok(value) = serde_json::from_str::<DomainPrecondition>(json) {
        return Ok(value);
    }
    let revision: String = serde_json::from_str(json)
        .map_err(|e| JournalError::Schema(format!("precondition: {e}")))?;
    Ok(DomainPrecondition::Revision(notez_protocol::request::RevisionPrecondition::MustMatch {
        revision: notez_protocol::request::NonEmptyRevision::new(revision)
            .map_err(|e| JournalError::Schema(format!("legacy revision: {e}")))?,
    }))

}

pub struct SqliteAuditLog {
    conn: Mutex<Connection>,
}

impl SqliteAuditLog {
    pub fn new(conn: Mutex<Connection>) -> Self {
        Self { conn }
    }
}

impl AuditLog for SqliteAuditLog {
    fn append(&self, record: AuditRecord) -> Result<(), AuditError> {
        let outcome_json = serde_json::to_string(&record.outcome).map_err(|e| AuditError::Storage(e.to_string()))?;
        let precondition_json = serde_json::to_string(&record.precondition).map_err(|e| AuditError::Storage(e.to_string()))?;
        let guard = self.conn.lock().map_err(|e| AuditError::Storage(e.to_string()))?;
        guard.execute(
            "INSERT INTO audit_records (change_id, principal, action, target_ref, precondition_json, outcome_json, recorded_at_unix_millis) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![record.change_id.to_string(), record.principal, record.action, record.target.to_string(), precondition_json, outcome_json, record.recorded_at_unix_millis],
        ).map_err(|e| AuditError::Storage(e.to_string()))?;
        Ok(())
    }

    fn for_target(&self, target: &ResourceRef) -> Result<Vec<AuditRecord>, AuditError> {
        let guard = self.conn.lock().map_err(|e| AuditError::Storage(e.to_string()))?;
        let mut stmt = guard.prepare("SELECT change_id, principal, action, precondition_json, outcome_json, recorded_at_unix_millis FROM audit_records WHERE target_ref = ?1 ORDER BY id ASC").map_err(|e| AuditError::Storage(e.to_string()))?;
        let rows = stmt.query_map(params![target.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, String>(4)?, row.get::<_, i64>(5)?))
        }).map_err(|e| AuditError::Storage(e.to_string()))?.collect::<Result<Vec<_>, _>>().map_err(|e| AuditError::Storage(e.to_string()))?;
        rows.into_iter().map(|(cid, principal, action, precondition_json, outcome_json, at)| {
            let precondition = decode_audit_precondition(precondition_json.as_deref())?;
            Ok(AuditRecord { change_id: ulid::Ulid::from_string(&cid).map_err(|e| AuditError::Storage(e.to_string()))?, principal, action, target: target.clone(), precondition, outcome: serde_json::from_str(&outcome_json).map_err(|e| AuditError::Storage(e.to_string()))?, recorded_at_unix_millis: at })
        }).collect()
    }
}

fn decode_audit_precondition(json: Option<&str>) -> Result<DomainPrecondition, AuditError> {
    let Some(json) = json else { return Ok(DomainPrecondition::UnconditionalObservation); };
    if let Ok(value) = serde_json::from_str(json) { return Ok(value); }
    let legacy: Option<String> = serde_json::from_str(json).map_err(|e| AuditError::Storage(e.to_string()))?;
    Ok(match legacy {
        Some(revision) => DomainPrecondition::Revision(notez_protocol::request::RevisionPrecondition::MustMatch {
            revision: notez_protocol::request::NonEmptyRevision::new(revision).map_err(|e| AuditError::Storage(e.to_string()))?,
        }),
        None => DomainPrecondition::UnconditionalObservation,
    })
}

#[cfg(test)]
mod tests {
    use super::decode_precondition;
    use crate::domain::change::{ChangeOp, ChangePrecondition};
    use notez_protocol::request::{NonEmptyRevision, RevisionPrecondition};

    #[test]
    fn decode_precondition_preserves_new_json_shape() {
        let value = ChangePrecondition::Revision(RevisionPrecondition::MustMatch {
            revision: NonEmptyRevision::new("rev-json").unwrap(),
        });
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(decode_precondition(Some(&encoded), &ChangeOp::Writeback).unwrap(), value);
    }

    #[test]
    fn decode_precondition_accepts_legacy_bare_string() {
        let encoded = serde_json::to_string("rev-legacy").unwrap();
        assert_eq!(decode_precondition(Some(&encoded), &ChangeOp::Writeback).unwrap(), ChangePrecondition::Revision(RevisionPrecondition::MustMatch {
            revision: NonEmptyRevision::new("rev-legacy").unwrap(),
        }));
    }
}