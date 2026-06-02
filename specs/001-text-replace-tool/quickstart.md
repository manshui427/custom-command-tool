# 快速上手：custom-command-tool (`cct`)

**Feature**: 001-text-replace-tool | **Date**: 2026-06-01

本文档面向使用者，演示如何构建并使用 `cct` 及其 `trt` 子命令。

---

## 1. 构建

```bash
# 调试构建
cargo build

# 发布构建（推荐用于处理大型目录，性能更佳）
cargo build --release
# 产物：target/release/cct（Windows 为 cct.exe）
```

## 2. 查看可用子命令与帮助

```bash
# 列出所有子命令
cct -ls

# 不带子命令，效果同 -ls
cct

# 查看帮助
cct -h

# 查看 trt 子命令的帮助
cct trt -h
```

预期 `cct -ls` 输出：

```text
可用子命令：
  trt (text-replace-tool)  文本替换工具
```

## 3. trt：基本文本替换

将 `./project` 目录下所有文本文件中的 `foo` 替换为 `bar`（默认启用备份）：

```bash
cct trt -d ./project -o foo -n bar
```

完成后输出摘要，例如：

```text
已扫描 1240 个文件，修改 37 个，跳过二进制 8 个，失败 0 个，共替换 152 处。
备份已生成：../backup/backup_20260601_181530.zip
```

## 4. 批量替换（多组规则）

准备规则文件 `rules.tsv`（制表符分隔，`#` 开头为注释，空行忽略）：

```text
# 旧文本<TAB>新文本
foo	bar
baz	qux
TODO	DONE
```

执行批量替换：

```bash
cct trt -d ./project --rules ./rules.tsv
```

多组规则采用"单遍同时匹配"：每个位置至多被一条规则命中，重叠时按规则在文件中的先后顺序优先，
不会发生"前一条规则的结果被后一条再次替换"的级联。

## 5. 正则替换与捕获组

```bash
# 将 2026-06 形式的年月改为 06/2026
cct trt -d ./logs -r 1 -o '(\d{4})-(\d{2})' -n '${2}/${1}'
```

`-r 1` 时 `-n` 支持 `$1`、`${name}` 捕获组引用；未参与匹配的组按空字符串处理。

## 6. 大小写不敏感

```bash
cct trt -d ./docs -o hello -n hi -c 0
```

`-c`、`-r` 等开关对本次运行的所有规则统一生效。

## 7. 关闭备份

```bash
cct trt -d ./project -o foo -n bar -b 0
```

注意：关闭备份后将无法通过 `-u` 撤销本次替换。

## 8. 撤销上一次替换

```bash
cct trt -d ./project -u 1
```

按目标目录定位：仅还原与 `-d ./project` 匹配的最近一次备份，还原成功后自动删除该备份归档。
即使多个目录共用同级 `backup/` 目录也互不干扰。

## 9. 手动还原备份

备份位于目标目录同级的 `backup/` 下，文件名形如 `backup_yyyyMMddHHmmss.zip`，
内部保留原始目录层级。如需手动还原，直接解压并覆盖到目标目录即可：

```bash
# 示例（Linux）
unzip ../backup/backup_20260601_181530.zip -d ./project
```

## 10. 验证（开发者）

```bash
# 运行全部测试
cargo test

# 代码质量门禁（提交前）
cargo fmt
cargo clippy
```

---

## 验收对照速查

| 想验证 | 命令 | 期望 |
|--------|------|------|
| 基本替换（US1） | `cct trt -d <dir> -o foo -n bar` | 文本文件中 foo→bar，二进制不变，报告修改数 |
| 批量规则 | `cct trt -d <dir> --rules rules.tsv` | 多组规则单遍同时生效 |
| 备份生成（US2） | 默认 `-b 1` 执行替换 | 同级 backup/ 下生成带时间戳 ZIP |
| 一键撤销（US2） | `cct trt -d <dir> -u 1` | 目录还原且备份被删除 |
| 主命令探索（US3） | `cct` / `cct -ls` / `cct -h` | 列出子命令 / 帮助 |
