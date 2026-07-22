# Notez

Notez is a local-first, Org-mode-first knowledge federation engine written in Rust.

## Core Principles

1. **Files are authoritative:** Native Org files are the single source of truth. SQLite databases, knowledge graphs, and projections are completely disposable and rebuildable from original files.
2. **Org-mode first:** Org TODO states, properties, headings, and ID links are core domain primitives.
3. **Unified resource algebra:** All document nodes and headings use stable `ResourceRef` identifiers (`document:<ULID>` and `heading:<ULID>`). Paths and titles are not identities.
4. **Rebuildable projection:** Deleting `.notez/index.sqlite` is always safe after stopping Notez; running `notez space rebuild` reconstructs the projection byte-for-byte.

## Workspace Layout

```text
crates/
├── domain/       Resource algebra, types, selectors, and projection traits
├── document/     Lossless Org scanner (preserves raw content and byte spans)
├── storage/      Disposable SQLite projection store
├── application/  Application service (scan_native, query, resolve, read, rebuild)
├── cli/          CLI entry point (`notez`)
└── mcp/          Model Context Protocol stdio server
```

## Building and Installation

```bash
cargo build --release
```

The published binary is `target/release/notez`.

## CLI Usage

### Scan Native Space

Scans `.org` documents into the SQLite projection store:

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

### Rebuild Projection Index

Deletes index cache and rescans space from raw files:

```bash
notez --space /path/to/space space rebuild
```

### MCP Stdio Server

Exposes query, resolve, read, and inspect tools over stdio using newline-delimited JSON-RPC:

```bash
notez --space /path/to/space mcp serve
```

## Exit Codes

- `0`: Success
- `2`: Invalid request or argument error
- `3`: Resource not found
- `4`: Ambiguous resolve match
- `5`: Internal application failure or I/O error
