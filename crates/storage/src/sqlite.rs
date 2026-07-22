use domain::{ProjectionStore, QueryPage, Resource, ResourceRef, ResourceRelation, Selector};
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
            ",
        )?;
        Ok(())
    }
}
impl ProjectionStore for SqliteProjection {
    type Error = StorageError;
    fn replace_source(
        &mut self,
        source_id: &str,
        resources: Vec<Resource>,
        relations: Vec<ResourceRelation>,
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

    fn clear(&mut self) -> Result<(), StorageError> {
        self.conn.execute("DELETE FROM resources", [])?;
        self.conn.execute("DELETE FROM relations", [])?;
        Ok(())
    }
}
