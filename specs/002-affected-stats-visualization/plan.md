# Implementation Plan: 被影响文件统计与图形化操作界面

**Branch**: `002-affected-stats-visualization` | **Date**: 2026-06-02 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/002-affected-stats-visualization/spec.md`

## Summary

在不改变现有命令行行为的前提下，为 `cct` 增加两项能力：
1. **统计以被影响文件为核心**：trt 替换完成后，结果以"被影响（实际改动）的文件数 + 总替换次数"为主体呈现，
   并收集每个被影响文件及其替换次数（供 CLI 摘要与 GUI 列表共用）。
2. **桌面图形操作界面**：新增 `cct gui` 子命令，启动一个桌面原生窗口，把 trt 的参数做成表单，点击「执行」
   等效于运行命令行；执行后在界面内展示被影响文件统计与文件列表。GUI 复用 trt 核心逻辑，
   通过后台线程 + 通道实现执行期间不冻结。

技术上：把 trt 核心执行与"进度上报"解耦为回调，使 CLI 与 GUI 共用同一替换逻辑；GUI 以 `eframe`(egui) 实现，
并通过 Cargo **feature 门控**，保证 CLI-only 构建（及 musl 静态交叉编译）不受 GUI 重依赖影响。

## Technical Context

**Language/Version**: Rust 1.95（stable，edition 2024，沿用现有工具链）

**Primary Dependencies**:
- 现有依赖不变（clap / ignore / rayon / aho-corasick / regex / zip / memchr / tempfile / indicatif / anyhow / thiserror / serde / serde_json / chrono）
- **新增（均在 `gui` feature 下，可选）**：
  - `eframe` + `egui`——纯 Rust、跨平台的桌面即时模式 GUI，可静态编入单一可执行文件
  - `rfd`——原生文件/目录选择对话框（FR-017）

**Storage**: 无新增持久化（FR-018，不记忆参数）；仍为文件系统（目标目录 + 同级 backup）

**Testing**: `cargo test`。GUI 的**逻辑层**（表单字段 ↔ `TrtArgs`/`TrtOptions` 映射与校验、被影响文件收集）
编写单元/集成测试；窗口渲染本身以手动验证为主。

**Target Platform**: Windows 与 Linux 桌面（GUI 需图形环境）；CLI 仍可在无图形环境构建与运行。

**Project Type**: 单一 CLI 可执行程序，新增 `gui` 子命令（feature-gated）。

**Performance Goals**:
- GUI 执行期间界面持续响应、不冻结（后台线程执行 + 通道回传进度，UI 线程仅渲染）。
- 统计改进与被影响文件收集 MUST NOT 使替换吞吐显著回退。

**Constraints**:
- 单一可执行文件（GUI 作为子命令）。
- GUI 经 `gui` feature 门控；`--no-default-features` 得到 CLI-only 构建，保留 musl 静态交叉编译能力。
- 向后兼容：默认命令行行为（`cct`/`cct -ls`/`cct trt ...`）逐字节不变。
- 不持久化用户输入。

**Scale/Scope**: 改造 `trt`（统计与进度回调）+ 新增 `gui` 模块；预计新增/改动 ~600–900 LOC。

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

依据 `.specify/memory/constitution.md` v1.0.0 逐项核验：

| 原则 | 符合性 |
|------|--------|
| I. 子命令优先的 CLI 架构 | ✅ GUI 作为新增 `gui` 子命令，仍是单一可执行文件；不改变现有子命令与无参行为 |
| II. 主流最佳实践与官方推荐写法 | ✅ `eframe`/`egui` 是 Rust 社区事实标准的即时模式 GUI，`rfd` 为主流文件对话框；继续 cargo/fmt/clippy；无 unsafe |
| III. 清晰注释与中文文档 | ✅ 新增模块与公共项均配中文 `///` 注释；本规划文档全中文 |
| IV. 拒绝过度抽象与面条代码 | ⚠️ 见复杂度跟踪：GUI 引入重依赖 `eframe`，通过 feature 门控 + 单层进度回调将复杂度局部化，不做多余抽象 |
| V. 编译驱动的迭代优化 | ✅ 以 cargo build/clippy/test 反馈驱动；GUI 逻辑层可测 |

**技术栈约束**：Rust stable ✅；cargo + Cargo.lock ✅；标准布局，GUI 独立模块 ✅；fmt/clippy 纳入 ✅；
跨平台注意 GUI 在 Linux 需 X11/Wayland 运行库（见 research）。

**结论**：门禁 **PASS**（GUI 重依赖已在复杂度跟踪中说明并以 feature 门控缓解，非过度抽象违规）。

## Project Structure

### Documentation (this feature)

```text
specs/002-affected-stats-visualization/
├── plan.md              # 本文件
├── research.md          # Phase 0 技术决策
├── data-model.md        # Phase 1 实体模型
├── quickstart.md        # Phase 1 使用示例
├── contracts/           # Phase 1 契约
│   ├── cli-gui.md       # cct gui 子命令契约
│   └── trt-stats-output.md  # trt 统计输出（被影响文件口径）契约变更
└── tasks.md             # Phase 2（/speckit-tasks 生成）
```

### Source Code (repository root) — 增量改造

```text
src/
├── cli.rs               # 改：新增 Gui 子命令变体
├── registry.rs          # 改：注册表新增 gui 条目
├── main.rs              # 改：分发 Gui（feature 关闭时给出明确提示）
├── commands/
│   ├── mod.rs           # 改：声明 gui 模块（feature-gated）
│   ├── trt/
│   │   ├── mod.rs       # 改：RunSummary 增加 affected_files；run_replace 收集被影响文件 + 接受进度回调；
│   │   │                #     摘要以被影响文件为核心；抽出供 GUI 调用的执行入口
│   │   └── replacer.rs  # 改（如需）：FileOutcome 已含 path/replacements，供收集被影响文件
│   └── gui/             # 新增（#[cfg(feature = "gui")]）
│       ├── mod.rs       # gui 子命令入口：启动 eframe；无图形环境/初始化失败给出明确提示
│       ├── app.rs       # eframe App：表单状态、执行状态机、结果（统计+文件列表）展示
│       ├── form.rs      # 参数表单字段 ↔ TrtArgs/TrtOptions 映射与前置校验
│       └── runner.rs    # 后台线程执行 trt 核心，经 mpsc 通道回传进度与最终结果
└── util/                # 不变

tests/
├── trt_stats.rs         # 新增：被影响文件统计口径（CLI 摘要）集成测试
└── gui_form.rs          # 新增：表单 ↔ 参数映射与校验的单元/集成测试（不渲染窗口）
```

**Structure Decision**：沿用单一项目布局。`trt` 做最小改造以输出被影响文件并支持进度回调；
GUI 自成 `commands/gui/` 模块并整体置于 `gui` feature 之下，使 CLI-only 构建零 GUI 依赖。
进度上报由 `trt` 内部直接操作 indicatif 改为接受一个轻量回调，CLI 与 GUI 各自提供实现——
这是支撑两个前端所必需的唯一抽象，不引入额外层次。

## Complexity Tracking

> 记录确需引入、可能与"简单优先/拒绝过度抽象"产生张力的复杂度及其正当理由。

| 复杂度 | 为何需要 | 已拒绝的更简方案及原因 |
|--------|----------|------------------------|
| 引入 `eframe`/`egui` 重依赖 | 用户明确要求桌面图形界面（点击执行替代命令行）；eframe 是纯 Rust、跨平台、可编入单一可执行文件的最简路径 | 自绘 GUI/绑定系统原生控件——工作量与维护成本远高；TUI/网页——已被用户排除 |
| GUI 经 Cargo feature 门控 | 让 CLI-only 构建零 GUI 依赖，保留 musl 静态交叉编译；GUI 重依赖不拖累纯命令行场景 | 不门控、GUI 始终编入——会破坏现有 Windows→Linux 静态交叉编译且增大所有构建体积 |
| trt 执行引入进度回调抽象 | CLI（indicatif）与 GUI（通道）两个前端需共用同一替换核心并各自呈现进度 | 复制一份 trt 逻辑给 GUI——违背 DRY 且易使两端行为漂移 |
