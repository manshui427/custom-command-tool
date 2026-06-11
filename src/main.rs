//! custom-command-tool (`cct`) 程序入口。
//!
//! 负责解析顶层参数并分发：
//! - 带 `--ls` 或不带任何子命令时，输出子命令清单（FR-002/FR-004）；
//! - `-h/--help` 由 clap 自动处理（FR-003）；
//! - 匹配到子命令时分发到对应实现，并将错误映射为非零退出码（FR-022）。

// 启用 gui 时以 Windows 子系统运行，双击 exe 或 cct gui 不弹出控制台黑窗。
// CLI 命令（如 cct trt）需要终端输出，通过 attach_console() 重新连接父终端。
#![cfg_attr(all(feature = "gui", target_os = "windows"), windows_subsystem = "windows")]

mod cli;
mod commands;
mod error;
mod registry;
mod util;

use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, Commands};

/// Windows GUI 子系统下重新连接父进程的控制台，使 CLI 命令的 stdout/stderr
/// 仍能在终端窗口可见。无 GUI feature 或非 Windows 时为空操作。
#[cfg(all(feature = "gui", target_os = "windows"))]
fn attach_console() {
    unsafe extern "system" {
        fn AttachConsole(pid: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(all(feature = "gui", target_os = "windows")))]
fn attach_console() {}

fn main() -> ExitCode {
    attach_console();
    // 规格要求支持单横线长选项 `-ls`（FR-002）。clap 默认不识别此形式，
    // 故在解析前先做预处理：当用户以 `cct -ls` 调用时直接输出子命令清单。
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "-ls") {
        print!("{}", registry::format_subcommand_list());
        return ExitCode::SUCCESS;
    }

    let cli = Cli::parse();

    // 无子命令或显式 --ls：
    // - 启用 gui 时直接打开图形界面（双击 exe 的自然预期，避免控制台闪退）；
    // - CLI-only 构建下输出子命令清单（FR-002/FR-004）。
    if cli.command.is_none() {
        if cli.list {
            print!("{}", registry::format_subcommand_list());
            return ExitCode::SUCCESS;
        }
        #[cfg(feature = "gui")]
        {
            return match run_gui() {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("错误：{e}");
                    ExitCode::FAILURE
                }
            };
        }
        #[cfg(not(feature = "gui"))]
        {
            print!("{}", registry::format_subcommand_list());
            return ExitCode::SUCCESS;
        }
    }

    if cli.list {
        print!("{}", registry::format_subcommand_list());
        return ExitCode::SUCCESS;
    }

    // 分发到具体子命令。
    let result = match cli.command.unwrap() {
        Commands::Trt(args) => commands::trt::run(args),
        Commands::Gui => run_gui(),
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

/// 启动图形界面子命令。
///
/// 启用 `gui` feature 时委托给 `commands::gui::run()`；未启用时返回明确错误，
/// 以保证 CLI-only 构建下 `cct gui` 给出友好提示而非编译期缺失（FR-007）。
#[cfg(feature = "gui")]
fn run_gui() -> error::CctResult<()> {
    commands::gui::run()
}

/// CLI-only 构建下的 `cct gui`：提示本可执行文件未包含图形界面支持。
#[cfg(not(feature = "gui"))]
fn run_gui() -> error::CctResult<()> {
    Err(error::CctError::Gui(
        "本可执行文件未包含图形界面支持（请使用启用 gui 特性的构建）".to_string(),
    ))
}
