# Phase 0 研究：技术决策 (trt + 主命令框架)

**Feature**: 001-text-replace-tool | **Date**: 2026-06-01

本文件固化实现所需的关键技术选型。所有规格中的歧义已在 `/speckit-clarify` 阶段解决并写入 spec.md，
故此处不再有 NEEDS CLARIFICATION，仅记录技术方案的决策、理由与备选。具体 crate 的精确 API 行为
在实现阶段以 `cargo build`/`cargo test` 反馈驱动验证与微调（符合宪法原则 V）。

---

## 1. CLI 解析框架

- **Decision**: 使用 `clap` v4 的 derive 风格定义顶层命令与 `trt` 子命令。
- **Rationale**: clap 是 Rust 社区事实标准，自动生成 `--help`、子命令分发、参数校验与短/长选项，
  直接满足 FR-002/FR-003/FR-005~FR-007 及宪法原则 I（每个子命令须有 `--help`）。derive 风格代码简洁、
  贴近"官方推荐写法"（原则 II）。
- **主命令特殊行为**: `-ls` 与"无子命令时等同 `-ls`"需自定义处理——顶层用 `arg_required_else_help(false)`，
  在 `main.rs` 中判断：若未匹配到子命令或带 `-ls`，则调用 registry 打印子命令清单；`-h` 交给 clap 默认帮助。
- **Alternatives considered**:
  - `argh`/`pico-args`：更轻量但需手写帮助与子命令分发，违背"优先官方推荐写法"。
  - 纯手写解析：过度增加维护成本，违背原则 IV。

## 2. 子命令注册表（可扩展框架）

- **Decision**: 用一个静态的 `&[SubcommandInfo { name, alias, description }]` 切片集中登记子命令元信息，
  供 `-ls` 列举；实际分发仍由 clap 子命令枚举完成。
- **Rationale**: 项目目标是"通过子命令快速完成各种自定义命令"，未来会增加更多子命令。集中注册表使
  `-ls` 输出与新增子命令解耦，新增子命令时只需加一条登记。保持最小化，不引入插件/动态注册等过度设计（原则 IV）。
- **Alternatives considered**: 过程宏/inventory 自动收集——本期仅 1 个子命令，属过度抽象，拒绝。

## 3. 并行目录遍历（边扫描边处理）

- **Decision**: 使用 `ignore::WalkBuilder` 构建并行 walker，结合 `rayon` 处理已发现的文件条目；
  默认不跟随符号链接（`follow_links(false)`）。
- **Rationale**: FR-018/SC-002 要求"扫描与处理同步进行、数秒内反馈进度"。`ignore` crate（ripgrep 同源）
  提供高性能并行目录遍历，天然支持边遍历边回调；其条目流可与 rayon 协作实现流式并行，避免先收集全部路径
  （那样会在 300GB/海量文件场景耗时且占内存）。`follow_links(false)` 满足 spec 边界"不跟随符号链接"。
- **rayon 协作方式**: 优先使用 `ignore` 的并行 walker（`build_parallel` + 多线程访问者），在访问者内直接处理文件；
  若需更细粒度的 CPU 并行替换，再将文件内容处理交给 rayon 线程池。两者结合而非简单 `par_bridge`，
  以兼顾 I/O 并发与 CPU 并行。
- **Alternatives considered**:
  - `walkdir`：单线程遍历，需自行 `par_bridge` 并行；可用但并发遍历能力弱于 `ignore`。作为备选保留。
  - 自写 `std::fs::read_dir` 递归 + 线程池：重复造轮子，违背原则 II/IV。

## 4. 二进制文件检测

- **Decision**: 读取文件头部一段固定字节（如前 8KB），用 `memchr` 扫描 NUL 字节（`\0`）；含 NUL 即判定为二进制并跳过。
  同时，文本处理时若内容无法按 UTF-8 解码，也归类为二进制跳过。
- **Rationale**: NUL 字节启发式是业界通用、低成本且高准确的文本/二进制判别法（git、ripgrep 等采用）。
  满足 FR-009/SC-004（二进制 100% 跳过）与 spec 假设（"是否包含空字节"）。仅读头部，避免大文件全量读入。
- **Alternatives considered**:
  - 基于扩展名白/黑名单：不可靠（无扩展名、误判），拒绝。
  - `content_inspector` crate：可用但增加依赖；NUL 启发式已足够，遵循 YAGNI。

## 5. 多组替换规则的"单遍同时匹配"引擎（批量替换核心）

- **Decision**: 字面模式（`-r 0`）用 `aho-corasick` 多模式自动机一次扫描同时匹配所有规则旧文本；
  正则模式（`-r 1`）用 `regex::RegexSet` 定位 + 各 `Regex` 配合，或将多规则合并为带分支的单个 Regex。
  统一采用**最左最长 + 不重叠**消解策略：每个位置至多被一条规则命中，重叠时按规则在规则集中的先后顺序优先，
  替换输出不再参与后续匹配（无级联）。
- **Rationale**: 直接实现 spec 澄清的"单遍同时匹配"语义（FR-008a）。`aho-corasick` 专为多模式单遍匹配设计，
  性能与正确性俱佳，并提供 leftmost-first/leftmost-longest 等匹配语义可映射"按规则顺序优先"。
  字面与正则分流：绝大多数批量替换是字面，aho-corasick 远快于多次正则。
- **大小写不敏感**（`-c 0`）: 字面模式用 `aho-corasick` 的 `ascii_case_insensitive`（或对 Unicode 做规范化）；
  正则模式加 `(?i)` 标志。统一对所有规则生效（FR-016b）。
- **捕获组**（FR-016a）: 仅正则模式支持；用 `Regex::replace`/`replacen` 的替换串语法（`$1`、`${name}`），
  未匹配组按空串——这是 `regex` crate 替换语法的既有行为。
- **Alternatives considered**:
  - 对每条规则各扫一遍并链式替换：会产生级联替换（违背 FR-008a），且 O(规则数 × 文件大小)，拒绝。
  - 仅用一个大正则交替：字面场景下性能不如 aho-corasick，且转义繁琐；保留为正则模式的实现手段之一。

## 6. 大文件流式替换（带重叠窗口）

- **Decision**: 设阈值（如 8MB，可常量定义）。小于阈值的文件整体读入内存替换；超过阈值的文件按块读取，
  相邻块保留"重叠窗口"，窗口长度 = 最长规则旧文本长度 − 1（正则则取一个保守上界），以保证跨块边界的匹配不被漏掉。
- **Rationale**: 直接实现 spec 澄清的"阈值混合"策略（FR-019），满足 SC-003（内存恒定）。重叠窗口解决流式处理
  最大难点——模式跨越块边界。aho-corasick 支持流式 `stream_find_iter`，可进一步简化字面模式的跨块匹配。
- **正则流式的限制**: 任意正则在流式分块下的跨块匹配较复杂（可能匹配任意长）。决策：正则模式对超阈值文件
  采用"扩大块 + 重叠"近似，并在实现时验证；若正则匹配长度不可控，则对该文件回退为整体读入并在文档注明权衡。
- **Alternatives considered**:
  - 一律整体读入：超大单文件（大日志/SQL 导出）会爆内存，违背 SC-003，拒绝。
  - 内存映射 `mmap`：跨平台与安全性复杂（unsafe），违背原则 II，拒绝。

## 7. 崩溃安全的原子写回

- **Decision**: 用 `tempfile::NamedTempFile::new_in(<目标文件所在目录>)` 在**同目录**创建临时文件，
  写完后 `persist`/`rename` 原子覆盖原文件。封装于 `util/fs_atomic.rs`。
- **Rationale**: 实现 FR-019a 与 FR-022（"不留下部分损坏的目录"）。同目录建临时文件确保 rename 在同一文件系统
  （跨文件系统 rename 非原子）。POSIX `rename` 与 Windows `ReplaceFile`/`MoveFileEx` 均提供原子覆盖语义。
- **权限/元数据**: 原子替换需保留原文件权限位（Unix mode）；实现时从原文件读取并应用到临时文件。
- **Alternatives considered**:
  - 就地 `OpenOptions` 覆盖写：崩溃留半写文件，违背 FR-019a，拒绝。
  - 写 `.bak` 再改名：与 ZIP 备份职责重叠，且 `.bak` 散落目录，拒绝。

## 8. ZIP 备份归档 + 来源目录元数据

- **Decision**: 用 `zip` crate 流式写出单个 `backup_yyyyMMddHHmmss.zip` 至"目标目录同级"的 `backup/` 目录；
  仅对**实际被修改**的文件，在覆盖前将其原始内容写入 ZIP，内部路径 = 相对目标目录的原始层级；
  另在 ZIP 内写一个清单条目（如 `.cct-manifest.json`）记录来源目标目录的绝对/规范化路径与时间戳。
- **Rationale**: 实现 FR-011~FR-013、FR-013a。流式写入 ZIP 避免备份占用额外大内存（性能优化第 3 点）。
  清单中的来源目录元数据是"按目标目录定位撤销"（FR-014）在多目录共享同一 `backup/` 时的归属判据。
  内部保留层级使 SC-006（解压可直接覆盖还原）成立。
- **时机**: 备份必须在替换写回**之前**完成对应文件的归档（先备份后改），保证可还原。
- **Alternatives considered**:
  - 复制整个目录再替换：300GB 场景空间/时间不可接受，违背"仅备份被改文件"，拒绝。
  - 每文件独立备份文件：碎片化、不便撤销，spec 明确要求单个 ZIP，拒绝。
  - 元数据存 ZIP 文件名：文件名信息有限且易冲突，改用内部清单更稳。

## 9. 按目标目录定位的撤销

- **Decision**: `-u` 模式扫描目标目录同级 `backup/` 下所有 ZIP，读取各自清单的来源目录元数据，
  筛选出来源 == 当前 `-d`（规范化路径比较）的归档，取时间戳最近的一个，解压覆盖还原后删除该 ZIP。
- **Rationale**: 实现 FR-014 与 spec 边界（多目录共享 backup 互不干扰、按时间逐次回退）。规范化路径比较
  避免相对/绝对路径或大小写差异导致误判。
- **Alternatives considered**:
  - 仅按时间取最新（不看来源）：多目录共享时会误还原他人备份，违背澄清决策，拒绝。
  - 维护独立索引文件：增加状态一致性负担；清单内嵌于各 ZIP 自洽即可，遵循 YAGNI。

## 10. 进度显示

- **Decision**: 用 `indicatif` 提供进度反馈；`--progress 0` 时不创建进度条。因总数未知（边扫边处理），
  采用 spinner + 已处理/已修改计数，而非百分比进度条。
- **Rationale**: 满足 FR-017/SC-002。边扫描边处理意味着分母未知，spinner 更诚实；输出走 stderr 以不污染 stdout。
- **Alternatives considered**: 自写 stderr 刷新——重复造轮子；先扫总数再显示百分比——违背"不等扫描完成"，拒绝。

## 11. 错误处理

- **Decision**: 库内模块用 `thiserror` 定义结构化错误类型；`main.rs` 边界用 `anyhow` 聚合并映射为退出码与
  stderr 友好信息。非法输入（目录不存在、旧文本为空、规则文件非法、正则无效）→ 非零退出码（FR-022）。
- **Rationale**: 贴合 Rust 官方推荐的"库用具体错误、应用用 anyhow"分工（原则 II）。满足 FR-021/FR-022 与
  CLI 约定（错误进 stderr、退出码非 0）。
- **Alternatives considered**: 到处 `unwrap`/`panic`——破坏 FR-022 与用户体验，拒绝。

---

## 依赖清单汇总（拟写入 Cargo.toml）

| crate | 用途 | 关键性 |
|-------|------|--------|
| clap (v4, derive) | CLI 解析与帮助 | 必需 |
| ignore | 并行目录遍历、跳过符号链接 | 必需 |
| rayon | CPU 并行处理 | 必需 |
| aho-corasick | 多模式字面单遍匹配 | 必需 |
| regex | 正则匹配与捕获组替换 | 必需 |
| zip | 备份归档流式压缩 | 必需 |
| memchr | 二进制检测（NUL 扫描） | 必需 |
| tempfile | 同目录临时文件 + 原子改名 | 必需 |
| indicatif | 进度条/spinner | 必需 |
| anyhow | 应用层错误聚合 | 必需 |
| thiserror | 库层结构化错误 | 必需 |
| serde + serde_json | 备份清单（来源目录元数据）序列化 | 必需 |
| chrono | 时间戳格式化（yyyyMMddHHmmss） | 必需 |

注：具体版本号在 `cargo add` 时取当前稳定版并锁入 `Cargo.lock`（宪法要求提交 Cargo.lock）。
