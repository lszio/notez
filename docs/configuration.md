# Notez 配置

## 1. 配置分层

全局配置位于 `$XDG_CONFIG_HOME/notez/config.toml`，未设置 `XDG_CONFIG_HOME` 时使用
`~/.config/notez/config.toml`。它负责空间注册表、默认空间和跨空间用户偏好。

空间配置默认为 `<space>/notez.toml`。它负责 sources、workflow、索引、同步和可选格式覆盖。
数据库、缓存、任务状态与诊断数据位于 `<space>/.notez/`，不属于声明式配置。

## 2. 全局配置示例

```toml
version = 1
default_space = "personal"

[spaces.personal]
path = "~/Notes/personal"

[spaces.work]
path = "~/Notes/work"
config = "notez.toml"

[preferences]
output = "human"
log_level = "warn"
```

`~` 只在路径字段内由 Notez 展开。注册名必须唯一；路径解析后不得指向同一个空间却产生冲突
名称。全局配置不保存某空间的 sources 或 workflow。

## 3. 空间配置示例

```toml
version = 1

[space]
name = "personal"
database = ".notez/index.sqlite"

[workflow]
todo = ["TODO", "NEXT", "WAIT"]
done = ["DONE", "QUIT"]

[[sources]]
name = "vault"
kind = "obsidian"
path = "../vault"
read_only = true
```

相对路径基于 `notez.toml` 所在目录解析，而非进程当前目录。常见链接解析使用内置 profile，
空间配置不需要声明；自定义规则只覆盖指定字段。

## 4. 空间选择

1. `--space` 指定的注册名或路径。
2. 从当前目录向上查找 `notez.toml`。
3. 全局 `default_space`。
4. 若仍无法确定，返回错误且不创建空间。

显式 `--config` 可以选择非默认空间配置文件，其父目录成为相对路径基准。

## 5. 覆盖顺序

从低到高依次是：内置默认值和链接 profiles、全局用户偏好、空间配置、环境变量、本次 CLI
参数。CLI 和环境变量只影响当前进程，不回写文件。`config show` 应显示有效值及每个值的来源。

## 6. 校验与迁移

配置顶层必须包含受支持的 `version`，未知字段默认报错。错误报告包含文件路径、字段路径和
原因；不得静默回退。

旧 `.notez/sources.json` 通过显式迁移命令写入 `notez.toml`。旧 communities 数据迁移为
领域声明，不混入配置。迁移先生成预览和冲突报告，成功前保留原文件，不在普通启动中自动写入。
