# Notez

Notez is a local-first, Org-mode-first knowledge federation engine written in Rust.

## Core Principles

1. **Files are authoritative:** Native Org and Markdown files are the single source of truth. SQLite databases, knowledge graphs, and projections are completely disposable and rebuildable from original files.
2. **Org & Markdown support:** Org TODO states, properties, headings, and ID links, plus Markdown frontmatter, HTML comment heading IDs, and WikiLinks (`[[...]]`).
3. **Unified resource algebra:** All document nodes, headings, and attachments use stable `ResourceRef` identifiers (`document:<ULID>`, `heading:<ULID>`, `attachment:<ULID>`). Paths and titles are not identities.
4. **Rebuildable projection:** Deleting `.notez/index.sqlite` is always safe after stopping Notez; running `notez space rebuild` or `notez source sync` reconstructs the projection byte-for-byte.

## Workspace Layout

```text
crates/
├── domain/       Resource algebra, types, selectors, rules, communities, and projection traits
├── document/     Lossless Org and Markdown scanners and security guards
├── storage/      Disposable SQLite projection store and Content-Addressed BlobStore
├── artifact/     Attachment extractors, segment slicer, recipe engine, and SKILL.md exporter
├── source/       SourceAdapters (Native, Git, Obsidian Vaults)
├── sync/         Manifests, object store, 3-way merge engine, and folder transport
├── application/  Application service (scan, query, resolve, read, attachment, sync, doctor)
├── cli/          CLI entry point (`notez`)
└── mcp/          Model Context Protocol stdio server
```

## Building and Installation

```bash
cargo build --release
```

The published binary is `target/release/notez`.

## Acceptance Test Suites

Run the full end-to-end release acceptance suite:

```bash
bash scripts/acceptance-release.sh
```

Sub-acceptance scripts:
- `scripts/acceptance-core.sh`
- `scripts/acceptance-rules.sh`
- `scripts/acceptance-federation.sh`
- `scripts/acceptance-attachments.sh`
- `scripts/acceptance-artifacts.sh`
- `scripts/acceptance-sync.sh`

## CLI Usage

### Scan Native Space

Scans `.org` and `.md` documents into the SQLite projection store:

```bash
notez --space /path/to/space scan
notez --space /path/to/space scan --json
```

### Query Resources

Search resources by kind or title substring:

```bash
notez --space /path/to/space query --kind heading --title-contains "sync" --json
```

### Resolve Reference

Resolves a ULID, exact `ResourceRef`, file path locator, or title query:

```bash
notez --space /path/to/space resolve "01J00000000000000000000001"
```

### Read Resource Details

Fetch full resource metadata and properties:

```bash
notez --space /path/to/space read heading:01J00000000000000000000001 --json
```
### Anytype Source Adapter & Writeback

Mount Anytype sources and execute atomic writeback mutations:

```bash
notez --space /path/to/space source add --id anytype_src --kind anytype --path /anytype/path --read-only
notez --space /path/to/space source writeback --id anytype_src --r-ref heading:01J00000000000000000000001 --payload "Updated Title"
```

### Block-Level References

Fine-grained paragraph and code block entities use `block:<ULID>` references:

```bash
notez --space /path/to/space query --kind block --json
```

### Relay/P2P Sync Transport

Sync via Relay server (the folder-based transport is the default):

```bash
notez --space /path/to/space sync relay --id anytype_src
```

### Inspect Rule Traces

```bash
notez --space /path/to/space inspect heading:01J00000000000000000000001 --rules --json
```

### Agenda View

Query scheduled or deadline items across space:

```bash
notez --space /path/to/space agenda --json
```

### Task State Transition

Atomic Org TODO state transition with CLOSED timestamp and LOGBOOK entry:

```bash
notez --space /path/to/space task transition heading:01J00000000000000000000001 --to DONE
```

### Attachment Operations & Text Extraction

Add attachments, run text/metadata extraction jobs, and query extracted segments:

```bash
notez --space /path/to/space attachment add --path /path/to/file.txt --mime text/plain
notez --space /path/to/space attachment extract attachment:01J00000000000000000000001
notez --space /path/to/space attachment segments attachment:01J00000000000000000000001 --json
```

### Community Management

Create and list communities bound by selectors:

```bash
notez --space /path/to/space community create --id dev_comm --name "DevSync" --title-contains "sync"
notez --space /path/to/space community list --json
```

### Derive Recipe Artifacts

Derive `summary`, `llms.txt`, `context-pack`, or `skill-ir` artifacts:

```bash
notez --space /path/to/space derive --community dev_comm --recipe summary --json
```

### Agent Skill Export

Export a standalone `SKILL.md` package containing prompt directives and references:

```bash
notez --space /path/to/space skill export --community dev_comm --description "DevSync Prompt Skill" --out /path/to/skill
```

### Source Management (Federation)

Mount external sources such as Obsidian Vaults or Git repositories:

```bash
notez --space /path/to/space source add --id vault_src --kind obsidian --path /path/to/vault --read-only
notez --space /path/to/space source list --json
notez --space /path/to/space source sync --json
```

### Folder Synchronization

Synchronize notes offline between devices via a shared folder (`heads/`, `manifests/`, `objects/`, `tombstones/`):

```bash
notez --space /path/to/space_a sync push --actor device_a --folder /path/to/shared
notez --space /path/to/space_b sync pull --actor device_b --folder /path/to/shared
notez --space /path/to/space_b sync conflicts --json
```

### Space Doctor & Diagnostics

Run space integrity checks (broken links, missing files, corrupt indexes):

```bash
notez --space /path/to/space space doctor --json
```

### Job Management & Artifact Freshness

List background jobs and check artifact freshness:

```bash
notez --space /path/to/space job list --json
notez --space /path/to/space artifact stale --json
```

### Rebuild Projection Index

Deletes index cache and rescans space from raw files:

```bash
notez --space /path/to/space space rebuild
```

### MCP Stdio Server

Exposes query, resolve, read, inspect, agenda, task transition, attachment, community, derive, skill export, source management, and sync tools over stdio using newline-delimited JSON-RPC:

```bash
notez --space /path/to/space mcp serve
```

## Exit Codes

- `0`: Success
- `2`: Invalid request or argument error
- `3`: Resource not found
- `4`: Ambiguous resolve match
- `5`: Internal application failure or I/O error
