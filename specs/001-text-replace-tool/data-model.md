# Phase 1 数据模型：trt + 主命令框架

**Feature**: 001-text-replace-tool | **Date**: 2026-06-01

本文件描述实现所需的核心数据结构（实体）、字段、校验规则与状态流转。命名为拟用的 Rust 类型名，
最终以实现阶段编译结果为准。所有面向外部的类型与字段在代码中均须配中文文档注释（宪法原则 III）。

---

## 实体总览

```text
SubcommandInfo            子命令元信息（注册表项，供 -ls 列举）
ReplacementRule           单条替换规则（旧文本 → 新文本）
RuleSet                   规则集（一条或多条 ReplacementRule，含全局开关）
TrtOptions                trt 一次运行的完整输入（替换任务）
TrtMode                   运行模式枚举（替换 / 撤销）
FileOutcome               单个文件的处理结果
RunSummary                整体运行结果摘要
BackupArchive             备份归档（ZIP）的逻辑视图
BackupManifest            备份归档内的清单（来源目录元数据）
```

---

## 1. SubcommandInfo（子命令元信息）

供主命令框架 `-ls` 列举使用。

| 字段 | 类型 | 说明 |
|------|------|------|
| name | &'static str | 子命令全名（如 `text-replace-tool`） |
| alias | &'static str | 简写别名（如 `trt`） |
| description | &'static str | 中文简要描述（如 `文本替换工具`） |

- **来源需求**: FR-001、FR-002、FR-005。
- **校验规则**: name 与 alias 在注册表内唯一。
- **生命周期**: 编译期静态常量，无运行时状态。

---

## 2. ReplacementRule（替换规则）

替换的最小单元。

| 字段 | 类型 | 说明 |
|------|------|------|
| old_text | String | 被替换文本（字面或正则模式串），非空 |
| new_text | String | 替换文本；正则模式下可含 `$1`/`${name}` 捕获组引用 |
| order | usize | 在规则集中的序号，决定重叠命中时的优先级（小者优先） |

- **来源需求**: FR-007、FR-007a、FR-008a、FR-016a。
- **校验规则**:
  - old_text MUST 非空（空旧文本属非法输入 → FR-022 / spec 边界）。
  - 正则模式下 old_text MUST 为合法正则，否则非法输入报错。
  - new_text 可为空（表示删除匹配内容）。

---

## 3. RuleSet（规则集）

一次运行中全部规则 + 对所有规则统一生效的全局开关。

| 字段 | 类型 | 说明 |
|------|------|------|
| rules | Vec<ReplacementRule> | 有序规则列表，至少 1 条 |
| case_sensitive | bool | 大小写敏感（对应 `-c`），统一生效 |
| use_regex | bool | 正则模式（对应 `-r`），统一生效 |

- **来源需求**: FR-007a、FR-007b、FR-008a、FR-015、FR-016、FR-016b。
- **校验规则**:
  - rules 非空（既无 `-o/-n` 也无有效 `--rules` → 非法输入）。
  - `-o/-n` 与 `--rules` 同时提供时，命令行单组规则追加为规则集中的一条（FR-007b）。
  - 全局开关对所有规则一致，不存在逐条开关（FR-016b）。
- **构建产物**: 由 RuleSet 编译出"匹配器"——字面模式 → aho-corasick 自动机；正则模式 → 合并/集合正则。
  匹配语义：最左、不重叠、按 order 优先、无级联（FR-008a）。

### 规则文件格式（`--rules <文件>` 解析规则）

- 按行解析；每行一组：`<old><分隔符><new>`（分隔符约定为制表符 `\t`）。
- 以 `#` 起始的行与空行 MUST 忽略（spec 假设）。
- 行格式非法（缺少分隔符）→ 报错并指明行号，不执行部分替换（spec 边界）。
- 文件不存在/不可读/无有效规则 → 非法输入，非零退出（spec 边界）。

---

## 4. TrtOptions（替换任务，trt 运行的完整输入）

对应 spec "替换任务 (Replacement Task)" 实体，是 CLI 参数解析后的归一化结果。

| 字段 | 类型 | 说明 | CLI 来源 | 默认 |
|------|------|------|----------|------|
| directory | PathBuf | 目标目录（相对或绝对，运行时规范化） | `-d`/`--directory` | 必填 |
| mode | TrtMode | 运行模式（替换 / 撤销） | 由 `-u` 推导 | Replace |
| rule_set | Option<RuleSet> | 规则集（撤销模式可为 None） | `-o`/`-n`/`--rules` | — |
| backup_enabled | bool | 是否启用备份 | `-b`/`--backup`（1/0） | true |
| show_progress | bool | 是否显示进度 | `--progress`（1/0） | true |

- **来源需求**: FR-006、FR-007、FR-010、FR-014、FR-017。
- **校验规则**:
  - directory MUST 存在且可访问，否则非零退出（FR-022 / spec 边界）。
  - Replace 模式下 rule_set MUST 为 Some 且非空；Undo 模式下 rule_set 可为 None（FR-007、FR-014）。
  - 开关类参数取值限定 1/0，其他值视为非法输入（spec 假设）。

---

## 5. TrtMode（运行模式枚举）

```text
enum TrtMode {
    Replace,   // 正常替换（-u 0，默认）
    Undo,      // 撤销上一次替换（-u 1）
}
```

- **来源需求**: FR-014。
- **状态流转**: 二选一，由 `-u` 决定；两模式互斥，不可同时。

---

## 6. FileOutcome（单文件处理结果）

并行处理中每个文件产出一项，用于汇总与进度。

| 字段 | 类型 | 说明 |
|------|------|------|
| path | PathBuf | 文件路径（相对目标目录） |
| status | FileStatus | 处理状态 |
| replacements | u64 | 该文件内发生的替换次数（仅 Modified 时有意义） |

```text
enum FileStatus {
    Modified,        // 文本文件且发生了替换
    Unchanged,       // 文本文件但无匹配
    SkippedBinary,   // 检测为二进制，跳过
    Failed(String),  // 处理失败（含原因），不影响其他文件继续
}
```

- **来源需求**: FR-008、FR-009、FR-021、FR-022。
- **校验/不变量**: 仅 Modified 的文件进入备份归档（FR-011）。

---

## 7. RunSummary（运行摘要）

操作结束时向用户报告（FR-021）。

| 字段 | 类型 | 说明 |
|------|------|------|
| files_scanned | u64 | 已扫描文件总数 |
| files_modified | u64 | 被修改文件数 |
| files_skipped_binary | u64 | 跳过的二进制文件数 |
| files_failed | u64 | 处理失败文件数 |
| total_replacements | u64 | 总替换次数 |
| backup_path | Option<PathBuf> | 生成的备份 ZIP 路径（未备份则 None） |

- **来源需求**: FR-021、SC-004。
- **用途**: 决定进程退出码（有 Failed 且关键 → 非零；纯统计成功 → 0）。

---

## 8. BackupArchive（备份归档，逻辑视图）

物理形态为单个 ZIP 文件，本结构描述其逻辑属性。

| 属性 | 说明 |
|------|------|
| 物理路径 | `<目标目录父目录>/backup/backup_yyyyMMddHHmmss.zip`（FR-012） |
| 内部布局 | 被改文件按"相对目标目录的原始层级"存放（FR-013、SC-006） |
| 清单条目 | 内含 `.cct-manifest.json`（见 BackupManifest），记录来源目录元数据（FR-013a） |
| 内容范围 | 仅包含本次实际被修改文件的**替换前**原始内容（FR-011） |

- **来源需求**: FR-011、FR-012、FR-013、FR-013a、SC-005、SC-006。
- **不变量**: 备份在对应文件被覆盖**之前**写入（先备份后改），保证可还原。

---

## 9. BackupManifest（备份清单）

写入备份 ZIP 内部的元数据条目，是撤销时归属判定的依据。

| 字段 | 类型 | 说明 |
|------|------|------|
| source_directory | String | 来源目标目录的规范化绝对路径（撤销按此匹配 `-d`） |
| created_at | String | 创建时间戳（yyyyMMddHHmmss，与文件名一致） |
| tool_version | String | 生成工具版本（便于未来兼容判断） |
| file_count | u64 | 归档内被备份文件数 |

- **来源需求**: FR-013a、FR-014、spec 边界（多目录共享 backup 的归属）。
- **校验规则**: 撤销时以 `source_directory`（规范化后）== 当前 `-d`（规范化后）筛选归档，
  在匹配集合中取 `created_at` 最近者还原（spec 边界：按目录定位 + 按时间逐次回退）。
- **序列化**: serde_json。

---

## 实体关系

```text
SubcommandInfo[]  ──(注册表，供 -ls)──>  主命令框架

TrtOptions
 ├── directory: PathBuf
 ├── mode: TrtMode
 ├── rule_set: Option<RuleSet>
 │                 └── rules: Vec<ReplacementRule>   (1..N，order 决定优先级)
 ├── backup_enabled: bool
 └── show_progress: bool

执行 Replace:
 TrtOptions ──> Scanner ──> 多个 FileOutcome ──> RunSummary
                            └─(Modified 文件，覆盖前)─> BackupArchive(含 BackupManifest)

执行 Undo:
 TrtOptions(dir) ──> 扫描 backup/*.zip 的 BackupManifest
                 ──> 按 source_directory==dir 取最近 ──> 还原覆盖 ──> 删除该 ZIP
```
