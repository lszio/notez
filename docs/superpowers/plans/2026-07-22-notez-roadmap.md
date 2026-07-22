# Notez Implementation Roadmap

The approved design is split into independently testable plans:

1. **Core vertical slice** — Cargo workspace, resource algebra, lossless Org scanning, SQLite projection, application service, CLI, and MCP.
2. **Org behavior and rules** — TYPE/Trait/Schema, deterministic rules, configurable workflows, Task, agenda, and PARA.
3. **Markdown and federation** — Markdown profiles plus Native, Git, and Obsidian SourceAdapters and import.
4. **Attachments** — content-addressed blobs, extraction jobs, segments, sandbox limits, and initial extractors.
5. **Communities and Agent artifacts** — selectors, community candidates, recipes, summary, llms.txt, context-pack, Skill IR, and SKILL.md exporter.
6. **Folder synchronization** — manifests, encrypted objects, heads, tombstones, three-way merge, and conflict inspection.
7. **Hardening and release** — cross-platform fault injection, rebuild verification, security tests, packaging, and operational documentation.

Each plan must leave the repository in a runnable state and begins only after the preceding plan's public interfaces are accepted.
