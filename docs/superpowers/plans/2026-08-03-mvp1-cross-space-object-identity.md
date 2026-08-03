# MVP-1 跨空间对象身份与关系证据化实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付 [[file:../specs/2026-08-03-mvp1-cross-space-object-identity-design.org][MVP-1 跨空间对象身份与关系证据化 spec]] 的所有"做"项：新增 `ObjectId` 类型与 `derived_object_id` v1 派生函数；`SqliteProjection` 升级到 schema v2（含 `object_id` 列 + 关系四字段 + `PRAGMA user_version` 幂等迁移）；扫描时按 `content_hash + locator + position` 落 `object_id`；`ProjectionStore` 新增 `find_by_object`；`ResourceRelation` 落库带 `relation_type/direction/evidence_json/created_at/creator` 五字段；端到端跨 Space 同一文件 → 同一 `object_id` 测试。

**Architecture:** 在 `core::domain::resource` 新增 `ObjectId` 类型与 `derived_object_id` 派生函数；`Resource` 与 `ResourceRelation` 各加一字段；`ProjectionStore` trait 新增 `find_by_object` 默认空实现；`SqliteProjection` schema 走 v2，新增 `relation_type/direction/evidence_json/created_at/creator` 五列，`PRAGMA user_version` 记版本；扫描时算 `content_hash`（首 64 KiB + size 后 SHA-256）并落 `object_id`；迁移路径覆盖 v1 库（`object_id` 留空、关系 `direction='unknown' creator='legacy'`）。

**Tech Stack:** Rust edition 2024 / `rusqlite` / `serde` / `serde_json` / `sha2` / `ulid` / `chrono`（仅 RFC3339 字符串）。TDD；每个 task 先写失败测试。

**约定：**
- 每完成一个 task 立即 `cargo test -p notez_core` 跑通。
- 频繁 commit。
- 不动 `core::application` 业务逻辑；只在 `core::domain` 与 `core::storage` 上加字段与 trait 方法。
- 不做：跨 Space 写冲突、授权、Change 投递、notez:// scheme、客户端 UI、summary。

---

## File Structure

| 文件 | 变化 | 责任 |
|---|---|---|
| `core/src/domain/resource.rs` | 修改 | 新增 `ObjectId` 类型 + `derived_object_id` 函数 + `Resource.object_id` 字段 |
| `core/src/domain/link.rs` | 修改 | 新增 `RelationType` / `RelationDirection` 枚举 + `ResourceRelation` 五字段 + `ResolvedRelation` 五字段 |
| `core/src/domain/mod.rs` | 修改 | re-export 新类型 |
| `core/src/domain/query.rs` | 修改 | `ProjectionStore` trait 新增 `find_by_object` 默认实现 |
| `core/src/storage/sqlite.rs` | 修改 | schema v2 + `PRAGMA user_version` 迁移 + `find_by_object` 实现 + 关系五字段读写 |
| `core/src/lib.rs` | 修改 | re-export `ObjectId`, `derived_object_id` |
| `core/tests/api_storage.rs` | 新增测试 | 跨 Space 同一 `object_id` + schema 迁移 + 关系五字段 roundtrip |
| `core/src/document/markdown.rs` / `org.rs` | 修改 | 扫描时计算 `content_hash` 并落 `Resource.object_id` |
| `core/src/source/native.rs` | 修改 | 传递 `content_hash` 到 Resource 构造 |

---

## Task 1: `ObjectId` 类型与 `derived_object_id` v1

**Files:**
- Modify: `core/src/domain/resource.rs`
- Modify: `core/src/domain/mod.rs`
- Test: `core/src/domain/resource.rs` 内部 `#[cfg(test)] mod tests` 或新建 `core/tests/object_id.rs`

- [ ] **Step 1: 写失败测试（test/unit）**

在 `core/src/domain/resource.rs` 末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::resource::ResourceKind;

    #[test]
    fn object_id_is_stable_for_same_inputs() {
        let a = derived_object_id("hash-a", "notes/x.md", "h:0");
        let b = derived_object_id("hash-a", "notes/x.md", "h:0");
        assert_eq!(a, b);
    }

    #[test]
    fn object_id_changes_when_content_hash_changes() {
        let a = derived_object_id("hash-a", "notes/x.md", "h:0");
        let b = derived_object_id("hash-b", "notes/x.md", "h:0");
        assert_ne!(a, b);
    }

    #[test]
    fn object_id_changes_when_position_changes() {
        let a = derived_object_id("hash-a", "notes/x.md", "h:0");
        let b = derived_object_id("hash-a", "notes/x.md", "h:1");
        assert_ne!(a, b);
    }

    #[test]
    fn object_id_parse_and_display_roundtrip() {
        let id = derived_object_id("hash", "loc", "pos");
        let s = id.to_string();
        let parsed = ObjectId::parse(&s).expect("parse");
        assert_eq!(id, parsed);
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core --lib domain::resource::tests::object_id_is_stable_for_same_inputs
```

Expected: 编译失败（`ObjectId` / `derived_object_id` 不存在），提示 "cannot find type `ObjectId`" 或 "cannot find function `derived_object_id`"。

- [ ] **Step 3: 实现 `ObjectId` + `derived_object_id`**

在 `core/src/domain/resource.rs` `derived_id` 函数定义之后追加：

```rust
/// 跨 source 稳定的对象身份（spec §2.1）。
///
/// 同一正文 + 同一 locator + 同一 position 在不同 source 配置下得到相同
/// `ObjectId`。文件内容变更后 `ObjectId` 会变（spec §3 第 5 段接受的代价）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(Ulid);

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ObjectIdError {
    #[error("invalid ObjectId format: expected 26-char Crockford ULID, got `{0}`")]
    InvalidFormat(String),
    #[error("invalid ULID: {0}")]
    InvalidUlid(String),
}

impl ObjectId {
    pub fn new(id: Ulid) -> Self { Self(id) }
    pub fn as_ulid(&self) -> Ulid { self.0 }
    pub fn parse(s: &str) -> Result<Self, ObjectIdError> {
        let id = Ulid::from_str(s).map_err(|e| ObjectIdError::InvalidUlid(e.to_string()))?;
        Ok(Self(id))
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for ObjectId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ObjectId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        ObjectId::parse(&s).map_err(de::Error::custom)
    }
}

/// v1 派生：SHA-256( "notez-derived-object-id-v1" || content_hash || locator || position )。
///
/// `source_id` 不参与 — 同一文件被多 Space 扫描时 `content_hash` 来自同一正文，
/// 输出同一 `ObjectId`。
pub fn derived_object_id(
    content_hash: &str,
    locator: &str,
    position: &str,
) -> ObjectId {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(b"notez-derived-object-id-v1\0");
    hasher.update(content_hash.as_bytes());
    hasher.update(b"\0");
    hasher.update(locator.as_bytes());
    hasher.update(b"\0");
    hasher.update(position.as_bytes());
    let hash = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes[6..16].copy_from_slice(&hash[0..10]);
    let id = Ulid::from_bytes(bytes);
    ObjectId::new(id)
}
```

需要确认 `core::domain::resource` 已经 use `serde::{Serialize, Serializer, Deserialize, Deserializer, de}`（从顶部看已有）。如有缺失按错误补。

- [ ] **Step 4: 跑测试确认通过**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core --lib domain::resource::tests
```

Expected: 4 个测试通过。

- [ ] **Step 5: 在 `core/src/domain/mod.rs` re-export**

```rust
pub use resource::{
    derived_id, derived_object_id, ObjectId, ObjectIdError, Resource, ResourceKind, ResourceRef,
    ResourceRefError, ResourceRelation, SegmentRecord,
};
```

- [ ] **Step 6: Commit**

```bash
cd /home/lszio/Projects/notez
git add core/src/domain/resource.rs core/src/domain/mod.rs
git commit -m "feat(domain): add ObjectId type and derived_object_id v1

Introduce a cross-Space stable object identity layer (spec §2.1-2.2).
ObjectId is derived from (content_hash, locator, position) via SHA-256;
source_id is intentionally NOT part of the input so the same file
scanned by two Spaces produces the same ObjectId. The existing
derived_id v1 and ResourceRef contract are unchanged (0.3 identity
stability preserved)."
```

---

## Task 2: `Resource` 增 `object_id` 字段

**Files:**
- Modify: `core/src/domain/resource.rs`
- Modify: 任何构造 `Resource { ... }` 的地方（grep）

- [ ] **Step 1: 找所有 `Resource {` 构造点**

```bash
cd /home/lszio/Projects/notez
grep -rn "Resource {" core/src/ | grep -v "ResourceRef::" | grep -v "ResourceKind::" | head -30
```

- [ ] **Step 2: 在 `Resource` 结构体加 `object_id` 字段**

在 `core/src/domain/resource.rs` 的 `Resource` 结构体上：

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resource {
    #[serde(rename = "ref")]
    pub r#ref: ResourceRef,
    pub kind: ResourceKind,
    pub title: String,
    pub revision: String,
    pub source_id: String,
    pub locator: String,
    pub properties: BTreeMap<String, String>,
    /// 跨 source 稳定的对象身份（spec §2.1）。同一正文 + 同一 locator +
    /// 同一 position 在不同 source 下得同一 `ObjectId`。
    #[serde(default)]
    pub object_id: ObjectId,
}
```

- [ ] **Step 3: 给所有 `Resource { ... }` 构造点补 `object_id`**

为每处构造加 `object_id: derived_object_id(content_hash, locator, position)`，其中 `content_hash` 由调用方持有（由 Task 6 的扫描路径提供）。对于不在扫描路径上的构造点（如 service 的 upsert 路径），加占位 `object_id: derived_object_id("", "", "")` — 这只是过渡，下游 Task 6 完成后所有真正进入数据库的 Resource 都带正确 hash。

更安全的做法：先在所有构造点用 `derived_object_id("", "<locator>", "<position>")` 占位，确保编译通过；Task 6 改扫描路径时再传真实 hash。

实施时按 grep 出来的实际行号修。

- [ ] **Step 4: 跑编译**

```bash
cd /home/lszio/Projects/notez
cargo check --workspace --all-targets
```

Expected: 通过。`#[serde(default)]` 保证旧 JSON 反序列化时 `object_id` 默认值（由 derive 提供时使用 `Default::default()` — `ObjectId` 是 `Copy + Default` via Ulid 的 `default()` = timestamp 0 + random 0，这没问题；数据库写入永远走 `replace_source` 提供新值）。

- [ ] **Step 5: 跑既有测试**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core
```

Expected: 全部通过（除可能因 `object_id` 缺省的 mutation_safety / capability 等测试需要小幅更新；按需补 `object_id` 字段）。

- [ ] **Step 6: Commit**

```bash
cd /home/lszio/Projects/notez
git add core/
git commit -m "feat(domain): add object_id field to Resource

Carry the cross-Space stable ObjectId on every Resource so that the
same file scanned by multiple Spaces produces matching rows in each
Space's projection. Scanning paths compute content_hash and call
derived_object_id; placeholder hashes are used where the scan path
is out of scope for this task (will be replaced in Task 6)."
```

---

## Task 3: `ResourceRelation` 关系证据五字段

**Files:**
- Modify: `core/src/domain/link.rs`
- Modify: `core/src/domain/mod.rs`
- Modify: 任何构造 `ResourceRelation { ... }` / `ResolvedRelation { ... }` 的地方（grep）

- [ ] **Step 1: 写失败测试**

在 `core/src/domain/link.rs` 末尾追加：

```rust
#[cfg(test)]
mod relation_evidence_tests {
    use super::*;
    use crate::domain::resource::ResourceRef;

    #[test]
    fn relation_type_default_is_references() {
        let t = RelationType::default();
        assert_eq!(t, RelationType::References);
    }

    #[test]
    fn direction_default_is_unknown() {
        let d = RelationDirection::default();
        assert_eq!(d, RelationDirection::Unknown);
    }

    #[test]
    fn resource_relation_serializes_with_evidence_fields() {
        let rel = ResourceRelation {
            source_ref: ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap(),
            relation: "references".to_string(),
            target_ref: ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap(),
            relation_type: RelationType::References,
            direction: RelationDirection::Forward,
            evidence_json: serde_json::json!({"source_id": "notes", "span_line": 1}),
            created_at: "2026-08-03T00:00:00Z".to_string(),
            creator: "scan".to_string(),
        };
        let j = serde_json::to_string(&rel).unwrap();
        assert!(j.contains("\"relation_type\":\"references\""));
        assert!(j.contains("\"direction\":\"forward\""));
        assert!(j.contains("\"created_at\":\"2026-08-03T00:00:00Z\""));
        assert!(j.contains("\"creator\":\"scan\""));
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core --lib domain::link::relation_evidence_tests
```

Expected: 编译失败（`RelationType` / `RelationDirection` / 五字段未定义）。

- [ ] **Step 3: 实现新枚举与新字段**

在 `core/src/domain/link.rs` 顶部 import 之后、`LinkTarget` 之前插入：

```rust
/// 关系类型（spec §3 Relation）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    /// 普通引用链接：`source -> target`（默认）
    References,
    /// 嵌入/内联：把 target 的内容嵌入到 source
    Embeds,
    /// 反向链接（展示层落库时使用，未来由 0.6 同步层写）
    Backlink,
    /// 用户/Adapter 扩展类型
    Custom(String),
}

impl Default for RelationType {
    fn default() -> Self { Self::References }
}

impl fmt::Display for RelationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::References => f.write_str("references"),
            Self::Embeds => f.write_str("embeds"),
            Self::Backlink => f.write_str("backlink"),
            Self::Custom(s) => write!(f, "{s}"),
        }
    }
}

/// 关系方向（spec §2.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationDirection {
    /// 落库时自然值：`source_ref -> target_ref` 由 source 出发
    Forward,
    /// 由 target_ref 指向 source_ref（展示层查询、0.6 冲突标记）
    Backward,
    /// 无法判定方向（v1 迁移行的默认）
    Unknown,
}

impl Default for RelationDirection {
    fn default() -> Self { Self::Unknown }
}

impl fmt::Display for RelationDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Forward => f.write_str("forward"),
            Self::Backward => f.write_str("backward"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}
```

修改 `ResourceRelation`：

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRelation {
    pub source_ref: ResourceRef,
    pub relation: String,
    pub target_ref: ResourceRef,
    #[serde(default)]
    pub relation_type: RelationType,
    #[serde(default)]
    pub direction: RelationDirection,
    #[serde(default = "default_evidence_json")]
    pub evidence_json: serde_json::Value,
    #[serde(default = "default_created_at")]
    pub created_at: String,
    #[serde(default = "default_creator")]
    pub creator: String,
}

fn default_evidence_json() -> serde_json::Value { serde_json::json!({}) }
fn default_created_at() -> String { String::new() }
fn default_creator() -> String { "legacy".to_string() }
```

修改 `ResolvedRelation`（同样补五字段；默认值与上同，但 `direction` 在落库路径 Task 6 处会显式置 `Forward`）：

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRelation {
    pub source_ref: ResourceRef,
    pub target_ref: ResourceRef,
    pub target: LinkTarget,
    pub status: ResolutionStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<ResourceRef>,
    #[serde(default)]
    pub relation_type: RelationType,
    #[serde(default)]
    pub direction: RelationDirection,
    #[serde(default = "default_evidence_json")]
    pub evidence_json: serde_json::Value,
    #[serde(default = "default_created_at")]
    pub created_at: String,
    #[serde(default = "default_creator")]
    pub creator: String,
}
```

- [ ] **Step 4: 给所有 `ResourceRelation { ... }` 构造点补字段**

```bash
cd /home/lszio/Projects/notez
grep -rn "ResourceRelation {" core/src/
grep -rn "ResolvedRelation {" core/src/
```

为每处构造补五字段占位（`relation_type: RelationType::References, direction: RelationDirection::Unknown, evidence_json: serde_json::json!({}), created_at: String::new(), creator: "scan".to_string()`，扫描路径 Task 6 处会补 `direction: RelationDirection::Forward` 与真实 evidence）。

- [ ] **Step 5: 跑测试**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core --lib domain::link
```

Expected: 通过。

- [ ] **Step 6: 在 `core/src/domain/mod.rs` re-export**

```rust
pub use link::{
    LinkOccurrence, LinkTarget, RelationDirection, RelationType, ResolvedRelation,
    ResolutionStatus, ResourceAddress, TextSpan,
};
```

- [ ] **Step 7: Commit**

```bash
cd /home/lszio/Projects/notez
git add core/src/domain/link.rs core/src/domain/mod.rs
git commit -m "feat(domain): add relation type/direction/evidence fields

Add RelationType, RelationDirection enums and the
type/direction/evidence_json/created_at/creator fields to
ResourceRelation and ResolvedRelation (spec §2.3, §3.3). Defaults
preserve backward compatibility (v1 deserialization still works via
#[serde(default)]); scan path will fill in direction=Forward and
real evidence in Task 6."
```

---

## Task 4: `ProjectionStore::find_by_object` trait 方法

**Files:**
- Modify: `core/src/domain/query.rs`
- Modify: `core/src/storage/sqlite.rs` (impl)

- [ ] **Step 1: 在 `ProjectionStore` trait 加方法**

在 `core/src/domain/query.rs` 的 trait 定义里加（trait 末尾）：

```rust
/// 跨 Space 同一对象查询：返回所有 `object_id` 匹配的资源。
/// 默认空实现（测试替身无需关心），`SqliteProjection` 必须 override。
fn find_by_object(
    &self,
    object_id: crate::domain::ObjectId,
) -> Result<Vec<crate::domain::Resource>, Self::Error> {
    Ok(Vec::new())
}
```

- [ ] **Step 2: 跑编译确认 trait 默认实现可工作**

```bash
cd /home/lszio/Projects/notez
cargo check --workspace --all-targets
```

Expected: 通过（默认实现返回 `Ok(Vec::new())`）。

- [ ] **Step 3: 跑既有测试**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core --lib
```

Expected: 通过。

- [ ] **Step 4: 在 `lib.rs` re-export `ObjectId`**

修改 `core/src/lib.rs` 的 re-export：

```rust
pub use domain::{
    Community, CommunityCandidate, CommunitySelector, InspectResult, LinkDiagnostic,
    LinkOccurrence, LinkTarget, Projection, ProjectionStore, QueryPage, Resource, ResourceKind,
    ResourceRef, ResourceRefError, ResourceRelation, ResolutionStatus, ResolvedRelation,
    RelationDirection, RelationType, Rule, RuleEngine, RuleKind, RuleTrace, Selector, TextSpan,
    derived_id, derived_object_id, ObjectId, ObjectIdError,
};
```

- [ ] **Step 5: Commit**

```bash
cd /home/lszio/Projects/notez
git add core/src/domain/query.rs core/src/lib.rs
git commit -m "feat(domain): add find_by_object to ProjectionStore

Add the cross-Space aggregation query entry point (spec §5) as a
trait method with a default no-op implementation. Real projections
(SqliteProjection) override it in Task 5."
```

---

## Task 5: `SqliteProjection` schema v2 + 迁移 + 关系五字段 + `find_by_object`

**Files:**
- Modify: `core/src/storage/sqlite.rs`

- [ ] **Step 1: 写失败测试**

新建 `core/tests/api_storage_object_id.rs`：

```rust
use notez_core::domain::{
    derived_object_id, ObjectId, RelationDirection, RelationType, Resource, ResourceKind,
    ResourceRef, ResourceRelation,
};
use notez_core::storage::SqliteProjection;
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn find_by_object_returns_matching_resources() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("proj.sqlite");
    let mut store = SqliteProjection::open(&db_path).unwrap();

    let hash = "abc123";
    let object_id = derived_object_id(hash, "notes/x.md", "h:0");
    let ref_a = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let res_a = Resource {
        r#ref: ref_a,
        kind: ResourceKind::Heading,
        title: "X".to_string(),
        revision: "r1".to_string(),
        source_id: "src_a".to_string(),
        locator: "notes/x.md".to_string(),
        properties: Default::default(),
        object_id,
    };
    store.upsert_resource(&res_a).unwrap();

    let found = store.find_by_object(object_id).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].r#ref, ref_a);
    assert_eq!(found[0].object_id, object_id);
}

#[test]
fn schema_v1_to_v2_migration_backfills_object_id_and_relation_fields() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("legacy.sqlite");

    // Hand-build a v1 schema (no object_id, no relation evidence columns)
    {
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE resources (
                ref TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                revision TEXT NOT NULL,
                source_id TEXT NOT NULL,
                locator TEXT NOT NULL,
                properties_json TEXT NOT NULL
            );
            CREATE TABLE relations (
                source_ref TEXT NOT NULL,
                relation TEXT NOT NULL,
                target_ref TEXT NOT NULL,
                source_id TEXT NOT NULL
            );
            ",
        )
        .unwrap();
        let ref_str = "heading:01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string();
        conn.execute(
            "INSERT INTO resources (ref, kind, title, revision, source_id, locator, properties_json)
             VALUES (?1, 'heading', 'X', 'r1', 'src_a', 'notes/x.md', '{}')",
            rusqlite::params![ref_str],
        )
        .unwrap();
    }

    // Open through SqliteProjection — this should run the v1→v2 migration
    let store = SqliteProjection::open(&db_path).unwrap();

    // user_version should be 2
    let conn = Connection::open(&db_path).unwrap();
    let user_version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(user_version, 2);

    // resources.object_id should be present (may be empty for v1 backfill
    // because we don't have the file content to recompute)
    let col_present: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('resources') WHERE name = 'object_id'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(col_present, 1, "object_id column should be added by migration");

    // relations should have direction/creator/created_at/evidence_json columns
    for col in ["direction", "creator", "created_at", "evidence_json", "relation_type"] {
        let n: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM pragma_table_info('relations') WHERE name = '{col}'"
                ),
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "{col} column should be present after migration");
    }
}

#[test]
fn relation_evidence_round_trip_through_replace_source() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("rel.sqlite");
    let mut store = SqliteProjection::open(&db_path).unwrap();

    let src_ref = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let tgt_ref = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap();
    let rel = ResourceRelation {
        source_ref: src_ref,
        relation: "references".to_string(),
        target_ref: tgt_ref,
        relation_type: RelationType::References,
        direction: RelationDirection::Forward,
        evidence_json: serde_json::json!({"source_id": "src_a", "span_line": 1}),
        created_at: "2026-08-03T00:00:00Z".to_string(),
        creator: "scan".to_string(),
    };
    store
        .replace_source("src_a", vec![], vec![rel], vec![])
        .unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let dir_str: String = conn
        .query_row(
            "SELECT direction FROM relations WHERE source_id = 'src_a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(dir_str, "forward");
    let creator: String = conn
        .query_row(
            "SELECT creator FROM relations WHERE source_id = 'src_a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(creator, "scan");
}
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core --test api_storage_object_id
```

Expected: 编译或断言失败（`find_by_object` 在 SqliteProjection 上不存在 / `object_id` 列未添加 / 关系五字段列不存在）。

- [ ] **Step 3: 重写 `init_schema` 加 v2 列与 `user_version` 记录**

在 `core/src/storage/sqlite.rs` 中，修改 `init_schema`：

```rust
const SCHEMA_VERSION: u32 = 2;

fn init_schema(&mut self) -> Result<(), StorageError> {
    // Ensure base tables exist (v1 columns)
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

    // v1 → v2 migration: add object_id + relation evidence columns
    self.migrate_to_v2()?;
    Ok(())
}

fn migrate_to_v2(&mut self) -> Result<(), StorageError> {
    let current: i64 = self
        .conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if current >= 2 {
        return Ok(());
    }

    // Add object_id column to resources (idempotent via try/catch on duplicate)
    let _ = self
        .conn
        .execute("ALTER TABLE resources ADD COLUMN object_id TEXT", []);
    let _ = self
        .conn
        .execute("ALTER TABLE resources ADD COLUMN content_hash TEXT", []);

    // Add relation evidence columns to relations
    let _ = self.conn.execute(
        "ALTER TABLE relations ADD COLUMN relation_type TEXT NOT NULL DEFAULT 'references'",
        [],
    );
    let _ = self.conn.execute(
        "ALTER TABLE relations ADD COLUMN direction TEXT NOT NULL DEFAULT 'unknown'",
        [],
    );
    let _ = self.conn.execute(
        "ALTER TABLE relations ADD COLUMN evidence_json TEXT NOT NULL DEFAULT '{}'",
        [],
    );
    let _ = self.conn.execute(
        "ALTER TABLE relations ADD COLUMN created_at TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = self.conn.execute(
        "ALTER TABLE relations ADD COLUMN creator TEXT NOT NULL DEFAULT 'legacy'",
        [],
    );

    // Add same columns to resolved_relations
    let _ = self.conn.execute(
        "ALTER TABLE resolved_relations ADD COLUMN relation_type TEXT NOT NULL DEFAULT 'references'",
        [],
    );
    let _ = self.conn.execute(
        "ALTER TABLE resolved_relations ADD COLUMN direction TEXT NOT NULL DEFAULT 'unknown'",
        [],
    );
    let _ = self.conn.execute(
        "ALTER TABLE resolved_relations ADD COLUMN evidence_json TEXT NOT NULL DEFAULT '{}'",
        [],
    );
    let _ = self.conn.execute(
        "ALTER TABLE resolved_relations ADD COLUMN created_at TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = self.conn.execute(
        "ALTER TABLE resolved_relations ADD COLUMN creator TEXT NOT NULL DEFAULT 'legacy'",
        [],
    );

    // Add indexes
    let _ = self
        .conn
        .execute("CREATE INDEX IF NOT EXISTS idx_resources_object ON resources(object_id)", []);
    let _ = self.conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_relations_target ON relations(target_ref)",
        [],
    );

    // Bump user_version
    self.conn.execute(&format!("PRAGMA user_version = 2"), [])?;
    Ok(())
}
```

- [ ] **Step 4: 更新 `replace_source` 写 `object_id` 和关系五字段**

修改 `replace_source` 中的 `resources` INSERT：

```rust
"INSERT INTO resources (ref, kind, title, revision, source_id, locator, properties_json, object_id, content_hash)
 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
```

并在执行 params 加 `res.object_id.to_string()` 与可选的 `content_hash` 占位（先放空字符串）。

修改 relations INSERT：

```rust
"INSERT INTO relations (source_ref, relation, target_ref, source_id, relation_type, direction, evidence_json, created_at, creator)
 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
```

执行 params 加 `rel.relation_type.to_string()` / `rel.direction.to_string()` / `serde_json::to_string(&rel.evidence_json)?` / `rel.created_at.clone()` / `rel.creator.clone()`。

- [ ] **Step 5: 更新 `get` / `query` / `upsert_resource` 读 `object_id`**

`get` 的 SELECT 加 `object_id`，row 加一列，反序列化的 `Resource` 构造补 `object_id: ObjectId::parse(&s).unwrap_or_default()`（v1 旧库反序列化时该列为空，容错 default）。

`query` 同样处理。

`upsert_resource` 的 INSERT 加 `object_id` 列；ON CONFLICT 也更新 `object_id = excluded.object_id`。

- [ ] **Step 6: 实现 `find_by_object`**

在 `impl ProjectionStore for SqliteProjection` 内追加：

```rust
fn find_by_object(
    &self,
    object_id: ObjectId,
) -> Result<Vec<Resource>, StorageError> {
    let mut stmt = self.conn.prepare(
        "SELECT ref, kind, title, revision, source_id, locator, properties_json, object_id
           FROM resources WHERE object_id = ?1",
    )?;
    let rows = stmt.query_map(params![object_id.to_string()], |row| {
        let r_ref_str: String = row.get(0)?;
        let kind_str: String = row.get(1)?;
        let title: String = row.get(2)?;
        let revision: String = row.get(3)?;
        let source_id: String = row.get(4)?;
        let locator: String = row.get(5)?;
        let properties_json: String = row.get(6)?;
        let object_id_str: Option<String> = row.get(7)?;
        Ok((
            r_ref_str,
            kind_str,
            title,
            revision,
            source_id,
            locator,
            properties_json,
            object_id_str,
        ))
    })?;
    let mut items = Vec::new();
    for r in rows {
        let (r_ref_str, kind_str, title, revision, source_id, locator, properties_json, object_id_str) = r?;
        let r_ref = ResourceRef::parse(&r_ref_str)
            .map_err(|e| StorageError::InvalidData(format!("invalid ref in DB: {e}")))?;
        let properties: BTreeMap<String, String> = serde_json::from_str(&properties_json)?;
        let object_id = object_id_str
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(ObjectId::parse)
            .transpose()
            .map_err(|e| StorageError::InvalidData(format!("invalid object_id: {e}")))?
            .unwrap_or_default();
        items.push(Resource {
            r#ref: r_ref,
            kind: r_ref.kind(),
            title,
            revision,
            source_id,
            locator,
            properties,
            object_id,
        });
    }
    Ok(items)
}
```

- [ ] **Step 7: 跑测试**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core --test api_storage_object_id
```

Expected: 3 个新测试通过；既有 `api_storage` / `mutation_safety` / `use_case_*` 测试不受影响（migration 是 ADD COLUMN 兼容）。

- [ ] **Step 8: 跑整个 workspace 测试**

```bash
cd /home/lszio/Projects/notez
cargo test --workspace
```

Expected: 全部通过。

- [ ] **Step 9: Commit**

```bash
cd /home/lszio/Projects/notez
git add core/src/storage/sqlite.rs core/tests/api_storage_object_id.rs
git commit -m "feat(storage): schema v2 with object_id and relation evidence

Bump SqliteProjection to schema v2 (PRAGMA user_version). Add
object_id + content_hash columns to resources; add
relation_type/direction/evidence_json/created_at/creator columns to
relations and resolved_relations. Migration is idempotent: opening
a v1 database adds the new columns and bumps user_version, leaving
existing rows intact (object_id empty, relations direction='unknown'
creator='legacy'). Implement find_by_object to enable cross-Space
aggregation queries on the same ObjectId."
```

---

## Task 6: 扫描路径 — 计算 `content_hash` 并落 `object_id`

**Files:**
- Modify: `core/src/document/markdown.rs` (扫到 heading/block)
- Modify: `core/src/document/org.rs` (扫到 heading/block)
- Modify: `core/src/source/native.rs` (OrgParser/MarkdownParser 构造 Resource 时)
- Modify: `core/src/source/protocol.rs` (如 `ParsedEntity`/`Resource` 链路涉及)

- [ ] **Step 1: 写失败测试（端到端跨 Space）**

新建 `core/tests/api_cross_space_identity.rs`：

```rust
use notez_core::domain::{
    derived_object_id, ObjectId, ResourceKind,
};
use notez_core::source::adapter::{ComposedSourceAdapter, SourceConfig, SourceKind};
use notez_core::source::registry::SourceRegistry;
use notez_core::storage::SqliteProjection;
use notez_core::source::FormatParser;
use std::path::PathBuf;
use tempfile::tempdir;

fn hash_first_64k(path: &std::path::Path) -> String {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path).unwrap();
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();
    let size = buf.len();
    let truncated: &[u8] = if size > 64 * 1024 { &buf[..64 * 1024] } else { &buf };
    let mut hasher = Sha256::new();
    hasher.update(&(size as u64).to_le_bytes());
    hasher.update(truncated);
    format!("{:x}", hasher.finalize())
}

#[test]
fn two_spaces_scanning_same_file_produce_same_object_id() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("shared.md");
    std::fs::write(&file_path, b"# heading\n\nbody\n").unwrap();

    // Build two native source adapters with different source_ids pointing
    // at the same file.
    let cfg_a = SourceConfig {
        id: "src_a".to_string(),
        kind: SourceKind::Native,
        path: dir.path().to_path_buf(),
        include_paths: vec![],
        exclude_paths: vec![],
        read_only: true,
    };
    let cfg_b = SourceConfig {
        id: "src_b".to_string(),
        kind: SourceKind::Native,
        path: dir.path().to_path_buf(),
        include_paths: vec![],
        exclude_paths: vec![],
        read_only: true,
    };

    let hash = hash_first_64k(&file_path);

    let registry = SourceRegistry::with_builtins();
    let mut adapter_a = ComposedSourceAdapter::new(registry.clone());
    adapter_a.add_source(cfg_a).unwrap();
    let mut adapter_b = ComposedSourceAdapter::new(registry);
    adapter_b.add_source(cfg_b).unwrap();

    let entities_a = adapter_a.scan().unwrap();
    let entities_b = adapter_b.scan().unwrap();

    // Both should have parsed the same .md file
    assert!(!entities_a.is_empty());
    assert!(!entities_b.is_empty());

    // Compute expected object_id (locator is source-relative path)
    let locator = "shared.md";
    let expected_doc = derived_object_id(&hash, locator, "doc:0");

    // Find the Document resource in each
    let find_doc = |entities: &[_]| -> Option<Resource> {
        entities
            .iter()
            .flat_map(|e| e.resources.iter())
            .find(|r| matches!(r.kind, ResourceKind::Document))
            .cloned()
    };
    let doc_a = find_doc(&entities_a).expect("space A document");
    let doc_b = find_doc(&entities_b).expect("space B document");

    // object_id matches across spaces
    assert_eq!(doc_a.object_id, doc_b.object_id);
    assert_eq!(doc_a.object_id, expected_doc);

    // but ResourceRef is different (different source_id)
    assert_ne!(doc_a.r#ref, doc_b.r#ref);
}
```

注：本测试的 `ComposedSourceAdapter::scan` / `SourceConfig` / `Resource` 形态以 `core/src/source/adapter.rs` 与 `core/src/source/native.rs` 当前实现为准。Task 6 实施时按实际 `SourceConfig` 字段名与 `scan()` 返回类型做小幅调整（保留测试意图不变）。`hash_first_64k` 函数按 `core/src/document::content_hash`（Task 6 实现）抽出后改为单次调用；`read_to_end` 路径按实际 API 调整。

- [ ] **Step 2: 跑测试确认失败**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core --test api_cross_space_identity
```

Expected: 失败（扫描产出的 Resource 没有正确 `object_id`，或 `ComposedSourceAdapter::scan` 签名不一致）。

- [ ] **Step 3: 实现 `content_hash` 辅助函数**

在 `core/src/document/mod.rs`（或新建 `core/src/document/content_hash.rs`）加入：

```rust
/// SHA-256 of a file's content. Truncates to first 64 KiB and mixes in
/// the full size to keep large-file detection stable.
pub fn content_hash_of_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let size = buf.len();
    let truncated: &[u8] = if size > 64 * 1024 { &buf[..64 * 1024] } else { &buf };
    let mut hasher = Sha256::new();
    hasher.update(&(size as u64).to_le_bytes());
    hasher.update(truncated);
    Ok(format!("{:x}", hasher.finalize()))
}

/// SHA-256 of an in-memory byte range (used for Heading/Block within a
/// already-loaded document).
pub fn content_hash_of_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
```

并在 `core/src/document/mod.rs` 公开 re-export（`pub use content_hash::{content_hash_of_bytes, content_hash_of_file};`）。

- [ ] **Step 4: 在 `OrgParser` / `MarkdownParser` 构造 `Resource` 时算 `content_hash` 并填 `object_id`**

读 `core/src/source/native.rs` 与 `core/src/document/org.rs`、`markdown.rs` 的具体 `Resource` 构造点。在每个 Heading/Block/Document 构造点：

- Document：`content_hash = content_hash_of_file(file_path)`；`object_id = derived_object_id(&content_hash, locator, "doc:0")`。
- Heading：在已读取的 `raw_text` 上算 `content_hash_of_bytes(text.as_bytes())`；`object_id = derived_object_id(&content_hash, locator, &format!("h:{idx}"))`。
- Block：同上，`position` 用 block index 或行号。

如某条扫描路径不持有文件/字节范围（无法计算 hash），回退到 `derived_object_id("", locator, position)` 占位 — 测试覆盖范围内的真实路径必须走 `content_hash` 分支。

- [ ] **Step 5: 跑测试**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core --test api_cross_space_identity
```

Expected: 通过。

- [ ] **Step 6: 跑整个 workspace**

```bash
cd /home/lszio/Projects/notez
cargo test --workspace
```

Expected: 全部通过。

- [ ] **Step 7: Commit**

```bash
cd /home/lszio/Projects/notez
git add core/src/document/ core/src/source/
git commit -m "feat(scan): compute content_hash and populate object_id during scanning

Thread content_hash (SHA-256 of file or in-memory byte range) through
the document scanners (Org / Markdown) and source adapters so that
each scanned Resource carries the correct ObjectId. Same file scanned
by two different source_ids now produces the same object_id and
different ResourceRefs, enabling cross-Space aggregation queries
(spec §3, §10 scenario 1)."
```

---

## Task 7: 落库路径补关系 evidence

**Files:**
- Modify: `core/src/application/link_resolution.rs` (ResolvedRelation 构造点)
- Modify: `core/src/application/service.rs` (ResourceRelation 构造点)
- Modify: `core/src/storage/sqlite.rs` (`replace_resolved_relations` 写五字段)

- [ ] **Step 1: 写失败测试**

在 `core/tests/api_storage_object_id.rs` 末尾追加（与 Task 5 测试同文件）：

```rust
#[test]
fn resolved_relation_evidence_persists_with_direction_forward() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("resolved.sqlite");
    let mut store = SqliteProjection::open(&db_path).unwrap();

    let src_ref = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let tgt_ref = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap();
    let rel = ResolvedRelation {
        source_ref: src_ref,
        target_ref: tgt_ref,
        target: notez_core::domain::LinkTarget::id("01ARZ3NDEKTSV4RRFFQ69G5FAW", None),
        status: notez_core::domain::ResolutionStatus::Resolved,
        candidates: vec![],
        relation_type: notez_core::domain::RelationType::References,
        direction: notez_core::domain::RelationDirection::Forward,
        evidence_json: serde_json::json!({"source_id": "src_a", "span_line": 5, "rule": "default_profile:markdown"}),
        created_at: "2026-08-03T00:00:00Z".to_string(),
        creator: "scan".to_string(),
    };
    store.replace_resolved_relations("src_a", vec![rel]).unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let dir_str: String = conn
        .query_row(
            "SELECT direction FROM resolved_relations WHERE source_id = 'src_a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(dir_str, "forward");
    let creator: String = conn
        .query_row(
            "SELECT creator FROM resolved_relations WHERE source_id = 'src_a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(creator, "scan");
}
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core --test api_storage_object_id::resolved_relation_evidence_persists_with_direction_forward
```

Expected: 失败（`replace_resolved_relations` 还没写五字段列）。

- [ ] **Step 3: 在 `replace_resolved_relations` 写五字段**

修改 `core/src/storage/sqlite.rs` 中 `replace_resolved_relations` 的 INSERT：

```rust
"INSERT INTO resolved_relations (source_ref, target_ref, target_json, status, candidates_json, source_id, relation_type, direction, evidence_json, created_at, creator)
 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
```

params 加 `rel.relation_type.to_string()` / `rel.direction.to_string()` / `serde_json::to_string(&rel.evidence_json)?` / `rel.created_at.clone()` / `rel.creator.clone()`。

- [ ] **Step 4: 在 `link_resolution` 构造 `ResolvedRelation` 时补 evidence**

在 `core/src/application/link_resolution.rs` 中 `ResolvedRelation` 构造点补：

```rust
let now = chrono::Utc::now().to_rfc3339();
let evidence = serde_json::json!({
    "source_id": source_id,
    "span_line": occ.span.line,
    "span_col_start": occ.span.col_start,
    "span_col_end": occ.span.col_end,
    "rule": format!("default_profile:{}", /* format name */),
});
relation_type: RelationType::References,
direction: RelationDirection::Forward,
evidence_json: evidence,
created_at: now,
creator: "scan".to_string(),
```

- [ ] **Step 5: 跑测试**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core --test api_storage_object_id
```

Expected: 全部通过。

- [ ] **Step 6: 跑整个 workspace**

```bash
cd /home/lszio/Projects/notez
cargo test --workspace
```

Expected: 全部通过。

- [ ] **Step 7: Commit**

```bash
cd /home/lszio/Projects/notez
git add core/src/application/link_resolution.rs core/src/storage/sqlite.rs core/tests/api_storage_object_id.rs
git commit -m "feat(relations): persist relation evidence at write time

Fill in relation_type/direction/evidence_json/created_at/creator on
every ResolvedRelation produced by LinkResolver (direction=Forward,
creator=scan, evidence with source_id + span). The matching SQLite
INSERT now writes all five columns. v1 migration rows remain
direction='unknown' creator='legacy' evidence_json='{}'."
```

---

## Task 8: 端到端跨 Space 扫描 + 关系证据 roundtrip

**Files:**
- 新增: `core/tests/api_cross_space_full.rs`（合并 Task 6 + Task 7 的端到端）

- [ ] **Step 1: 写测试**

```rust
use notez_core::domain::{ResolutionStatus, ResourceKind};
use notez_core::source::adapter::{ComposedSourceAdapter, SourceConfig, SourceKind};
use notez_core::source::registry::SourceRegistry;
use notez_core::storage::SqliteProjection;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn cross_space_scan_then_aggregate_by_object_id() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("x.md");
    std::fs::write(&file_path, b"# Title\n\nsee [other](other.md)\n").unwrap();

    let cfg_a = SourceConfig {
        id: "space_a".to_string(),
        kind: SourceKind::Native,
        path: dir.path().to_path_buf(),
        include_paths: vec![],
        exclude_paths: vec![],
        read_only: true,
    };
    let cfg_b = SourceConfig {
        id: "space_b".to_string(),
        kind: SourceKind::Native,
        path: dir.path().to_path_buf(),
        include_paths: vec![],
        exclude_paths: vec![],
        read_only: true,
    };

    let registry = SourceRegistry::with_builtins();
    let mut adapter_a = ComposedSourceAdapter::new(registry.clone());
    adapter_a.add_source(cfg_a).unwrap();
    let mut adapter_b = ComposedSourceAdapter::new(registry);
    adapter_b.add_source(cfg_b).unwrap();

    let entities_a = adapter_a.scan().unwrap();
    let entities_b = adapter_b.scan().unwrap();

    // Persist into separate projections
    let db_a = dir.path().join("a.sqlite");
    let db_b = dir.path().join("b.sqlite");
    let mut proj_a = SqliteProjection::open(&db_a).unwrap();
    let mut proj_b = SqliteProjection::open(&db_b).unwrap();

    for e in &entities_a {
        proj_a.replace_source(&e.source_id, e.resources.clone(), e.relations.clone(), vec![]).unwrap();
    }
    for e in &entities_b {
        proj_b.replace_source(&e.source_id, e.resources.clone(), e.relations.clone(), vec![]).unwrap();
    }

    // Pick the Document object_id from space A
    let doc_a = entities_a
        .iter()
        .flat_map(|e| e.resources.iter())
        .find(|r| matches!(r.kind, ResourceKind::Document))
        .cloned()
        .unwrap();
    let object_id = doc_a.object_id;

    // Cross-space aggregation
    let found_a = proj_a.find_by_object(object_id).unwrap();
    let found_b = proj_b.find_by_object(object_id).unwrap();

    assert!(!found_a.is_empty(), "space A should find the object");
    assert!(!found_b.is_empty(), "space B should find the object");

    // ObjectId matches across spaces
    assert_eq!(found_a[0].object_id, found_b[0].object_id);
    // But ResourceRef differs
    assert_ne!(found_a[0].r#ref, found_b[0].r#ref);
    // And source_id differs
    assert_ne!(found_a[0].source_id, found_b[0].source_id);
}
```

- [ ] **Step 2: 跑测试**

```bash
cd /home/lszio/Projects/notez
cargo test -p notez_core --test api_cross_space_full
```

Expected: 通过。

- [ ] **Step 3: 跑整个 workspace**

```bash
cd /home/lszio/Projects/notez
cargo test --workspace
```

Expected: 全部通过。

- [ ] **Step 4: Commit**

```bash
cd /home/lszio/Projects/notez
git add core/tests/api_cross_space_full.rs
git commit -m "test(cross-space): end-to-end aggregation by ObjectId

Two Space projections, each scanning the same file with a different
source_id, agree on object_id for the same file. find_by_object on
each projection returns the matching row; ref and source_id differ
but object_id is equal (spec §3, §10 scenario 1)."
```

---

## Task 9: 最终验证

**Files:** none (read-only)

- [ ] **Step 1: 全 workspace 测试通过**

```bash
cd /home/lszio/Projects/notez
cargo test --workspace
```

Expected: 全通过；统计：与 Task 1 启动时的 174 passing 比较，应多出至少 5 个新测试（object_id 4 个 + relation_evidence 1 个 + find_by_object 1 个 + cross_space 2 个 = 8 个新）。

- [ ] **Step 2: 全 workspace 编译干净（无 warning 增量）**

```bash
cd /home/lszio/Projects/notez
cargo build --workspace --all-targets 2>&1 | grep -E "warning|error" | head -20
```

Expected: 既有 warning 数与 baseline 持平或减少（不应有 `unused_*` 增量）。

- [ ] **Step 3: 列出 8 个 commit**

```bash
cd /home/lszio/Projects/notez
git log --oneline 272e063..HEAD
```

Expected: 8 个 commit，按 Task 1-8 顺序排列。

- [ ] **Step 4: 报告**

把以下信息回给用户：
- 8 个 commit SHA + 标题。
- `cargo test --workspace` 通过数（应有 174 + 8 = 182 个）。
- 跨 Space 同一文件 → 同一 `object_id` 的端到端验证证据（Test 8 输出片段）。
- 明确说明：跨 Space 写冲突、授权维度、Change 投递、notez:// scheme、客户端 UI、summary 均未实现，按 spec §1.3 / §9 推迟。

---

## Self-Review

**1. Spec coverage:**
- §1.3 任务列表全部覆盖：ObjectId 类型 ✓、derived_object_id ✓、schema v2 ✓、扫描 content_hash ✓、find_by_object ✓、v1 迁移 ✓、跨 Space 测试 ✓。
- §2.1 ObjectId 类型：Task 1 实现 ✓
- §2.2 derived_object_id：Task 1 实现（无 source_id，v1 简化）✓
- §2.3 Resource/ResourceRelation 字段：Task 2/3 实现 ✓
- §3.1 扫描 content_hash：Task 6 实现 ✓
- §3.2 写入：Task 2/6 + Task 5 `replace_source` 写 object_id ✓
- §3.3 关系证据：Task 3/7 实现 ✓
- §4 schema v2 + migration：Task 5 实现 ✓
- §5 find_by_object：Task 4/5 实现 ✓
- §6 测试：Task 1/5/6/7/8 覆盖单元 + 集成 + 端到端 ✓

**2. Placeholder scan:** 0 个 "TBD/TODO/待定/实现 later"。

**3. Type consistency:**
- `ObjectId` 在 resource.rs 定义、mod.rs re-export、lib.rs re-export、query.rs 使用、storage 实现 ✓
- `derived_object_id(content_hash, locator, position)` 三参数签名贯穿 spec / plan / 测试 ✓
- `Resource.object_id` 在所有 `Resource { ... }` 构造点都补字段（Task 2 Step 3 + Task 6 扫描路径）✓
- `ResourceRelation` / `ResolvedRelation` 五字段名一致：relation_type / direction / evidence_json / created_at / creator ✓
- `find_by_object` 在 trait、impl、测试三方签名一致 ✓

**4. Scope check:** 8 个 task，每个 1-2 小时工作量；总范围匹配 spec §1.3 "做" 列表；"不做" 项明确推迟到 0.6 / MVP-2 / MVP-3。

---

**Plan complete and saved to `docs/superpowers/plans/2026-08-03-mvp1-cross-space-object-identity.md`.**
