//! trt 子命令的命令行参数定义与归一化。
//!
//! 使用 clap derive 定义所有参数（参见 contracts/cli-trt.md），
//! 并将原始参数校验、归一化为内部使用的 [`TrtOptions`]（参见 data-model.md 实体 4/5）。

use std::path::PathBuf;

use clap::Args;

use crate::error::{CctError, CctResult};

/// trt 子命令的原始命令行参数。
///
/// 开关类参数统一采用 `1/0` 取值约定（见 spec 假设），故以 `u8` 接收后再校验。
#[derive(Debug, Args)]
pub struct TrtArgs {
    /// 要处理的目录（可以是相对路径或绝对路径）。
    #[arg(short = 'd', long = "directory")]
    pub directory: PathBuf,

    /// 要替换的文本（使用 -u 撤销时可选）。
    #[arg(short = 'o', long = "old-text")]
    pub old_text: Option<String>,

    /// 替换后的文本（使用 -u 撤销时可选）。
    #[arg(short = 'n', long = "new-text")]
    pub new_text: Option<String>,

    /// 多组替换规则文件（每行一组，旧/新以制表符分隔，# 注释行与空行忽略）。
    #[arg(long = "rules")]
    pub rules: Option<PathBuf>,

    /// 启用备份（1=启用, 0=禁用）。
    #[arg(short = 'b', long = "backup", default_value_t = 1)]
    pub backup: u8,

    /// 撤销模式：还原上一次操作并删除备份（1=撤销, 0=正常）。
    #[arg(short = 'u', long = "undo", default_value_t = 0)]
    pub undo: u8,

    /// 大小写敏感（1=敏感, 0=不敏感）。
    #[arg(short = 'c', long = "case-sensitive", default_value_t = 1)]
    pub case_sensitive: u8,

    /// 使用正则表达式（1=正则, 0=字面）。
    #[arg(short = 'r', long = "regex", default_value_t = 0)]
    pub regex: u8,

    /// 显示进度（1=显示, 0=隐藏）。
    #[arg(long = "progress", default_value_t = 1)]
    pub progress: u8,
}

/// trt 的运行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrtMode {
    /// 正常替换模式（-u 0，默认）。
    Replace,
    /// 撤销模式（-u 1）。
    Undo,
}

/// 归一化后的 trt 运行选项（参见 data-model.md 实体 4）。
#[derive(Debug)]
pub struct TrtOptions {
    /// 目标目录。
    pub directory: PathBuf,
    /// 运行模式。
    pub mode: TrtMode,
    /// 命令行单组规则（旧文本）；撤销模式下可为 None。
    pub old_text: Option<String>,
    /// 命令行单组规则（新文本）。
    pub new_text: Option<String>,
    /// 规则文件路径。
    pub rules_file: Option<PathBuf>,
    /// 是否启用备份。
    pub backup_enabled: bool,
    /// 是否大小写敏感。
    pub case_sensitive: bool,
    /// 是否使用正则。
    pub use_regex: bool,
    /// 是否显示进度。
    pub show_progress: bool,
}

impl TrtArgs {
    /// 将原始参数校验并归一化为 [`TrtOptions`]。
    ///
    /// 校验规则（见 FR-006/FR-007/FR-022 与 spec 边界）：
    /// - 目标目录必须存在且为目录；
    /// - 非撤销模式下，`-o/-n` 与 `--rules` 必须至少提供其一；
    /// - 提供了 `-o` 时其值不能为空字符串。
    pub fn into_options(self) -> CctResult<TrtOptions> {
        let mode = if self.undo == 1 {
            TrtMode::Undo
        } else {
            TrtMode::Replace
        };

        // 目标目录存在性校验。
        if !self.directory.is_dir() {
            return Err(CctError::DirectoryNotAccessible(self.directory));
        }

        // 旧文本非空校验（仅当提供了 -o 时）。
        if let Some(o) = &self.old_text
            && o.is_empty()
        {
            return Err(CctError::EmptyOldText);
        }

        // 非撤销模式下必须有规则来源。
        if mode == TrtMode::Replace && self.old_text.is_none() && self.rules.is_none() {
            return Err(CctError::NoRulesProvided);
        }

        Ok(TrtOptions {
            directory: self.directory,
            mode,
            old_text: self.old_text,
            new_text: self.new_text,
            rules_file: self.rules,
            backup_enabled: self.backup == 1,
            case_sensitive: self.case_sensitive == 1,
            use_regex: self.regex == 1,
            show_progress: self.progress == 1,
        })
    }
}
