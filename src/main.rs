//! custom-command-tool (`cct`) 程序入口。
//!
//! 负责解析顶层参数并分发：
//! - 带 `--ls` 或不带任何子命令时，输出子命令清单（FR-002/FR-004）；
//! - `-h/--help` 由 clap 自动处理（FR-003）；
//! - 匹配到子命令时分发到对应实现，并将错误映射为非零退出码（FR-022）。

// 启用 gui 时以 Windows 子系统运行，双击 exe 不弹出控制台黑窗。
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
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run_gui));
            return match result {
                Ok(Ok(())) => ExitCode::SUCCESS,
                Ok(Err(e)) => {
                    report_error(&e);
                    ExitCode::FAILURE
                }
                Err(panic) => {
                    let msg = if let Some(s) = panic.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = panic.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "未知错误".to_string()
                    };
                    report_error(&format!("程序崩溃：{msg}"));
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

/// 启动图形界面（仅在启用 `gui` feature 时可用）。
#[cfg(feature = "gui")]
fn run_gui() -> error::CctResult<()> {
    commands::gui::run()
}

/// Windows GUI 子系统下 stderr 不可见，通过 MessageBox 显示错误。
#[cfg(all(feature = "gui", target_os = "windows"))]
fn show_error(msg: &str) {
    unsafe extern "system" {
        fn MessageBoxW(
            hwnd: *const std::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            r#type: u32,
        ) -> i32;
    }
    const MB_OK: u32 = 0x00000000;
    const MB_ICONERROR: u32 = 0x00000010;
    let text: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
    let caption: Vec<u16> = "错误".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        MessageBoxW(
            std::ptr::null(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(all(feature = "gui", target_os = "windows"))]
fn report_error(e: &dyn std::fmt::Display) {
    show_error(&format!("错误：{e}"));
}

#[cfg(not(all(feature = "gui", target_os = "windows")))]
fn report_error(e: &dyn std::fmt::Display) {
    eprintln!("错误：{e}");
}
