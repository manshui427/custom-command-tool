---
description: "Task list for 主命令框架与文本替换子命令 (trt)"
---

# Tasks: 主命令框架与文本替换子命令 (trt)

**Input**: Design documents from `specs/001-text-replace-tool/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: 本项目规格未强制 TDD。考虑到替换/备份/撤销属破坏性操作且宪法原则 V 强调"编译/测试驱动迭代"，
为每个用户故事保留集成测试任务（建议执行，可按需取舍）。

**Organization**: 任务按用户故事分组，每个故事可独立实现与测试。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行（不同文件、无未完成依赖）
- **[Story]**: 所属用户故事（US1/US2/US3）
- 每条任务含明确文件路径

## Path Conventions

单一 Rust 项目，源码在仓库根 `src/`，集成测试在 `tests/`（见 plan.md 项目结构）。

---

## Phase 1: Setup（共享基础设施）

**Purpose**: 初始化 Cargo 项目与依赖、配置质量工具

- [X] T001 在仓库根用 `cargo init --name cct` 初始化二进制项目，生成 `Cargo.toml` 与 `src/main.rs`，并确认 `Cargo.lock` 入库
- [X] T002 在 `Cargo.toml` 声明依赖（`cargo add`）：clap(derive)、ignore、rayon、aho-corasick、regex、zip、memchr、tempfile、indicatif、anyhow、thiserror、serde、serde_json、chrono，执行 `cargo build` 确认依赖可解析
- [X] T003 [P] 在仓库根添加 `rustfmt.toml`（可空）与 `.gitignore`（忽略 `/target`），并运行 `cargo fmt`、`cargo clippy` 确认工具链就绪
- [X] T004 [P] 创建源码目录骨架：`src/cli.rs`、`src/registry.rs`、`src/commands/mod.rs`、`src/commands/trt/mod.rs`、`src/util/mod.rs`（占位模块 + 中文文档注释），并在 `main.rs`/`mod.rs` 中声明 `mod`

**Checkpoint**: `cargo build` 通过，空骨架可编译

---

## Phase 2: Foundational（阻塞性前置，所有用户故事的基础）

**Purpose**: 构建子命令框架与跨切面工具，US1/US2/US3 均依赖

**⚠️ CRITICAL**: 本阶段完成前，任何用户故事不可开工

- [X] T005 [P] 在 `src/registry.rs` 实现 `SubcommandInfo { name, alias, description }` 与静态注册表 `SUBCOMMANDS`，登记 trt（名称 `text-replace-tool`、别名 `trt`、描述 `文本替换工具`）；含唯一性约定注释（data-model 实体 1）
- [X] T006 在 `src/cli.rs` 用 clap derive 定义顶层 `Cli`：全局 `-ls` 标志、子命令枚举（含 trt 占位），`-h/--help` 由 clap 提供（FR-002/FR-003/FR-005，contracts/cli-main.md）
- [X] T007 [P] 在 `src/util/fs_atomic.rs` 实现"同目录临时文件 + 原子改名"写回封装（tempfile + persist/rename，保留原文件权限位），跨 Windows/Linux（FR-019a，research §7）
- [X] T008 [P] 在 `src/util/progress.rs` 封装 indicatif spinner + 已处理/已修改计数，支持开关（输出走 stderr）（FR-017，research §10）
- [X] T009 [P] 在 `src/commands/trt/mod.rs` 或 `src/error.rs` 定义结构化错误（thiserror）与退出码映射约定，main 边界用 anyhow（FR-022，research §11）

**Checkpoint**: 框架与工具就绪，子命令可被分发

---

## Phase 3: User Story 1 - 在目录中批量替换文本 (Priority: P1) 🎯 MVP

**Goal**: 对目录内所有文本文件执行单组或多组"旧→新"替换，跳过二进制，超大目录流式并行、内存恒定

**Independent Test**: 准备含文本+二进制文件的目录，运行 `cct trt -d <dir> -o foo -n bar`（及 `--rules`、`-r 1`、`-c 0`），
验证文本被正确替换、二进制不变、报告修改数；多组规则单遍同时生效无级联

### Implementation for User Story 1

- [X] T010 [P] [US1] 在 `src/commands/trt/args.rs` 用 clap derive 定义 trt 全部参数（-d/-o/-n/--rules/-b/-u/-c/-r/--progress）及 1/0 取值解析，构建 `TrtOptions`、`TrtMode`（data-model 实体 4/5，contracts/cli-trt.md）
- [X] T011 [US1] 在 `src/commands/trt/args.rs` 实现参数校验：目录存在性、非撤销模式下 `-o/-n` 或 `--rules` 必居其一、旧文本非空（FR-006/FR-007/FR-022）
- [X] T012 [P] [US1] 在 `src/commands/trt/rules.rs` 实现 `ReplacementRule`/`RuleSet`：单组 `-o/-n` 与 `--rules` 文件解析（制表符分隔、`#` 注释与空行忽略、非法行报行号）、`-o/-n` 追加进规则集（FR-007a/FR-007b，data-model 实体 2/3）
- [X] T013 [US1] 在 `src/commands/trt/matcher.rs` 实现单遍同时匹配引擎：字面用 aho-corasick（含 `ascii_case_insensitive`）、正则用 regex（含 `(?i)` 与 `$1/${name}` 捕获组）；统一最左不重叠+规则顺序优先+无级联语义（FR-008a/FR-016a/FR-016b，research §5）
- [X] T014 [P] [US1] 在 `src/commands/trt/scanner.rs` 实现并行目录遍历（ignore::WalkBuilder，`follow_links(false)`）+ 二进制检测（memchr 扫描头部 NUL）+ 文件条目分发（FR-008/FR-009/FR-018，research §3/§4）
- [X] T015 [US1] 在 `src/commands/trt/replacer.rs` 实现单文件替换：小文件整体载入、超阈值大文件流式替换，调用 matcher 与 fs_atomic 原子写回，产出 `FileOutcome`（FR-019/FR-019a，data-model 实体 6，research §6/§7）
- [X] T016 [US1] 在 `src/commands/trt/mod.rs` 编排 Replace 流程：解析→构建规则集→并行扫描+替换→汇总 `RunSummary`→stdout 打印摘要（FR-021，data-model 实体 7）
- [X] T017 [US1] 在 `src/main.rs`/`src/cli.rs` 接通 trt 分发，使 `cct trt ...` 与 `cct text-replace-tool ...` 可执行；运行 `cargo build` 与一次真实目录手动验证
- [X] T018 [P] [US1] 在 `tests/trt_replace.rs` 编写集成测试（assert_cmd + tempfile）：单组替换、多组规则单遍无级联、正则捕获组、大小写不敏感、目标不存在报错（US1 验收场景）
- [X] T019 [P] [US1] 在 `tests/trt_binary_skip.rs` 编写集成测试：含 NUL 的二进制文件被跳过且字节不变、非 UTF-8 文件按二进制处理（FR-009/SC-004，spec 边界）

**Checkpoint**: US1 可独立运行——基本与批量替换功能完整且可测，构成 MVP

---

## Phase 4: User Story 2 - 备份与一键撤销 (Priority: P2)

**Goal**: 替换前对被改文件结构化 ZIP 备份（含来源目录元数据），支持按目标目录定位的一键撤销

**Independent Test**: 启用备份执行替换→确认同级 backup/ 生成带时间戳 ZIP（仅含被改文件、保留层级）；
执行 `-u 1`→目录还原且该备份被删除；`-b 0` 时不生成备份

### Implementation for User Story 2

- [X] T020 [P] [US2] 在 `src/commands/trt/backup.rs` 实现 `BackupManifest`（serde_json：source_directory 规范化路径、created_at、tool_version、file_count）（FR-013a，data-model 实体 9）
- [X] T021 [US2] 在 `src/commands/trt/backup.rs` 实现备份归档：流式写出 `<目标目录父目录>/backup/backup_yyyyMMddHHmmss.zip`（chrono 时间戳），仅含被改文件、内部保留相对层级、写入 manifest（FR-011/FR-012/FR-013，research §8，data-model 实体 8）
- [X] T022 [US2] 在 `src/commands/trt/replacer.rs`/`mod.rs` 接入"先备份后改"：覆盖前将原始内容写入归档，并据 `-b` 开关控制启停（FR-010/FR-011）
- [X] T023 [US2] 在 `src/commands/trt/undo.rs` 实现撤销：扫描同级 backup/*.zip 读取各 manifest，按 source_directory==当前 `-d`（规范化比较）筛选取最近，解压覆盖还原后删除该 ZIP；无匹配备份给出提示并非零退出（FR-014，research §9，spec 边界）
- [X] T024 [US2] 在 `src/commands/trt/mod.rs` 编排 Undo 流程分支（`-u 1` 时跳过规则集要求，进入 undo），接通到 trt 入口（FR-014）
- [X] T025 [P] [US2] 在 `tests/trt_backup_undo.rs` 编写集成测试：备份生成与内部层级、撤销 100% 还原并删除归档、`-b 0` 不备份、多目录共享同一 backup/ 按目录归属互不误还原（US2 验收场景，SC-005/SC-006）

**Checkpoint**: US1 + US2 均可独立工作；替换具备完整安全网

---

## Phase 5: User Story 3 - 主命令探索与帮助 (Priority: P3)

**Goal**: `cct -ls` 列出子命令、`cct -h` 标准帮助、无子命令时等同 `-ls`

**Independent Test**: 分别运行 `cct -ls`、`cct -h`、`cct`（无参数），验证三者输出符合契约

### Implementation for User Story 3

- [X] T026 [US3] 在 `src/main.rs`/`src/cli.rs` 实现顶层分发逻辑：带 `-ls` 或无子命令时调用 registry 打印子命令清单（`<别名>  <描述>`）至 stdout，退出码 0（FR-002/FR-004，contracts/cli-main.md）
- [X] T027 [US3] 校验 `-h/--help` 输出标准帮助、未知子命令报错入 stderr 且非零退出（FR-003，contracts/cli-main.md）
- [X] T028 [P] [US3] 在 `tests/main_framework.rs` 编写集成测试：`cct -ls`、`cct -h`、`cct`（无参数等同 -ls）、未知子命令退出码（US3 验收场景，SC-008）

**Checkpoint**: 全部用户故事独立可用

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: 跨故事的收尾与质量加固

- [X] T029 [P] 为所有公共模块、函数、子命令补全中文文档注释（`///`），确保对外项均有用途/参数/错误说明（宪法原则 III）
- [X] T030 运行 `cargo fmt` 与 `cargo clippy`，修复全部格式问题与告警直至干净通过（宪法原则 II/V）
- [X] T031 [P] 用 `--release` 构建并在一个较大目录上手动验证进度反馈及时性与内存平稳（SC-001/SC-002/SC-003）
- [X] T032 [P] 跨平台核对：路径分隔、换行、原子改名在 Windows 与 Linux 行为一致（FR-020/SC-007）（代码层用 PathBuf/ignore/tempfile 等跨平台库；已在 Windows 实测，Linux 侧待实机验证）
- [X] T033 按 `specs/001-text-replace-tool/quickstart.md` 逐条走查命令，确认与实际行为一致

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖，最先执行
- **Foundational (Phase 2)**: 依赖 Setup 完成 — 阻塞所有用户故事
- **User Stories (Phase 3-5)**: 均依赖 Foundational 完成
  - US1 (P1)：仅依赖 Foundational，可独立完成（MVP）
  - US2 (P2)：依赖 Foundational；逻辑上在 US1 产生替换/被改文件后才有备份对象，建议在 US1 之后
  - US3 (P3)：仅依赖 Foundational，与 US1/US2 完全独立，可随时并行
- **Polish (Phase 6)**: 依赖目标用户故事完成

### User Story Dependencies

- **US1 (P1)**: Foundational 后即可开始，无对其他故事的依赖
- **US2 (P2)**: Foundational 后可开始；与 US1 共享 replacer/mod.rs，建议串行于 US1 之后以减少冲突
- **US3 (P3)**: Foundational 后可开始，独立于 US1/US2

### Within Each User Story

- args/rules（数据与解析）→ matcher（匹配）→ scanner（遍历）→ replacer（替换）→ mod（编排）→ 接通入口 → 集成测试
- US2：manifest → 备份归档 → 接入"先备份后改" → 撤销 → 编排
- 标 [P] 的任务可并行（不同文件）

### Parallel Opportunities

- Setup：T003、T004 可并行
- Foundational：T005、T007、T008、T009 可并行（不同文件；T006 依赖 T005 的注册表类型）
- US1：T010/T012/T014 可并行起步；T018/T019 测试可并行
- US3 整体可与 US1/US2 并行（独立文件，仅 main.rs 分发处需协调）
- Polish：T029、T031、T032 可并行

---

## Parallel Example: User Story 1

```bash
# US1 起步阶段可并行的任务（不同文件）：
Task: "T010 [US1] 定义 trt 参数与 TrtOptions in src/commands/trt/args.rs"
Task: "T012 [US1] 实现规则集与规则文件解析 in src/commands/trt/rules.rs"
Task: "T014 [US1] 实现并行扫描与二进制检测 in src/commands/trt/scanner.rs"

# US1 测试阶段可并行：
Task: "T018 [US1] 替换功能集成测试 in tests/trt_replace.rs"
Task: "T019 [US1] 二进制跳过集成测试 in tests/trt_binary_skip.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. 完成 Phase 1 Setup
2. 完成 Phase 2 Foundational（关键，阻塞所有故事）
3. 完成 Phase 3 US1（含批量规则）
4. **停下验证**：在真实目录独立测试 US1 替换
5. 可作为 MVP 交付/演示

### Incremental Delivery

1. Setup + Foundational → 框架就绪
2. 加 US1 → 独立测试 → 交付（MVP：能替换）
3. 加 US2 → 独立测试 → 交付（安全网：备份+撤销）
4. 加 US3 → 独立测试 → 交付（易用性：探索与帮助）
5. Polish → 文档注释、clippy、跨平台、性能核对

---

## Notes

- [P] = 不同文件、无未完成依赖，可并行
- [Story] 标签将任务映射到用户故事以便追踪
- 每个用户故事应可独立完成并测试
- 每完成一个任务或一组逻辑后建议提交一次
- 在任意 Checkpoint 可停下独立验证对应故事
- 避免：含糊任务、同文件并行冲突、破坏故事独立性的跨故事依赖
