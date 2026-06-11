//! 全局错误类型与退出码约定。
//!
//! 设计原则（参见宪法原则 II）：库内模块使用 `thiserror` 定义结构化错误，
//! 程序入口（`main.rs`）边界使用 `anyhow` 聚合并映射为退出码与 stderr 友好信息。

use std::path::PathBuf;

use thiserror::Error;

/// trt 子命令及框架在执行过程中可能产生的结构化错误。
///
/// 每个变体对应规格中的一类失败场景（见 spec.md 的 FR-022 与边界情况）。
#[derive(Debug, Error)]
pub enum CctError {
    /// 目标目录不存在或无法访问。
    #[error("目标目录不存在或无法访问：{0}")]
    DirectoryNotAccessible(PathBuf),

    /// 非撤销模式下既未提供 `-o/-n` 也未提供 `--rules`。
    #[error("未提供任何替换规则：请使用 -o/-n 或 --rules 指定")]
    NoRulesProvided,

    /// 旧文本为空字符串（非法输入）。
    #[error("被替换文本（-o）不能为空")]
    EmptyOldText,

    /// 规则文件无法读取。
    #[error("无法读取规则文件 {path}：{source}")]
    RulesFileUnreadable {
        /// 规则文件路径。
        path: PathBuf,
        /// 底层 IO 错误。
        source: std::io::Error,
    },

    /// 规则文件中某行格式非法（缺少分隔符）。
    #[error("规则文件第 {line} 行格式非法（缺少分隔符 :$#split#$:）：{content}")]
    InvalidRuleLine {
        /// 出错的行号（从 1 开始）。
        line: usize,
        /// 该行原始内容。
        content: String,
    },

    /// 规则文件中没有任何有效规则。
    #[error("规则文件中没有有效规则：{0}")]
    NoValidRules(PathBuf),

    /// 正则表达式编译失败。
    #[error("正则表达式无效：{0}")]
    InvalidRegex(String),

    /// 撤销模式下未找到与目标目录匹配的备份。
    #[error("未找到与目录匹配的备份，无法撤销：{0}")]
    NoBackupFound(PathBuf),

    /// 备份归档读写失败。
    #[error("备份操作失败：{0}")]
    Backup(String),

    /// 图形界面相关错误（无图形环境、窗口初始化失败等）。
    #[cfg(feature = "gui")]
    #[error("{0}")]
    Gui(String),

    /// 通用 IO 错误。
    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),
}

/// `CctError` 的便捷 Result 别名。
pub type CctResult<T> = Result<T, CctError>;
