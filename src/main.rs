//! custom-command-tool (`cct`) 程序入口。
//!
//! 负责解析顶层参数并分发：
//! - 带 `--ls` 或不带任何子命令时，输出子命令清单（FR-002/FR-004）；
//! - `-h/--help` 由 clap 自动处理（FR-003）；
//! - 匹配到子命令时分发到对应实现，并将错误映射为非零退出码（FR-022）。

mod cli;
mod commands;
mod error;
mod registry;
mod util;

use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, Commands};

fn main() -> ExitCode {
    // 规格要求支持单横线长选项 `-ls`（FR-002）。clap 默认不识别此形式，
    // 故在解析前先做预处理：当用户以 `cct -ls` 调用时直接输出子命令清单。
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "-ls") {
        print!("{}", registry::format_subcommand_list());
        return ExitCode::SUCCESS;
    }

    let cli = Cli::parse();

    // 无子命令或显式 --ls：输出子命令清单（FR-002/FR-004）。
    if cli.command.is_none() || cli.list {
        print!("{}", registry::format_subcommand_list());
        return ExitCode::SUCCESS;
    }

    // 分发到具体子命令。
    let result = match cli.command.unwrap() {
        Commands::Trt(args) => commands::trt::run(args),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // 错误信息输出到 stderr，返回非零退出码（FR-022，宪法原则 I）。
            eprintln!("错误：{e}");
            ExitCode::FAILURE
        }
    }
}
