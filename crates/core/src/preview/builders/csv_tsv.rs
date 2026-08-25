//! Previewer for CSV and TSV tabular attachments.
//!
//! Matches on a `.csv`/`.tsv` locator suffix or the corresponding
//! MIME types (`text/csv`, `text/tab-separated-values`). The full
//! byte payload must be present in `ctx.bytes`; if it is missing the
//! render step returns `PreviewError::MissingBytes`.
//!
//! Parsing uses the `csv` crate with the RFC 4180 default builder:
//! variable record length, the chosen delimiter, double-quote escape,
//! no header detection. The first record is treated as the header
//! row when present (heuristic: every cell starts with an ASCII
//! letter or `_`); otherwise `headers` is empty and every record
//! lands in `rows`.
//!
//! Row count is capped at `MAX_ROWS` records to bound the rendered
//! table; remaining rows are dropped silently. CSV parse errors are
//! surfaced to the caller rather than swallowed, because a truncated
//! or malformed CSV is a real bug, not an empty result.

use crate::domain::{Resource, ResourceKind, ResourceRef};
use crate::preview::{PreviewContext, PreviewError, PreviewModel, Previewer, Table};
use bytes::Bytes;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Maximum number of records (header + rows) parsed before truncation.
const MAX_ROWS: usize = 5_000;

/// Previewer for `.csv` and `.tsv` tabular attachments.
pub struct CsvTsvPreviewer;

impl Previewer for CsvTsvPreviewer {
    fn id(&self) -> &'static str {
        "csv_tsv"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        let lower = ctx.locator.to_string_lossy().to_ascii_lowercase();
        if lower.ends_with(".csv") || lower.ends_with(".tsv") {
            return true;
        }
        matches!(
            ctx.mime.as_deref(),
            Some("text/csv") | Some("application/csv") | Some("text/tab-separated-values")
        )
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let bytes: Bytes = ctx
            .bytes
            .clone()
            .ok_or_else(|| PreviewError::MissingBytes(ctx.resource.r#ref.to_string()))?;

        let delimiter = if ctx
            .locator
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".tsv")
        {
            b'\t'
        } else {
            b','
        };

        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(bytes.as_ref());

        let mut records: Vec<csv::StringRecord> = Vec::new();
        for (i, rec) in reader.records().enumerate() {
            if i >= MAX_ROWS {
                break;
            }
            let rec = rec.map_err(|e| PreviewError::Extraction(format!("csv parse: {e}")))?;
            records.push(rec);
        }

        let (headers, rows) = split_header(records);

        Ok(PreviewModel::Table {
            table: Table { headers, rows },
        })
    }
}

/// Treat the first record as a header iff every cell starts with an
/// ASCII letter or underscore (cheap heuristic; matches typical
/// spreadsheet exports). Otherwise the first record is data.
fn split_header(mut records: Vec<csv::StringRecord>) -> (Vec<String>, Vec<Vec<String>>) {
    let headers = match records.first() {
        Some(first) if looks_like_header(first) => {
            let h: Vec<String> = first.iter().map(|s| s.to_string()).collect();
            records.remove(0);
            h
        }
        _ => Vec::new(),
    };
    let rows: Vec<Vec<String>> = records
        .into_iter()
        .map(|rec| rec.iter().map(|s| s.to_string()).collect())
        .collect();
    (headers, rows)
}

fn looks_like_header(rec: &csv::StringRecord) -> bool {
    if rec.is_empty() {
        return false;
    }
    rec.iter().all(|cell| {
        let mut chars = cell.chars();
        match chars.next() {
            Some(c) if c.is_ascii_alphabetic() || c == '_' => true,
            _ => false,
        }
    })
}

/// Build a placeholder `PreviewContext` for unit tests. The builder
/// only consults `ctx.bytes`, `ctx.mime`, and `ctx.locator`; the rest
/// is filled in with the bare minimum required by the type.
#[cfg(test)]
fn ctx<'a>(
    bytes: &'a [u8],
    locator: PathBuf,
    mime: &'a str,
    catalog: &'a crate::preview::PreviewerCatalog,
) -> PreviewContext<'a> {
    let resource = Resource {
        r#ref: ResourceRef::new(ResourceKind::Attachment, ulid::Ulid::new()),
        kind: ResourceKind::Attachment,
        title: String::new(),
        revision: String::new(),
        source_id: String::from("native"),
        locator: locator.to_string_lossy().into_owned(),
        properties: BTreeMap::new(),
        object_id: Default::default(),
        primary_source_id: String::new(),
    };
    PreviewContext {
        resource,
        bytes: Some(Bytes::copy_from_slice(bytes)),
        mime: Some(mime.to_string()),
        locator,
        segments: Vec::new(),
        siblings: Vec::new(),
        catalog,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::PreviewerCatalog;

    #[test]
    fn matches_csv_locator() {
        let cat = PreviewerCatalog::new();
        let c = ctx(b"a,b\n1,2\n", PathBuf::from("data.csv"), "text/plain", &cat);
        assert!(CsvTsvPreviewer.matches(&c));
    }

    #[test]
    fn matches_tsv_by_mime() {
        let cat = PreviewerCatalog::new();
        let c = ctx(
            b"a\tb\n1\t2\n",
            PathBuf::from("weird.dat"),
            "text/tab-separated-values",
            &cat,
        );
        assert!(CsvTsvPreviewer.matches(&c));
    }

    #[test]
    fn rejects_other_extensions() {
        let cat = PreviewerCatalog::new();
        let c = ctx(b"hello", PathBuf::from("note.txt"), "text/plain", &cat);
        assert!(!CsvTsvPreviewer.matches(&c));
    }

    #[test]
    fn renders_csv_with_header() {
        let cat = PreviewerCatalog::new();
        let c = ctx(
            b"name,age\nalice,30\nbob,40\n",
            PathBuf::from("people.csv"),
            "text/csv",
            &cat,
        );
        let out = CsvTsvPreviewer.render(&c).expect("render ok");
        match out {
            PreviewModel::Table { table } => {
                assert_eq!(table.headers, vec!["name", "age"]);
                assert_eq!(
                    table.rows,
                    vec![
                        vec!["alice".to_string(), "30".to_string()],
                        vec!["bob".to_string(), "40".to_string()],
                    ]
                );
            }
            other => panic!("expected Table, got {other:?}"),
        }
    }

    #[test]
    fn renders_tsv_uses_tab_delimiter() {
        let cat = PreviewerCatalog::new();
        let c = ctx(
            b"a\tb\tc\n1\t2,3\t4\n",
            PathBuf::from("mixed.tsv"),
            "text/tab-separated-values",
            &cat,
        );
        let out = CsvTsvPreviewer.render(&c).expect("render ok");
        match out {
            PreviewModel::Table { table } => {
                assert_eq!(table.headers, vec!["a", "b", "c"]);
                // Comma is part of the second cell, NOT a column separator.
                assert_eq!(
                    table.rows,
                    vec![vec!["1".to_string(), "2,3".to_string(), "4".to_string()]]
                );
            }
            other => panic!("expected Table, got {other:?}"),
        }
    }

    #[test]
    fn no_header_when_first_row_starts_with_digit() {
        let cat = PreviewerCatalog::new();
        let c = ctx(
            b"1,2,3\n4,5,6\n",
            PathBuf::from("nums.csv"),
            "text/csv",
            &cat,
        );
        let out = CsvTsvPreviewer.render(&c).expect("render ok");
        match out {
            PreviewModel::Table { table } => {
                assert!(table.headers.is_empty());
                assert_eq!(table.rows.len(), 2);
            }
            other => panic!("expected Table, got {other:?}"),
        }
    }

    #[test]
    fn missing_bytes_surfaces_error() {
        let cat = PreviewerCatalog::new();
        let resource = Resource {
            r#ref: ResourceRef::new(ResourceKind::Attachment, ulid::Ulid::new()),
            kind: ResourceKind::Attachment,
            title: String::new(),
            revision: String::new(),
            source_id: String::from("native"),
            locator: String::from("data.csv"),
            properties: BTreeMap::new(),
            object_id: Default::default(),
            primary_source_id: String::new(),
        };
        let c = PreviewContext {
            resource,
            bytes: None,
            mime: Some(String::from("text/csv")),
            locator: PathBuf::from("data.csv"),
            segments: Vec::new(),
            siblings: Vec::new(),
            catalog: &cat,
        };
        let err = CsvTsvPreviewer.render(&c).unwrap_err();
        assert!(matches!(err, PreviewError::MissingBytes(_)), "got {err:?}");
    }
}
