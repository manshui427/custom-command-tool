# Phase 0 研究：被影响文件统计与图形化操作界面

**Feature**: 002-affected-stats-visualization | **Date**: 2026-06-02

固化本特性的关键技术决策。规格歧义已在 `/speckit-clarify` 解决，此处聚焦"如何实现"。
具体 crate 版本在实现阶段 `cargo add` 时取镜像可解析的稳定版并锁入 `Cargo.lock`（宪法原则 II）。

> 备注：本环境使用 tuna 镜像源；`cargo add` 偶发 SSL 证书吊销检查告警（CRYPT_E_REVOCATION_OFFLINE），
> 重试即可（`rfd` 已实测可解析到 v0.17.2）。

---

## 1. 被影响文件的收集（需求 1）

- **Decision**: 在 `run_replace` 的并行处理中，除现有原子计数外，额外收集"被影响文件"列表
  `Vec<AffectedFile { path, replacements }>`（用 `Mutex<Vec<_>>` 聚合）；将其纳入 `RunSummary`
  作为影响报告的主体。CLI 摘要与 GUI 列表共用此数据。
- **Rationale**: 实现 FR-001/FR-002/FR-005 与 FR-010（GUI 文件列表）。被影响文件即 `FileStatus::Modified`
  的文件，`FileOutcome` 已含 `path` 与 `replacements`，收集成本极低。
- **呈现口径**: CLI 摘要改为以"被影响 N 个文件、共替换 M 处"为首行；扫描/跳过/失败作为次要信息保留（FR-003）。
- **Alternatives considered**: 二次遍历目录统计——多余 I/O；仅保留聚合计数不留列表——无法满足 GUI 文件列表与 FR-010。
- **规模保护**: 被影响文件可能很多，列表内存与 GUI 渲染需有上限/截断（GUI 用滚动区，必要时提示"仅显示前 N 项"）。

## 2. GUI 框架选型

- **Decision**: 采用 `eframe`（`egui` 的官方应用框架）实现桌面原生窗口。
- **Rationale**:
  - 纯 Rust、即时模式、跨平台（Windows/Linux/macOS），可**静态编入单一可执行文件**，契合宪法"单一可执行文件"。
  - 即时模式 GUI 心智简单（每帧重绘当前状态），与"表单 + 执行 + 结果展示"的需求高度契合，避免复杂的控件树/事件绑定。
  - 社区事实标准，维护活跃，文档完善（原则 II）。
- **Alternatives considered**:
  - `iced`（Elm 架构/保留模式）——架构更重，消息建模成本高；
  - `tauri`——需打包 Web 前端 + 系统 WebView，违背"单一可执行文件/简单优先"，且引入前端技术栈；
  - `gtk`/`qt` 绑定——需系统库与较重的 FFI，跨平台分发复杂；
  - 原生 Win32/系统对话框——不跨平台、表单能力弱。

## 3. GUI 的 Cargo feature 门控

- **Decision**: 新增 Cargo feature `gui`（**默认开启**），把 `eframe`/`egui`/`rfd` 声明为 `optional = true`
  并归入该 feature；`commands/gui` 整个模块加 `#[cfg(feature = "gui")]`。`cct gui` 在未启用 feature 的构建下
  输出明确提示并以非零码退出。
- **Rationale**:
  - 保留现有 **Windows→Linux musl 静态交叉编译**能力：`cargo build --no-default-features --target x86_64-unknown-linux-musl`
    得到零 GUI 依赖的 CLI-only 二进制（GUI 库在 Linux 需 X11/Wayland，静态交叉编译困难）。
  - GUI 重依赖不拖累纯命令行场景的体积与构建（呼应原则 IV）。
- **取舍**: 默认构建含 GUI（面向桌面用户）；发布/交叉编译 CLI 版时显式 `--no-default-features`。
- **Alternatives considered**: 不门控——破坏交叉编译且全场景增大体积；GUI 拆为独立 crate/可执行——违背"单一可执行文件"。

## 4. 进度上报的解耦（CLI 与 GUI 共用替换核心）

- **Decision**: 将 `run_replace` 内部"直接操作 indicatif"改为接受一个**进度回调**
  （`progress: impl Fn(ProgressUpdate) + Sync` 或等价的轻量 sink）。CLI 提供更新 indicatif 的回调；
  GUI 提供把 `ProgressUpdate` 发送到 mpsc 通道的回调。
- **Rationale**: 让两个前端复用同一替换/统计逻辑（DRY），各自决定如何呈现进度。这是支撑双前端**必需**的最小抽象，
  不引入额外层次（原则 IV）。
- **Alternatives considered**: 为 GUI 复制一份 trt 逻辑——重复且行为易漂移；引入完整的事件总线/观察者框架——过度设计。

## 5. GUI 执行不冻结：后台线程 + 通道

- **Decision**: eframe 在主线程运行 UI；点击「执行」后，在**后台线程**（`std::thread::spawn`）调用 trt 核心，
  通过 `std::sync::mpsc` 通道把"进度更新"与"最终结果（影响报告/错误）"回传；UI 每帧轮询通道并 `ctx.request_repaint()`。
- **Rationale**: 实现 FR-012/SC-006（界面不冻结、持续响应）。窗口事件循环必须在主线程，耗时替换必须离开主线程。
- **并发与防重入**: 执行期间禁用「执行」按钮，UI 进入 Running 状态机，结束后回到 Idle 并展示结果（呼应"防止重入"边界）。
- **Alternatives considered**: 在 UI 线程内同步执行——大目录会冻结窗口；async 运行时——对一次性后台任务属过度引入。

## 6. 文件/目录选择对话框

- **Decision**: 用 `rfd`（Rust File Dialog）提供原生"选择目录""选择规则文件"对话框（FR-017）。
- **Rationale**: 跨平台、调用系统原生对话框、API 简单；同样置于 `gui` feature 下。
- **Alternatives considered**: 在 egui 内自绘文件浏览器——重复造轮子且体验差。

## 7. 无图形环境的处理

- **Decision**: `gui::run()` 尝试启动 eframe；当无可用显示/初始化失败时，捕获错误并返回
  `CctError`，由 main 边界输出明确中文提示（如"当前环境无图形界面支持，请在桌面环境运行，或使用命令行 cct trt …"）
  并以非零码退出（FR-015、SC-007）。
- **Rationale**: 满足无头/远程无转发场景下"给出提示而非崩溃"。
- **Alternatives considered**: 直接 panic——违背 FR-015 与宪法错误处理约定。

## 8. GUI 与撤销等破坏性操作

- **Decision**: 表单中的"撤销"开关或撤销动作，在真正执行前弹出确认对话框（egui 模态/确认区域）（FR-016）。
- **Rationale**: 撤销会改写目录，需二次确认防误触。

## 9. 跨平台与交叉编译影响（重要）

- **Decision**: 记录并接受如下事实：含 GUI 的构建在 Linux 需要 X11/Wayland 等系统运行库；
  Windows→Linux 的**静态 musl 交叉编译仅对 CLI-only**（`--no-default-features`）保证可用。
  GUI 的 Linux 构建建议在 Linux 本机或带相应系统库的环境进行。
- **Rationale**: 既满足 GUI 跨平台运行（FR-014），又不破坏既有交叉编译能力。
- **README/quickstart 将明确**：默认构建含 GUI（桌面）；交叉编译 Linux CLI 用 `--no-default-features`。

## 10. 现有功能与向后兼容

- **Decision**: 不改变 `cct`（无参=列子命令）、`cct -ls`、`cct trt …` 的任何行为与输出；
  仅新增 `gui` 子命令与注册表条目，并对 trt 摘要做"以被影响文件为核心"的呈现调整（不改变替换/备份/撤销实际结果）。
- **Rationale**: FR-013/SC-003/SC-004。摘要文案调整属呈现层，需以集成测试确认替换结果逐字节不变。
- **既有测试**: 现有 4 个集成测试若断言了旧摘要文案，需相应更新；替换/备份/撤销结果断言保持不变。

---

## 依赖增量汇总（拟写入 Cargo.toml，置于 `gui` feature）

| crate | 用途 | 关键性 |
|-------|------|--------|
| eframe (+egui) | 桌面即时模式 GUI 框架与窗口 | gui feature 必需 |
| rfd | 原生文件/目录选择对话框 | gui feature 必需 |

`[features] default = ["gui"]`；`eframe`/`rfd` 标记 `optional = true` 并由 `gui` 启用。
