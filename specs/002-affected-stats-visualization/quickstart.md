# 快速上手：被影响文件统计与图形界面

**Feature**: 002-affected-stats-visualization | **Date**: 2026-06-02

演示本特性交付后的两项新能力：以被影响文件为核心的统计，以及 `cct gui` 图形界面。

---

## 1. 构建

```bash
# 默认构建（含图形界面，面向桌面）
cargo build --release
# 产物：target/release/cct（Windows 为 cct.exe）

# CLI-only 构建（不含图形界面，体积更小、可静态交叉编译）
cargo build --release --no-default-features

# 交叉编译 Linux CLI（无 GUI 依赖，沿用全局 musl + rust-lld 配置）
cargo build --release --no-default-features --target x86_64-unknown-linux-musl
```

## 2. 以被影响文件为核心的统计（需求 1）

正常使用 trt，替换完成后摘要以"被影响文件"为主体：

```bash
cct trt -d ./project -o foo -n bar
```

预期输出（示意）：

```text
被影响 37 个文件，共替换 152 处。
（扫描 1240，跳过二进制 8，失败 0）
备份已生成：../backup/backup_20260602_101530.zip
```

- 无命中时：`被影响 0 个文件`，命令成功退出。
- 扫描/跳过/失败等信息仍保留，但不再是首要信息。

## 3. 图形界面（需求 2）

启动桌面图形窗口：

```bash
cct gui
```

在窗口中：

1. 选择/填写**目标目录**（可点「选择目录」调用系统对话框）。
2. 填写**旧文本**、**新文本**；或选择**规则文件**做批量替换。
3. 按需切换**备份 / 大小写 / 正则 / 撤销 / 进度**等开关。
4. 点击「**执行**」：
   - 执行期间界面显示进度且不冻结；
   - 若为撤销操作，会先弹出确认；
   - 完成后展示**被影响文件数、总替换次数**与**被影响文件列表**（每文件替换次数，可滚动）。

无图形环境（如纯无头服务器）下运行 `cct gui`：

```text
当前环境无图形界面支持，请在桌面环境运行，或改用命令行：cct trt -d <目录> -o <旧> -n <新>
```

（进程以非零码退出，不崩溃。）

## 4. 发现 gui 子命令

```bash
cct -ls
```

预期清单包含 gui：

```text
可用子命令：
  trt (text-replace-tool)  文本替换工具
  gui                      图形操作界面
```

## 5. 验证（开发者）

```bash
cargo test                       # 含新增的统计口径与表单映射测试
cargo clippy --all-targets       # 默认含 gui feature
cargo clippy --all-targets --no-default-features   # CLI-only 也应干净
cargo fmt --check
```

---

## 验收对照速查

| 想验证 | 方式 | 期望 |
|--------|------|------|
| 被影响文件统计（US1） | `cct trt -d <dir> -o a -n b` | 摘要以"被影响 N 个文件、共替换 M 处"为主体 |
| 无命中 | 替换不存在的文本 | "被影响 0 个文件"，退出码 0 |
| 图形界面执行（US2） | `cct gui` → 填表 → 执行 | 效果等同命令行，界面展示统计 + 文件列表 |
| 无图形环境提示 | 无头环境 `cct gui` | 明确提示并非零退出，不崩溃 |
| 向后兼容 | `cct` / `cct -ls` / `cct trt …` | 行为与升级前一致 |
| CLI-only 构建 | `--no-default-features` | 不含 GUI 依赖，可静态交叉编译 |
