//! 顶层命令行接口定义。
//!
//! 定义主程序 `cct` 的顶层参数与子命令枚举（参见 contracts/cli-main.md）：
//! - `-ls` 列出所有子命令；
//! - `-h/--help` 由 clap 自动提供；
//! - 不带任何子命令时，行为等同 `-ls`（在 main.rs 中处理）。

use clap::{Parser, Subcommand};

use crate::commands::trt::args::TrtArgs;

/// 顶层 CLI 定义。
#[derive(Debug, Parser)]
#[command(
    name = "cct",
    about = "自定义工具：通过子命令快速完成各种自定义命令",
    version,
    // 不带子命令时不自动报错，由 main.rs 输出子命令清单（等同 -ls）。
    arg_required_else_help = false,
    subcommand_required = false
)]
pub struct Cli {
    /// 列出所有已注册子命令及其简要描述。
    #[arg(long = "ls", short = 'l')]
    pub list: bool,

    /// 要执行的子命令。
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// 所有可用子命令。
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// 文本替换工具（别名 trt）。
    #[command(name = "text-replace-tool", alias = "trt")]
    Trt(TrtArgs),
}
