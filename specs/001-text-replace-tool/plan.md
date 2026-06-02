# Implementation Plan: 主命令框架与文本替换子命令 (trt)

**Branch**: `001-text-replace-tool` | **Date**: 2026-06-01 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-text-replace-tool/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command. See `.specify/templates/plan-template.md` for the execution workflow.

## Summary

构建单一 Rust 可执行文件 `cct`，以子命令方式组织功能。本期交付主命令框架（`-ls` 列出子命令、
`-h` 帮助、无子命令时等同 `-ls`）与第一个子命令 `trt`（文本替换工具）。trt 面向超大型目录（300GB+、
海量文件），采用"边扫描边并行处理"的流式架构：通过 `ignore`/`walkdir` 递归遍历并用 `rayon` 并行处理，
自动跳过二进制文件，对文本文件按"单遍同时匹配"语义应用一组或多组替换规则（支持字面/正则、大小写控制、
正则捕获组）。写回采用"临时文件 + 原子改名"保证崩溃安全；被修改文件在替换前流式写入带时间戳的 ZIP 备份
（内含来源目录元数据），并支持按目标目录定位的一键撤销。

## Technical Context

**Language/Version**: Rust 1.75+（stable 工具链）

**Primary Dependencies**:
- `clap` v4（derive 风格）——子命令与参数解析
- `rayon`——数据并行（`par_bridge` 实现扫描与处理同步进行）
- `ignore` 或 `walkdir`——递归目录遍历（`ignore` 支持并行 walker，优先）
- `regex`——正则模式与捕获组替换
- `aho-corasick`——多模式字面替换的单遍同时匹配（批量规则核心）
- `zip`——备份归档的流式压缩
- `memchr`——二进制检测（NUL 字节扫描）与快速字面查找
- `indicatif`——进度条显示
- `anyhow` + `thiserror`——错误处理（边界用 anyhow，库内用 thiserror）
- `tempfile`——同目录临时文件的安全创建

**Storage**: 本地文件系统（目标目录原地修改 + 同级 `backup/` 目录下的 ZIP 备份归档）

**Testing**: `cargo test`（单元测试 + 基于 `assert_cmd` + `tempfile` 的 CLI 集成测试）

**Target Platform**: Windows 与 Linux（跨平台一致）

**Project Type**: 单一 CLI 可执行程序（single project）

**Performance Goals**:
- 处理 300GB+、海量文件目录不崩溃、不耗尽内存（SC-001）
- 启动后数秒内开始反馈进度，不等待整个目录扫描完成（SC-002，FR-018）
- 内存占用基本恒定，不随文件数量增长而显著上升（SC-003）

**Constraints**:
- 内存：不将整个大文件一次性载入；大文件采用带重叠窗口的流式替换（FR-019）
- 崩溃安全：写回必须"临时文件 + 原子 rename"，中途崩溃不留半写文件（FR-019a）
- 数据安全：二进制文件 100% 跳过，不破坏非文本数据（FR-009、SC-004）
- 可还原性：启用备份时撤销可 100% 还原（SC-005、SC-006）

**Scale/Scope**: 1 个主命令框架 + 1 个子命令（trt）；本期不含其他子命令。预计源码规模 ~1500-2500 LOC。

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

依据 `.specify/memory/constitution.md` v1.0.0 的五条核心原则逐项核验：

| 原则 | 要求 | 本计划符合性 |
|------|------|-------------|
| I. 子命令优先的 CLI 架构 | 单一可执行文件 + 子命令；每个子命令职责单一、可独立测试、有 `--help` | ✅ `cct` 单可执行；`trt` 为独立子命令，clap 自动提供 `--help`；遵循 stdin/stdout/stderr 与退出码约定 |
| II. 主流最佳实践与官方推荐写法 | 用 cargo/rustfmt/clippy；优先主流 crate；禁止无理由 unsafe | ✅ 全程 cargo；选用 clap/rayon/regex/zip 等社区主流 crate；无 unsafe；提交前 fmt + clippy |
| III. 清晰注释与中文文档 | 对外函数/子命令/模块须有中文文档注释；注释解释"为什么" | ✅ 所有模块与公共项用 `///` 中文文档注释；规划文档全部中文 |
| IV. 拒绝过度抽象与面条代码 | YAGNI；简单直接；模块按子命令边界划分 | ✅ 按子命令分模块（`commands/trt`）；不引入假想未来的抽象；本期仅一个子命令，注册表保持最小 |
| V. 编译驱动的迭代优化 | 允许并鼓励 cargo build/test 迭代；交付前必须通过 | ✅ 实现阶段以 `cargo build`/`cargo clippy`/`cargo test` 反馈驱动迭代 |

**技术栈约束核验**: Rust stable ✅；cargo + Cargo.lock 提交 ✅；标准 Cargo 布局（`src/main.rs` 入口，子命令分模块）✅；
rustfmt/clippy 纳入流程 ✅；注意 Windows/Linux 路径与换行差异 ✅。

**结论**: 初次门禁 **PASS**，无违规项，无需填写复杂度跟踪表。

## Project Structure

### Documentation (this feature)

```text
specs/001-text-replace-tool/
├── plan.md              # 本文件 (/speckit-plan 输出)
├── research.md          # Phase 0 输出（技术决策）
├── data-model.md        # Phase 1 输出（实体模型）
├── quickstart.md        # Phase 1 输出（使用示例）
├── contracts/           # Phase 1 输出（CLI 命令契约）
│   ├── cli-main.md      # 主命令契约（-ls / -h / 无子命令）
│   └── cli-trt.md       # trt 子命令契约（参数、退出码、输出）
└── tasks.md             # Phase 2 输出 (/speckit-tasks，本命令不创建)
```

### Source Code (repository root)

```text
Cargo.toml               # 包清单与依赖声明
Cargo.lock               # 锁定依赖版本（提交入库）
src/
├── main.rs              # 程序入口：解析顶层参数，分发到子命令或框架行为
├── cli.rs               # 顶层 CLI 定义（clap）：全局选项 -ls/-h，子命令枚举
├── registry.rs          # 子命令注册表：名称、别名、简要描述（供 -ls 列举）
├── commands/
│   ├── mod.rs           # 子命令模块聚合
│   └── trt/
│       ├── mod.rs       # trt 子命令入口：参数解析、模式编排（替换 vs 撤销）
│       ├── args.rs      # trt 参数定义与校验（-d/-o/-n/-b/-u/-c/-r/--progress/--rules）
│       ├── rules.rs     # 替换规则集：单组参数与 --rules 文件解析、构建匹配器
│       ├── matcher.rs   # 单遍同时匹配引擎（aho-corasick 字面 / regex 正则）
│       ├── scanner.rs   # 并行目录遍历 + 二进制检测 + 文件分发（ignore + rayon）
│       ├── replacer.rs  # 单文件替换：小文件整体 / 大文件流式重叠窗口 + 原子写回
│       ├── backup.rs    # 备份归档：被改文件流式写入 ZIP + 来源目录元数据清单
│       └── undo.rs      # 撤销：按目标目录定位最近备份、还原、删除归档
└── util/
    ├── mod.rs
    ├── fs_atomic.rs     # 临时文件 + 原子改名的跨平台封装
    └── progress.rs      # 进度条封装（indicatif，可开关）

tests/
├── trt_replace.rs       # trt 替换功能集成测试（单组/多组规则、正则、大小写）
├── trt_backup_undo.rs   # 备份与撤销集成测试（多目录共享 backup 的归属）
├── trt_binary_skip.rs   # 二进制跳过与编码边界集成测试
└── main_framework.rs    # 主命令 -ls/-h/无子命令行为集成测试
```

**Structure Decision**: 采用单一项目（single project）布局，符合宪法"标准 Cargo 布局 + 子命令分模块"要求。
顶层 `main.rs`/`cli.rs`/`registry.rs` 构成可扩展的子命令框架；每个子命令独占 `commands/<name>/` 子目录，
内部再按职责（参数、规则、匹配、扫描、替换、备份、撤销）拆分为单一职责模块，避免面条代码。
跨切面的原子写入与进度条放入 `util/` 复用。测试按用户故事组织于 `tests/`。

## Complexity Tracking

> 本节仅在 Constitution Check 存在需说明的违规时填写。

本计划无宪法违规项，无需填写。
