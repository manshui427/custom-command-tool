---
description: "Task list for 被影响文件统计与图形化操作界面"
---

# Tasks: 被影响文件统计与图形化操作界面

**Input**: Design documents from `specs/002-affected-stats-visualization/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: 规格未强制 TDD。延续宪法原则 V 与 001 既有做法，为每个用户故事保留集成测试（建议执行）。
GUI 窗口渲染本身以手动验证为主，仅对其**逻辑层**（表单↔参数映射、校验）编写自动化测试。

**Organization**: 任务按用户故事分组。被影响文件收集、RunSummary 扩展、进度回调解耦为 US1/US2 共同前置，置于 Foundational。

**本特性为对 001 既有代码的增量改造**：trt 已实现，此处改造统计与进度上报，并新增 gui 模块。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行（不同文件、无未完成依赖）
- **[Story]**: 所属用户故事（US1/US2）
- 每条任务含明确文件路径

## Path Conventions

单一 Rust 项目，源码在 `src/`，集成测试在 `tests/`（见 plan.md）。

---

## Phase 1: Setup（共享基础设施）

**Purpose**: 引入 GUI 依赖与 feature 门控，保证 CLI-only 构建不受影响

- [X] T001 在 `Cargo.toml` 声明 `[features] default = ["gui"]`，并把 `eframe`、`rfd` 添加为 `optional = true` 的依赖（`cargo add eframe --optional`、`cargo add rfd --optional`，再将其归入 `gui` feature）
- [X] T002 验证两种构建均通过：`cargo build`（含 gui）与 `cargo build --no-default-features`（CLI-only），确认依赖解析与 feature 门控正确
- [X] T003 [P] 在 `Cargo.toml` 的 `gui` feature 注释中说明用途，并更新 `Cargo.lock` 入库

**Checkpoint**: 含/不含 GUI 两种构建均可编译

---

## Phase 2: Foundational（阻塞性前置，US1 与 US2 共同依赖）

**Purpose**: 改造 trt 核心以产出"影响报告"并支持进度回调，使 CLI 与 GUI 共用同一替换逻辑

**⚠️ CRITICAL**: 本阶段完成前，US1 的新统计与 US2 的 GUI 均无法正确实现

- [X] T004 在 `src/commands/trt/mod.rs` 定义 `AffectedFile { path, replacements }`（data-model 实体 1），并为 `RunSummary` 追加 `affected_files: Vec<AffectedFile>` 字段（data-model 实体 2）
- [X] T005 在 `src/commands/trt/mod.rs` 的 `run_replace` 中，用 `Mutex<Vec<AffectedFile>>` 收集 `FileStatus::Modified` 文件（path + replacements），结束时填入 `RunSummary.affected_files`；保证 `files_modified == affected_files.len()`、`total_replacements == Σ replacements`
- [X] T006 在 `src/commands/trt/mod.rs` 定义 `ProgressUpdate { scanned, modified }`（data-model 实体 3），并将 `run_replace` 改造为接受进度回调 `progress: impl Fn(ProgressUpdate) + Sync`，替换原先内部直接操作 indicatif 的逻辑
- [X] T007 在 `src/commands/trt/mod.rs` 提供 CLI 侧进度回调实现（更新 indicatif spinner 文案），保持现有命令行进度体验不变
- [X] T008 在 `src/commands/trt/mod.rs` 抽出供 GUI 调用的执行入口（如 `pub fn execute(options, progress_cb) -> CctResult<RunSummary>`），CLI 的 `run()` 改为调用它，确保替换/备份/撤销结果逐字节不变

**Checkpoint**: trt 产出含被影响文件的影响报告，且执行与进度上报已解耦，CLI 行为不变

---

## Phase 3: User Story 1 - 统计以"被影响的文件"为核心 (Priority: P1) 🎯 MVP

**Goal**: trt 命令行摘要以被影响文件数与总替换次数为主体，扫描/跳过/失败作为次要信息

**Independent Test**: 在含大量文件、仅少数命中的目录执行替换，验证摘要以"被影响 N 个文件、共替换 M 处"为主体，数字与实际一致

### Implementation for User Story 1

- [X] T009 [US1] 在 `src/commands/trt/mod.rs` 的 `print_summary` 中，将摘要首要信息改为"被影响 N 个文件，共替换 M 处"，扫描/跳过/失败降为次要行（FR-001/FR-003，contracts/trt-stats-output.md）
- [X] T010 [US1] 处理无命中场景：明确输出"被影响 0 个文件"且退出码为 0（FR-004）
- [X] T011 [P] [US1] 更新 001 既有集成测试中对旧摘要文案的断言（`tests/trt_replace.rs` 等），改为新口径；替换/备份/撤销的行为断言保持不变（research §10）
- [X] T012 [P] [US1] 在 `tests/trt_stats.rs` 新增集成测试：被影响文件数与总替换次数准确、无命中显示 0、次要信息仍可获得（US1 验收场景，FR-002/FR-005）

**Checkpoint**: US1 可独立交付——CLI 统计以被影响文件为核心，构成本特性的 MVP

---

## Phase 4: User Story 2 - 图形化操作界面替代命令行 (Priority: P2)

**Goal**: 新增 `cct gui`，打开桌面窗口填表执行，等效命令行，执行后展示统计与被影响文件列表

**Independent Test**: 运行 `cct gui`，填写参数点击执行，验证效果等同命令行且界面展示被影响文件统计与列表；无图形环境时给出提示而非崩溃

### Implementation for User Story 2

- [X] T013 [US2] 在 `src/cli.rs` 新增 `Gui` 子命令变体；在 `src/registry.rs` 注册 gui 条目（名称/描述）；在 `src/commands/mod.rs` 用 `#[cfg(feature = "gui")]` 声明 `gui` 模块（FR-007/FR-019，contracts/cli-gui.md）
- [X] T014 [US2] 在 `src/main.rs` 分发 `Gui`：feature 启用时调用 `commands::gui::run()`；未启用 feature 时输出"本可执行文件未包含图形界面支持"并非零退出（FR-007）
- [X] T015 [P] [US2] 在 `src/commands/gui/form.rs` 实现 `GuiFormState`（data-model 实体 4）及其 ↔ `TrtArgs`/`TrtOptions` 的映射与前置校验（复用 trt 校验：目录存在、旧文本非空、规则来源齐备）（FR-008/FR-011）
- [X] T016 [P] [US2] 在 `src/commands/gui/runner.rs` 实现后台线程执行：`GuiMessage`（Progress/Finished/Error，data-model 实体 6），后台调用 `trt::execute` 并经 mpsc 通道回传进度与结果（FR-012，research §5）
- [X] T017 [US2] 在 `src/commands/gui/app.rs` 实现 eframe App：`GuiExecState` 状态机（data-model 实体 5）、参数表单渲染、执行按钮（运行期禁用防重入）、撤销确认对话框（FR-016）、目标目录/规则文件的 rfd 选择按钮（FR-017）
- [X] T018 [US2] 在 `src/commands/gui/app.rs` 实现执行流程：点击执行→校验→（撤销则先确认）→启动 runner→每帧 `try_recv` 推进状态并 `ctx.request_repaint()`→完成后展示汇总统计 + 被影响文件列表（可滚动、过大截断提示）（FR-009/FR-010/FR-012）
- [X] T019 [US2] 在 `src/commands/gui/mod.rs` 实现 `run()`：启动 eframe 原生窗口；初始化失败/无图形环境时捕获错误，返回 `CctError` 由 main 输出明确中文提示并非零退出（FR-014/FR-015，research §7）
- [X] T020 [P] [US2] 在 `tests/gui_form.rs` 编写逻辑层测试（不渲染窗口）：`GuiFormState`→`TrtOptions` 映射正确、各开关与命令行选项语义一致、非法输入被校验拦截（US2 验收场景 2/3）

**Checkpoint**: US1 + US2 均可用；图形界面等效命令行并展示被影响文件结果

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: 跨故事收尾与质量加固

- [X] T021 [P] 为新增/改动的公共项（AffectedFile、ProgressUpdate、execute、gui 各模块）补全中文文档注释（宪法原则 III）
- [X] T022 运行 `cargo fmt` 与 `cargo clippy --all-targets`（含 gui）及 `cargo clippy --all-targets --no-default-features`（CLI-only），修复全部告警直至干净（宪法原则 II/V）
- [X] T023 [P] 手动验证 GUI：Windows 上 `cct gui` 填表执行、撤销确认、进度不冻结、结果列表；无图形环境下的提示行为（FR-012~FR-016，SC-006/SC-007）
- [X] T024 [P] 验证向后兼容：`cct`、`cct -ls`、`cct trt ...` 行为与升级前一致；CLI-only 构建可静态交叉编译 Linux（`--no-default-features --target x86_64-unknown-linux-musl`）（FR-013/SC-004）
- [X] T025 按 `specs/002-affected-stats-visualization/quickstart.md` 逐条走查命令与界面流程，确认与实际一致；更新根 `README.md` 增补 gui 子命令与 CLI-only/GUI 构建说明

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖，最先（引入 gui 依赖与 feature 门控）
- **Foundational (Phase 2)**: 依赖 Setup —— 阻塞 US1 与 US2（影响报告 + 进度回调 + 统一执行入口）
- **US1 (Phase 3)**: 依赖 Foundational；仅改 CLI 摘要呈现，独立可交付（MVP）
- **US2 (Phase 4)**: 依赖 Foundational（需 `trt::execute` 与影响报告）；与 US1 相互独立，可并行
- **Polish (Phase 5)**: 依赖目标用户故事完成

### User Story Dependencies

- **US1 (P1)**: 仅依赖 Foundational，无对 US2 的依赖
- **US2 (P2)**: 仅依赖 Foundational；不依赖 US1（GUI 直接用影响报告，不依赖 CLI 摘要文案）

### Within Each User Story

- US1：摘要呈现改造 → 无命中处理 → 更新旧测试/新增统计测试
- US2：分发与注册 → 表单映射/校验 + 后台 runner（可并行）→ App 渲染与执行流程 → 入口与无图形环境处理 → 逻辑层测试

### Parallel Opportunities

- Setup：T003 可与 T001/T002 收尾并行
- Foundational：T004→T005 顺序（同文件）；T006/T007/T008 围绕 mod.rs，主要顺序推进
- US1：T011、T012 测试可并行
- US2：T015（form）、T016（runner）、T020（测试）可并行起步；T017/T018 同 app.rs 需顺序
- **US1 与 US2 可由不同人并行**（Foundational 完成后）
- Polish：T021、T023、T024 可并行

---

## Parallel Example: User Story 2

```bash
# Foundational 完成后，US2 起步阶段可并行（不同文件）：
Task: "T015 [US2] GuiFormState 映射与校验 in src/commands/gui/form.rs"
Task: "T016 [US2] 后台执行 + 通道消息 in src/commands/gui/runner.rs"
Task: "T020 [US2] 表单逻辑层测试 in tests/gui_form.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. 完成 Phase 1 Setup（feature 门控）
2. 完成 Phase 2 Foundational（影响报告 + 进度回调 + execute 入口）
3. 完成 Phase 3 US1（CLI 统计以被影响文件为核心）
4. **停下验证**：CLI 摘要与向后兼容
5. 可作为 MVP 交付（统计改进，零 GUI 风险）

### Incremental Delivery

1. Setup + Foundational → 核心改造就绪
2. 加 US1 → 验证 → 交付（统计改进）
3. 加 US2 → 验证 → 交付（图形界面）
4. Polish → 文档、clippy（双 feature）、跨平台、quickstart、README

---

## Notes

- [P] = 不同文件、无未完成依赖，可并行
- GUI 渲染靠手动验证；逻辑层（表单映射/校验）走自动化测试
- 每完成一个任务或一组逻辑后建议提交一次
- 关键约束：默认行为逐字节向后兼容；CLI-only 构建零 GUI 依赖、可静态交叉编译
- 避免：含糊任务、同文件并行冲突、破坏故事独立性的跨故事依赖
