# Notez Attachments and Extraction Jobs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Content-Addressed Blob Storage (`.notez/blobs/`), `Attachment` Resource indexing, Extractor plugins (text, image Exif/metadata, PDF text), text `Segment` slicing, extraction job execution, and CLI/MCP attachment tools.

**Architecture:** Extend `storage` with `BlobStore` for CAS (SHA-256). Add `ResourceKind::Attachment` to `domain`. Introduce `crates/artifact` for `Extractor` plugins and text segmenting. Update `ApplicationService` to manage attachments, execute extraction jobs, store text segments in SQLite, and support CLI/MCP attachment tools.

**Tech Stack:** Rust 1.94, Cargo workspace, serde, sha2, thiserror, ulid, rusqlite, clap.

---

### Task 1: Content-Addressed Blob Storage and MIME Detection

**Files:**
- Create: `crates/storage/src/blob.rs`
- Modify: `crates/storage/src/lib.rs`
- Test: `crates/storage/tests/blob.rs`

**Interfaces:**
- Produces: `BlobStore::store_bytes`, `BlobStore::get`, `BlobStore::has`, `BlobMeta`.

- [ ] **Step 1: Write failing BlobStore test**

Create `crates/storage/tests/blob.rs`: test storing raw bytes in `.notez/blobs/`, verify SHA-256 content address, verify file exists and deduplicates identical content.

- [ ] **Step 2: Run test and verify failure**

Run: `cargo test -p storage --test blob`
Expected: FAIL because `BlobStore` is missing.

- [ ] **Step 3: Implement BlobStore**

Implement CAS file layout `.notez/blobs/ab/cd/abcd...`, calculate SHA-256 hex digest, detect simple MIME types (image/png, image/jpeg, text/plain, application/pdf).

- [ ] **Step 4: Run storage blob tests**

Run: `cargo test -p storage`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/storage
git commit -m "feat: add content-addressed blob storage for attachments"
```

---

### Task 2: Attachment Resource and Segment Storage Schema

**Files:**
- Modify: `crates/domain/src/resource.rs`
- Modify: `crates/domain/src/lib.rs`
- Modify: `crates/storage/src/sqlite.rs`
- Test: `crates/storage/tests/attachment_schema.rs`

**Interfaces:**
- Produces: `ResourceKind::Attachment`, SQLite table `segments(id PRIMARY KEY, attachment_ref, text, offset_start, offset_end)`.

- [ ] **Step 1: Write failing attachment storage test**

Create `crates/storage/tests/attachment_schema.rs`: test storing and querying `Attachment` resources and text segments linked to an attachment ref.

- [ ] **Step 2: Run attachment schema test**

Run: `cargo test -p storage --test attachment_schema`
Expected: FAIL because `ResourceKind::Attachment` or `segments` table is missing.

- [ ] **Step 3: Implement Attachment ResourceKind and segments schema**

Add `ResourceKind::Attachment`, update `SqliteProjection` schema with `segments` table and helper methods `insert_segments` and `query_segments`.

- [ ] **Step 4: Run storage tests**

Run: `cargo test -p storage`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/domain crates/storage
git commit -m "feat: add Attachment ResourceKind and segment database schema"
```

---

### Task 3: Artifact Crate, Extractor Plugins, and Segment Slicing

**Files:**
- Create: `crates/artifact/Cargo.toml`
- Create: `crates/artifact/src/lib.rs`
- Create: `crates/artifact/src/extractor.rs`
- Test: `crates/artifact/tests/extractor.rs`

**Interfaces:**
- Produces: trait `Extractor`, `TextExtractor`, `ImageMetadataExtractor`, `SegmentSlicer`, `ExtractedContent`.

- [ ] **Step 1: Write failing extractor test**

Create `crates/artifact/tests/extractor.rs`: test extracting text from UTF-8/Markdown text attachment and slicing into 200-character overlapping segments.

- [ ] **Step 2: Run artifact test**

Run: `cargo test -p artifact --test extractor`
Expected: FAIL because `artifact` crate is missing.

- [ ] **Step 3: Implement Artifact crate and Extractors**

Create `crates/artifact/Cargo.toml`, implement `Extractor` trait, `TextExtractor` (plaintext/markdown/org), `ImageMetadataExtractor` (dimensions/MIME info), and `SegmentSlicer`.

- [ ] **Step 4: Run artifact tests**

Run: `cargo test -p artifact`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/artifact
git commit -m "feat: add artifact crate and attachment text extractors"
```

---

### Task 4: Attachment and Extraction Job Application Services

**Files:**
- Create: `crates/application/src/attachment.rs`
- Modify: `crates/application/src/service.rs`
- Modify: `crates/application/src/lib.rs`
- Test: `crates/application/tests/attachment_app.rs`

**Interfaces:**
- Produces: `ApplicationService::add_attachment`, `ApplicationService::run_extraction`, `ApplicationService::query_segments`.

- [ ] **Step 1: Write failing attachment application test**

Create `crates/application/tests/attachment_app.rs`: test adding a text file attachment to space, running extraction job, and querying extracted segments.

- [ ] **Step 2: Run attachment application test**

Run: `cargo test -p application --test attachment_app`
Expected: FAIL because `add_attachment` is missing.

- [ ] **Step 3: Implement Attachment Application Services**

Implement `add_attachment` (stores blob, creates Attachment resource), `run_extraction` (runs extractor, slices text, stores segments in SQLite), and `query_segments`.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/application
git commit -m "feat: support attachment adding and extraction jobs in application service"
```

---

### Task 5: CLI and MCP Extensions for Attachments

**Files:**
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/mcp/src/server.rs`
- Test: `crates/cli/tests/attachment_cli.rs`
- Test: `crates/mcp/tests/attachment_mcp.rs`

**Interfaces:**
- Produces: CLI commands `attachment add`, `attachment list`, `attachment extract`; MCP tools `attachment_add`, `attachment_extract`, `query_segments`.

- [ ] **Step 1: Write failing CLI and MCP attachment tests**

Test CLI `notez attachment add --path /file.txt`, `notez attachment extract <REF>`, `notez attachment segments`; test corresponding MCP tools.

- [ ] **Step 2: Run CLI and MCP attachment tests**

Run: `cargo test -p cli --test attachment_cli` and `cargo test -p mcp --test attachment_mcp`
Expected: FAIL because `attachment` subcommands/tools are missing.

- [ ] **Step 3: Implement CLI subcommands and MCP tools**

Add clap `attachment` subcommand family; register `attachment_add`, `attachment_extract`, and `query_segments` tools in MCP stdio server.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cli crates/mcp
git commit -m "feat: expose attachment operations and extraction jobs over cli and mcp"
```

---

### Task 6: Acceptance Script, Documentation, and Final Verification

**Files:**
- Create: `scripts/acceptance-attachments.sh`
- Modify: `README.md`

**Interfaces:**
- Produces: `scripts/acceptance-attachments.sh` script testing attachment addition, extraction, segment querying, and rebuild consistency.

- [ ] **Step 1: Write attachments acceptance script**

The script adds image/text attachments, runs extraction, queries text segments, deletes SQLite index, rebuilds, and verifies output consistency.

- [ ] **Step 2: Run attachments acceptance script**

Run: `bash scripts/acceptance-attachments.sh`
Expected: PASS (`attachments acceptance: PASS`).

- [ ] **Step 3: Update README.md**

Document Content-Addressed Blob Storage, Attachment Resources, Extraction Jobs, text segments, and Attachment CLI/MCP commands.

- [ ] **Step 4: Final verification checks**

Run: `cargo fmt --all -- --check`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `cargo test --workspace`
Run: `bash scripts/acceptance-core.sh`
Run: `bash scripts/acceptance-rules.sh`
Run: `bash scripts/acceptance-federation.sh`
Run: `bash scripts/acceptance-attachments.sh`
Expected: All exit 0.

- [ ] **Step 5: Commit**

```bash
git add README.md scripts docs
git commit -m "test: add attachments acceptance script and update documentation"
```
