use crate::domain::{
    LinkOccurrence, LinkTarget, ProjectionStore, QueryPage, ResolvedRelation, ResolutionStatus,
    Resource, ResourceRef, ResourceRelation, SegmentRecord, Selector, TextSpan,
};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeMap;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

pub struct SqliteProjection {
    conn: Connection,
}

impl SqliteProjection {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        let mut store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let mut store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&mut self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS resources (
                ref TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                revision TEXT NOT NULL,
                source_id TEXT NOT NULL,
                locator TEXT NOT NULL,
                properties_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS relations (
                source_ref TEXT NOT NULL,
                relation TEXT NOT NULL,
                target_ref TEXT NOT NULL,
                source_id TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_resources_source ON resources(source_id);
            CREATE INDEX IF NOT EXISTS idx_resources_kind ON resources(kind);
            CREATE INDEX IF NOT EXISTS idx_resources_title ON resources(title);
            CREATE INDEX IF NOT EXISTS idx_relations_source ON relations(source_id);

            CREATE TABLE IF NOT EXISTS segments (
                id TEXT PRIMARY KEY,
                attachment_ref TEXT NOT NULL,
                text TEXT NOT NULL,
                offset_start INTEGER NOT NULL,
                offset_end INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_segments_attachment ON segments(attachment_ref);

            CREATE TABLE IF NOT EXISTS link_occurrences (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_ref TEXT NOT NULL,
                target_json TEXT NOT NULL,
                raw TEXT NOT NULL,
                display_text TEXT,
                span_line INTEGER NOT NULL,
                span_col_start INTEGER NOT NULL,
                span_col_end INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'unresolved',
                candidates_json TEXT NOT NULL DEFAULT '[]',
                source_id TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS link_diagnostics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_ref TEXT NOT NULL,
                source_id TEXT NOT NULL,
                status TEXT NOT NULL,
                candidates_json TEXT NOT NULL DEFAULT '[]',
                raw TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_link_occ_source_ref ON link_occurrences(source_ref);
            CREATE INDEX IF NOT EXISTS idx_link_occ_source_id ON link_occurrences(source_id);

            CREATE TABLE IF NOT EXISTS resolved_relations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_ref TEXT NOT NULL,
                target_ref TEXT NOT NULL,
                target_json TEXT NOT NULL,
                status TEXT NOT NULL,
                candidates_json TEXT NOT NULL DEFAULT '[]',
                source_id TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_resolved_source_ref ON resolved_relations(source_ref);
            CREATE INDEX IF NOT EXISTS idx_resolved_target_ref ON resolved_relations(target_ref);
            CREATE INDEX IF NOT EXISTS idx_resolved_source_id ON resolved_relations(source_id);
            ",
        )?;
        Ok(())
    }
    pub fn insert_segments(&mut self, segments: &[SegmentRecord]) -> Result<(), StorageError> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO segments (id, attachment_ref, text, offset_start, offset_end)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for seg in segments {
                stmt.execute(params![
                    seg.id,
                    seg.attachment_ref,
                    seg.text,
                    seg.offset_start as i64,
                    seg.offset_end as i64,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn query_segments(&self, attachment_ref: &str) -> Result<Vec<SegmentRecord>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, attachment_ref, text, offset_start, offset_end
             FROM segments WHERE attachment_ref = ?1 ORDER BY offset_start ASC",
        )?;
        let rows = stmt.query_map(params![attachment_ref], |row| {
            let id: String = row.get(0)?;
            let attachment_ref: String = row.get(1)?;
            let text: String = row.get(2)?;
            let offset_start: i64 = row.get(3)?;
            let offset_end: i64 = row.get(4)?;
            Ok(SegmentRecord {
                id,
                attachment_ref,
                text,
                offset_start: offset_start as usize,
                offset_end: offset_end as usize,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }
}
impl ProjectionStore for SqliteProjection {
    type Error = StorageError;
    fn replace_source(
        &mut self,
        source_id: &str,
        resources: Vec<Resource>,
        relations: Vec<ResourceRelation>,
        link_occurrences: Vec<LinkOccurrence>,
    ) -> Result<(), StorageError> {
        let tx = self.conn.transaction()?;

        tx.execute(
            "DELETE FROM resources WHERE source_id = ?1",
            params![source_id],
        )?;
        tx.execute(
            "DELETE FROM relations WHERE source_id = ?1",
            params![source_id],
        )?;
        tx.execute(
            "DELETE FROM link_occurrences WHERE source_id = ?1",
            params![source_id],
        )?;

        {
            let mut stmt_res = tx.prepare(
                "INSERT INTO resources (ref, kind, title, revision, source_id, locator, properties_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;

            for res in resources {
                let ref_str = res.r#ref.to_string();
                let kind_str = res.kind.as_str();
                let props_json = serde_json::to_string(&res.properties)?;
                stmt_res.execute(params![
                    ref_str,
                    kind_str,
                    res.title,
                    res.revision,
                    source_id,
                    res.locator,
                    props_json,
                ])?;
            }
        }

        {
            let mut stmt_rel = tx.prepare(
                "INSERT INTO relations (source_ref, relation, target_ref, source_id)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;

            for rel in relations {
                stmt_rel.execute(params![
                    rel.source_ref.to_string(),
                    rel.relation,
                    rel.target_ref.to_string(),
                    source_id,
                ])?;
            }
        }

        {
            let mut stmt_occ = tx.prepare(
                "INSERT INTO link_occurrences (source_ref, target_json, raw, display_text, span_line, span_col_start, span_col_end, source_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;

            for occ in link_occurrences {
                let target_json = serde_json::to_string(&occ.target)?;
                stmt_occ.execute(params![
                    occ.source_ref.to_string(),
                    target_json,
                    occ.raw,
                    occ.display_text,
                    occ.span.line as i64,
                    occ.span.col_start as i64,
                    occ.span.col_end as i64,
                    source_id,
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    fn get(&self, r#ref: &ResourceRef) -> Result<Option<Resource>, StorageError> {
        let ref_str = r#ref.to_string();
        let mut stmt = self.conn.prepare(
            "SELECT ref, kind, title, revision, source_id, locator, properties_json
             FROM resources WHERE ref = ?1",
        )?;

        let row = stmt
            .query_row(params![ref_str], |row| {
                let r_ref_str: String = row.get(0)?;
                let kind_str: String = row.get(1)?;
                let title: String = row.get(2)?;
                let revision: String = row.get(3)?;
                let source_id: String = row.get(4)?;
                let locator: String = row.get(5)?;
                let properties_json: String = row.get(6)?;

                Ok((
                    r_ref_str,
                    kind_str,
                    title,
                    revision,
                    source_id,
                    locator,
                    properties_json,
                ))
            })
            .optional()?;

        if let Some((r_ref_str, _kind_str, title, revision, source_id, locator, properties_json)) =
            row
        {
            let r_ref = ResourceRef::parse(&r_ref_str)
                .map_err(|e| StorageError::InvalidData(format!("invalid ref in DB: {e}")))?;
            let properties: BTreeMap<String, String> = serde_json::from_str(&properties_json)?;
            Ok(Some(Resource {
                r#ref: r_ref,
                kind: r_ref.kind(),
                title,
                revision,
                source_id,
                locator,
                properties,
            }))
        } else {
            Ok(None)
        }
    }
    fn query(&self, selector: &Selector) -> Result<QueryPage, StorageError> {
        let mut sql = "SELECT ref, kind, title, revision, source_id, locator, properties_json FROM resources WHERE 1=1".to_string();
        let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(kind) = selector.kind {
            sql.push_str(" AND kind = ?");
            query_params.push(Box::new(kind.as_str().to_string()));
        }

        if let Some(ref title_sub) = selector.title_contains {
            sql.push_str(" AND LOWER(title) LIKE LOWER(?)");
            query_params.push(Box::new(format!("%{title_sub}%")));
        }

        if !selector.exact_refs.is_empty() {
            sql.push_str(" AND ref IN (");
            for (idx, r) in selector.exact_refs.iter().enumerate() {
                if idx > 0 {
                    sql.push_str(", ");
                }
                sql.push('?');
                query_params.push(Box::new(r.to_string()));
            }
            sql.push(')');
        }

        if let Some(ref src) = selector.source_id {
            sql.push_str(" AND source_id = ?");
            query_params.push(Box::new(src.clone()));
        }

        // Stable ordering so the same query on the same database returns
        // identical results — necessary for snapshot tests and for predictable
        // user-facing listings like "Recent activity".
        sql.push_str(" ORDER BY ref ASC");

        let mut stmt = self.conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> =
            query_params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let r_ref_str: String = row.get(0)?;
            let _kind_str: String = row.get(1)?;
            let title: String = row.get(2)?;
            let revision: String = row.get(3)?;
            let source_id: String = row.get(4)?;
            let locator: String = row.get(5)?;
            let properties_json: String = row.get(6)?;

            Ok((
                r_ref_str,
                title,
                revision,
                source_id,
                locator,
                properties_json,
            ))
        })?;

        let mut items = Vec::new();
        for row_res in rows {
            let (r_ref_str, title, revision, source_id, locator, properties_json) = row_res?;
            let r_ref = ResourceRef::parse(&r_ref_str)
                .map_err(|e| StorageError::InvalidData(format!("invalid ref in DB: {e}")))?;
            let properties: BTreeMap<String, String> = serde_json::from_str(&properties_json)?;
            items.push(Resource {
                r#ref: r_ref,
                kind: r_ref.kind(),
                title,
                revision,
                source_id,
                locator,
                properties,
            });
        }

        Ok(QueryPage {
            items,
            next_cursor: None,
        })
    }

    fn upsert_resource(&mut self, resource: &Resource) -> Result<(), StorageError> {
        let ref_str = resource.r#ref.to_string();
        let kind_str = resource.kind.as_str();
        let props_json = serde_json::to_string(&resource.properties)?;
        self.conn.execute(
            "INSERT INTO resources (ref, kind, title, revision, source_id, locator, properties_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(ref) DO UPDATE SET
                 kind = excluded.kind,
                 title = excluded.title,
                 revision = excluded.revision,
                 source_id = excluded.source_id,
                 locator = excluded.locator,
                 properties_json = excluded.properties_json",
            params![
                ref_str,
                kind_str,
                resource.title,
                resource.revision,
                resource.source_id,
                resource.locator,
                props_json,
            ],
        )?;
        Ok(())
    }

    fn delete_resource(&mut self, r_ref: &ResourceRef) -> Result<(), StorageError> {
        let ref_str = r_ref.to_string();
        self.conn
            .execute("DELETE FROM resources WHERE ref = ?1", params![ref_str])?;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), StorageError> {
        self.conn.execute("DELETE FROM resources", [])?;
        self.conn.execute("DELETE FROM relations", [])?;
        self.conn
            .execute("DELETE FROM link_occurrences", [])?;
        self.conn
            .execute("DELETE FROM resolved_relations", [])?;
        Ok(())
    }
    fn insert_segments(&mut self, segments: &[SegmentRecord]) -> Result<(), StorageError> {
        self.insert_segments(segments)
    }

    fn query_segments(&self, attachment_ref: &str) -> Result<Vec<SegmentRecord>, StorageError> {
        self.query_segments(attachment_ref)
    }

    fn replace_link_occurrences(
        &mut self,
        source_id: &str,
        occurrences: Vec<LinkOccurrence>,
    ) -> Result<(), StorageError> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM link_occurrences WHERE source_id = ?1",
            params![source_id],
        )?;

        {
            let mut stmt_occ = tx.prepare(
                "INSERT INTO link_occurrences (source_ref, target_json, raw, display_text, span_line, span_col_start, span_col_end, source_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;

            for occ in occurrences {
                let target_json = serde_json::to_string(&occ.target)?;
                stmt_occ.execute(params![
                    occ.source_ref.to_string(),
                    target_json,
                    occ.raw,
                    occ.display_text,
                    occ.span.line as i64,
                    occ.span.col_start as i64,
                    occ.span.col_end as i64,
                    source_id,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn query_link_occurrences(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, StorageError> {
        let ref_str = source_ref.to_string();
        let mut stmt = self.conn.prepare(
            "SELECT source_ref, target_json, raw, display_text, span_line, span_col_start, span_col_end
             FROM link_occurrences WHERE source_ref = ?1 ORDER BY span_line, span_col_start",
        )?;

        let rows = stmt.query_map(params![ref_str], |row| {
            let source_ref_str: String = row.get(0)?;
            let target_json: String = row.get(1)?;
            let raw: String = row.get(2)?;
            let display_text: Option<String> = row.get(3)?;
            let span_line: i64 = row.get(4)?;
            let span_col_start: i64 = row.get(5)?;
            let span_col_end: i64 = row.get(6)?;
            Ok((
                source_ref_str,
                target_json,
                raw,
                display_text,
                span_line,
                span_col_start,
                span_col_end,
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            let (source_ref_str, target_json, raw, display_text, span_line, span_col_start, span_col_end) = r?;
            let source_ref = ResourceRef::parse(&source_ref_str)
                .map_err(|e| StorageError::InvalidData(format!("invalid ref: {e}")))?;
            let target: LinkTarget = serde_json::from_str(&target_json)?;
            results.push(LinkOccurrence {
                source_ref,
                target,
                raw,
                display_text,
                span: TextSpan {
                    line: span_line as usize,
                    col_start: span_col_start as usize,
                    col_end: span_col_end as usize,
                },
            });
        }
        Ok(results)
    }

    fn replace_resolved_relations(
        &mut self,
        source_id: &str,
        relations: Vec<ResolvedRelation>,
    ) -> Result<(), StorageError> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM resolved_relations WHERE source_id = ?1",
            params![source_id],
        )?;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO resolved_relations (source_ref, target_ref, target_json, status, candidates_json, source_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;

            for rel in relations {
                let target_json = serde_json::to_string(&rel.target)?;
                let status_str = serde_json::to_string(&rel.status)?;
                let candidates_json = serde_json::to_string(&rel.candidates)?;
                stmt.execute(params![
                    rel.source_ref.to_string(),
                    rel.target_ref.to_string(),
                    target_json,
                    status_str,
                    candidates_json,
                    source_id,
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    fn query_resolved_relations(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<ResolvedRelation>, StorageError> {
        let ref_str = source_ref.to_string();
        let mut stmt = self.conn.prepare(
            "SELECT source_ref, target_ref, target_json, status, candidates_json
             FROM resolved_relations WHERE source_ref = ?1",
        )?;

        let rows = stmt.query_map(params![ref_str], |row| {
            let source_ref_str: String = row.get(0)?;
            let target_ref_str: String = row.get(1)?;
            let target_json: String = row.get(2)?;
            let status_str: String = row.get(3)?;
            let candidates_json: String = row.get(4)?;
            Ok((
                source_ref_str,
                target_ref_str,
                target_json,
                status_str,
                candidates_json,
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            let (source_ref_str, target_ref_str, target_json, status_str, candidates_json) = r?;
            let source_ref = ResourceRef::parse(&source_ref_str)
                .map_err(|e| StorageError::InvalidData(format!("invalid ref: {e}")))?;
            let target_ref = ResourceRef::parse(&target_ref_str)
                .map_err(|e| StorageError::InvalidData(format!("invalid ref: {e}")))?;
            let target: LinkTarget = serde_json::from_str(&target_json)?;
            let status: ResolutionStatus = serde_json::from_str(&status_str)?;
            let candidates: Vec<ResourceRef> = serde_json::from_str(&candidates_json)?;
            results.push(ResolvedRelation {
                source_ref,
                target_ref,
                target,
                status,
                candidates,
            });
        }
        Ok(results)
    }

    fn write_link_diagnostics(
        &mut self,
        source_id: &str,
        diagnostics: &[(LinkOccurrence, ResolutionStatus, Vec<ResourceRef>)],
    ) -> Result<(), StorageError> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM link_diagnostics WHERE source_id = ?1",
            params![source_id],
        )?;
        let mut stmt = tx.prepare(
            "INSERT INTO link_diagnostics (source_ref, source_id, status, candidates_json, raw)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for (occ, status, candidates) in diagnostics {
            let status_str = serde_json::to_string(status)?;
            let candidates_json = serde_json::to_string(candidates)?;
            stmt.execute(params![
                occ.source_ref.to_string(),
                source_id,
                status_str,
                candidates_json,
                occ.raw,
            ])?;
        }
        drop(stmt);
        // Mirror onto link_occurrences so list_link_diagnostics can join
        // without a sidecar read. Match by raw text within a source.
        tx.execute(
            "UPDATE link_occurrences SET status = 'unresolved', candidates_json = '[]' WHERE source_id = ?1",
            params![source_id],
        )?;
        let mut stmt_update = tx.prepare(
            "UPDATE link_occurrences
                SET status = ?1, candidates_json = ?2
              WHERE source_id = ?3 AND raw = ?4",
        )?;
        for (occ, status, candidates) in diagnostics {
            let status_str = serde_json::to_string(status)?;
            let candidates_json = serde_json::to_string(candidates)?;
            stmt_update.execute(params![
                status_str,
                candidates_json,
                source_id,
                occ.raw,
            ])?;
        }
        drop(stmt_update);
        tx.commit()?;
        Ok(())
    }

    fn list_link_diagnostics(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Option<Vec<crate::domain::LinkDiagnostic>>, StorageError> {
        let ref_str = source_ref.to_string();
        let mut stmt = self.conn.prepare(
            "SELECT lo.source_ref, lo.target_json, lo.raw, lo.display_text,
                    lo.span_line, lo.span_col_start, lo.span_col_end,
                    lo.status, lo.candidates_json
               FROM link_occurrences lo
              WHERE lo.source_ref = ?1
              ORDER BY lo.span_line, lo.span_col_start",
        )?;
        let rows = stmt.query_map(params![ref_str], |row| {
            let source_ref_str: String = row.get(0)?;
            let target_json: String = row.get(1)?;
            let raw: String = row.get(2)?;
            let display_text: Option<String> = row.get(3)?;
            let span_line: i64 = row.get(4)?;
            let span_col_start: i64 = row.get(5)?;
            let span_col_end: i64 = row.get(6)?;
            let status_str: String = row.get(7)?;
            let candidates_json: String = row.get(8)?;
            Ok((
                source_ref_str,
                target_json,
                raw,
                display_text,
                span_line,
                span_col_start,
                span_col_end,
                status_str,
                candidates_json,
            ))
        })?;
        let mut results = Vec::new();
        let mut any = false;
        for r in rows {
            any = true;
            let (source_ref_str, target_json, raw, display_text,
                 span_line, span_col_start, span_col_end,
                 status_str, candidates_json) = r?;
            let source_ref = ResourceRef::parse(&source_ref_str)
                .map_err(|e| StorageError::InvalidData(format!("invalid ref: {e}")))?;
            let target: LinkTarget = serde_json::from_str(&target_json)?;
            let status: ResolutionStatus = serde_json::from_str(&status_str)?;
            let candidates: Vec<ResourceRef> = serde_json::from_str(&candidates_json)?;
            results.push(crate::domain::LinkDiagnostic {
                occurrence: LinkOccurrence {
                    source_ref,
                    target,
                    raw,
                    display_text,
                    span: TextSpan {
                        line: span_line as usize,
                        col_start: span_col_start as usize,
                        col_end: span_col_end as usize,
                    },
                },
                status,
                candidates,
            });
        }
        Ok(if any { Some(results) } else { None })
    }
}
