# Phase 1 数据模型：被影响文件统计与图形化操作界面

**Feature**: 002-affected-stats-visualization | **Date**: 2026-06-02

描述本特性新增/改动的数据结构。类型名为拟用名，以实现编译结果为准。所有对外项须配中文文档注释（原则 III）。
本特性为对 001 既有结构的**增量改动**，下面标注「新增」或「改动」。

---

## 实体总览

```text
AffectedFile      （新增）被影响文件：路径 + 替换次数
RunSummary        （改动）追加 affected_files，成为"影响报告"的载体
ProgressUpdate    （新增）进度快照，经回调/通道传递
GuiFormState      （新增, gui feature）GUI 参数表单状态
GuiExecState      （新增, gui feature）GUI 执行状态机
GuiMessage        （新增, gui feature）后台线程 → UI 的通道消息
```

---

## 1. AffectedFile（被影响文件）— 新增

对应 spec 实体「被影响文件」。

| 字段 | 类型 | 说明 |
|------|------|------|
| path | PathBuf | 相对目标目录的文件路径 |
| replacements | u64 | 该文件内发生的替换次数（>0） |

- **来源需求**: FR-002、FR-005、FR-010。
- **不变量**: 仅 `FileStatus::Modified` 的文件产生 `AffectedFile`；replacements 必 > 0。
- **产生点**: `replacer::process_file` 已返回含 `path`/`replacements` 的 `FileOutcome`，
  在 `run_replace` 聚合时对 Modified 项收集为 `AffectedFile`。

## 2. RunSummary（运行摘要 / 影响报告）— 改动

在 001 的 `RunSummary` 基础上追加被影响文件列表，使其成为 CLI 摘要与 GUI 展示的共同数据来源。

| 字段 | 类型 | 状态 | 说明 |
|------|------|------|------|
| affected_files | Vec<AffectedFile> | 新增 | 被影响文件列表（核心） |
| files_modified | u64 | 沿用 | 被影响文件数（= affected_files.len()，作为核心计数） |
| total_replacements | u64 | 沿用 | 总替换次数（核心） |
| files_scanned | u64 | 沿用 | 扫描总数（次要信息，FR-003） |
| files_skipped_binary | u64 | 沿用 | 跳过二进制数（次要） |
| files_failed | u64 | 沿用 | 失败数（次要） |
| backup_path | Option<PathBuf> | 沿用 | 备份归档路径 |

- **来源需求**: FR-001~FR-006、FR-010。
- **校验/一致性**: `files_modified == affected_files.len()`；`total_replacements == Σ affected_files[*].replacements`。
- **呈现**: CLI 摘要首行以"被影响 N 个文件、共替换 M 处"为主体；扫描/跳过/失败为次要行（FR-001/FR-003）。
- **规模**: 当 affected_files 很大时，GUI 以滚动区呈现并可截断显示（research §1）。

## 3. ProgressUpdate（进度快照）— 新增

替换执行过程中向前端上报的进度信息。

| 字段 | 类型 | 说明 |
|------|------|------|
| scanned | u64 | 已扫描文件数 |
| modified | u64 | 已被影响（已改动）文件数 |

- **来源需求**: FR-012、SC-006（GUI 不冻结、持续反馈）。
- **传递**: `run_replace` 接受进度回调 `Fn(ProgressUpdate) + Sync`；
  - CLI：回调更新 indicatif 文案（保持现有体验）；
  - GUI：回调将 `ProgressUpdate` 经 mpsc 发送给 UI 线程。
- **状态流转**: 执行期间多次上报；结束时由最终结果（RunSummary/Err）收尾。

## 4. GuiFormState（GUI 参数表单状态）— 新增（`gui` feature）

GUI 中各表单字段的当前值，与 `TrtArgs` 一一对应（参见 contracts/cli-trt.md）。

| 字段 | 类型 | 对应 trt 参数 |
|------|------|---------------|
| directory | String | -d/--directory |
| old_text | String | -o/--old-text |
| new_text | String | -n/--new-text |
| rules_file | String（可空） | --rules |
| backup | bool | -b（1/0） |
| undo | bool | -u（1/0） |
| case_sensitive | bool | -c（1/0） |
| use_regex | bool | -r（1/0） |
| show_progress | bool | --progress（1/0） |

- **来源需求**: FR-008、FR-018（不持久化：每次打开为默认值）。
- **映射与校验**: `form.rs` 将 `GuiFormState` 转换为 `TrtArgs`/`TrtOptions`，复用 trt 既有校验
  （目录存在、旧文本非空、规则来源齐备等），失败时在界面提示（FR-011）。
- **不变量**: 不写入磁盘、不读取历史（FR-018）。

## 5. GuiExecState（GUI 执行状态机）— 新增（`gui` feature）

```text
enum GuiExecState {
    Idle,                         // 空闲，可编辑表单与点击执行
    Confirming,                   // 破坏性操作（撤销）等待用户确认（FR-016）
    Running { progress: ProgressUpdate },  // 后台执行中，禁用执行按钮，显示进度
    Done(RunSummary),             // 完成，展示统计 + 被影响文件列表
    Failed(String),               // 失败，展示错误信息
}
```

- **来源需求**: FR-009~FR-012、FR-016。
- **流转**: Idle →（撤销需 Confirming →）Running →（Done | Failed）→ Idle（用户可再次执行）。
- **防重入**: 仅 Idle 态允许触发执行（边界：执行中重复点击无效）。

## 6. GuiMessage（后台 → UI 通道消息）— 新增（`gui` feature）

```text
enum GuiMessage {
    Progress(ProgressUpdate),  // 执行中进度
    Finished(RunSummary),      // 成功完成，携带影响报告
    Error(String),             // 执行失败
}
```

- **来源需求**: FR-012、SC-006。
- **传递**: 后台线程发送，UI 线程每帧 `try_recv` 消费并据此推进 `GuiExecState`，调用 `ctx.request_repaint()`。

---

## 关系与数据流

```text
GuiFormState ──(form.rs 映射+校验)──> TrtArgs/TrtOptions
                                          │
                          后台线程: trt 核心 execute(options, progress_cb)
                                          │  progress_cb 发送 ──> mpsc
   ┌──────────────────────────────────────┴───────────────┐
   │ ProgressUpdate（多次）                 最终 RunSummary / Err │
   └──> GuiMessage::Progress              └──> GuiMessage::Finished / Error
                    │                                    │
              UI 轮询通道 → 推进 GuiExecState → 渲染进度 / 结果（统计 + AffectedFile 列表）

CLI 路径（复用同一核心）:
TrtArgs ──> TrtOptions ──> execute(options, indicatif_cb) ──> RunSummary ──> 摘要(被影响文件为核心)
```
