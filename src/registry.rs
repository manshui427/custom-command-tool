//! 子命令注册表。
//!
//! 集中登记所有子命令的元信息（名称、别名、描述），供主命令 `-ls` 列举使用。
//! 新增子命令时只需在 [`SUBCOMMANDS`] 中追加一条登记，使 `-ls` 输出与子命令实现解耦
//! （参见 spec.md 的 FR-001/FR-002/FR-005，data-model.md 实体 1）。

/// 单个子命令的元信息。
#[derive(Debug, Clone, Copy)]
pub struct SubcommandInfo {
    /// 子命令全名（如 `text-replace-tool`）。
    pub name: &'static str,
    /// 简写别名（如 `trt`）。
    pub alias: &'static str,
    /// 中文简要描述（如 `文本替换工具`）。
    pub description: &'static str,
}

/// 全部已注册子命令。名称与别名在表内唯一。
pub const SUBCOMMANDS: &[SubcommandInfo] = &[
    SubcommandInfo {
        name: "text-replace-tool",
        alias: "trt",
        description: "文本替换",
    },
];

/// 将子命令清单格式化为可读文本（供 `-ls` 与无子命令时输出）。
///
/// 输出形如：
/// ```text
/// 可用子命令：
///   trt (text-replace-tool)  文本替换工具
/// ```
pub fn format_subcommand_list() -> String {
    let mut out = String::from("可用子命令：\n");
    for sc in SUBCOMMANDS {
        if sc.alias == sc.name {
            // 别名与全名相同时只显示一个，避免冗余。
            out.push_str(&format!("  {}  {}\n", sc.alias, sc.description));
        } else {
            out.push_str(&format!(
                "  {} ({})  {}\n",
                sc.alias, sc.name, sc.description
            ));
        }
    }
    out
}
