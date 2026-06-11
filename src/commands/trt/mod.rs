//! trt（text-replace-tool）子命令：高性能文本替换工具。
//!
//! 本模块编排整个替换/撤销流程，子职责拆分见各子模块：
//! - [`args`]：参数定义与校验；
//! - [`rules`]：规则集与规则文件解析；
//! - [`matcher`]：单遍同时匹配引擎；
//! - [`scanner`]：并行目录遍历与二进制检测；
//! - [`replacer`]：单文件替换与原子写回；
//! - [`backup`]：备份归档；
//! - [`undo`]：撤销。
//!
//! 执行与"进度上报"解耦：[`execute`] 接受一个进度回调，使命令行（indicatif）与图形界面
//! （通道）能复用同一替换逻辑并各自呈现进度（参见 002 plan/research §4）。

pub mod args;
pub mod backup;
pub mod matcher;
pub mod replacer;
pub mod rules;
pub mod scanner;
pub mod undo;

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::CctResult;

use args::{TrtArgs, TrtMode, TrtOptions};
use matcher::Matcher;
use replacer::FileStatus;
use rules::RuleSet;

/// 被影响文件：一次替换中实际发生改动的文件（参见 data-model.md 实体 1）。
///
/// 其字段主要供图形界面展示被影响文件列表（FR-010）；CLI-only 构建下不读取这些字段。
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
#[derive(Debug, Clone)]
pub struct AffectedFile {
    /// 相对目标目录的文件路径。
    pub path: PathBuf,
    /// 该文件内发生的替换次数（>0）。
    pub replacements: u64,
}

/// 替换执行过程中的进度快照（参见 data-model.md 实体 3）。
#[derive(Debug, Clone, Copy, Default)]
pub struct ProgressUpdate {
    /// 已扫描文件数。
    pub scanned: u64,
    /// 已被影响（已改动）文件数。
    pub modified: u64,
}

/// 一次运行的结果摘要 / 影响报告（参见 data-model.md 实体 2）。
///
/// 以"被影响文件"为核心：`affected_files` 与 `files_modified`/`total_replacements` 为主体，
/// 扫描/跳过/失败为次要信息（需求 1）。
#[derive(Debug, Default)]
pub struct RunSummary {
    /// 被影响文件列表（核心）。供图形界面展示文件列表；CLI-only 构建下不读取。
    #[cfg_attr(not(feature = "gui"), allow(dead_code))]
    pub affected_files: Vec<AffectedFile>,
    /// 已扫描文件总数（次要）。
    pub files_scanned: u64,
    /// 被影响文件数（核心，= affected_files.len()）。
    pub files_modified: u64,
    /// 跳过的二进制文件数（次要）。
    pub files_skipped_binary: u64,
    /// 处理失败文件数（次要）。
    pub files_failed: u64,
    /// 总替换次数（核心）。
    pub total_replacements: u64,
    /// 生成的备份归档路径（未备份则 None）。
    pub backup_path: Option<PathBuf>,
}

/// trt 子命令的命令行入口。解析参数后分发到替换或撤销流程，并以命令行方式呈现进度与结果。
pub fn run(args: TrtArgs) -> CctResult<()> {
    let options = args.into_options()?;
    match options.mode {
        TrtMode::Replace => {
            // 命令行进度回调：更新 indicatif spinner 文案。
            let progress = crate::util::progress::Progress::new(options.show_progress);
            let summary = execute_replace(&options, |u: ProgressUpdate| {
                progress.set_message(format!("已扫描 {}，已修改 {}", u.scanned, u.modified));
            })?;
            progress.finish();
            print_summary(&summary);
        }
        TrtMode::Undo => {
            let restored = undo::run_undo(&options.directory)?;
            println!("已撤销：还原 {restored} 个文件，已删除对应备份。");
        }
    }
    Ok(())
}

/// 供图形界面等其他前端复用的执行入口。
///
/// 根据 `options.mode` 执行替换或撤销，通过 `progress` 回调上报进度，返回结构化结果（影响报告）。
/// 替换/备份/撤销的实际效果与命令行逐字节一致。撤销模式不产生进度，返回的 `RunSummary`
/// 以 `files_modified` 记还原文件数、`affected_files` 为空。
///
/// 当前仅图形界面调用；CLI-only 构建下不编译 GUI，故标记允许未使用。
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
pub fn execute<F>(options: &TrtOptions, progress: F) -> CctResult<RunSummary>
where
    F: Fn(ProgressUpdate) + Sync,
{
    match options.mode {
        TrtMode::Replace => execute_replace(options, progress),
        TrtMode::Undo => {
            let restored = undo::run_undo(&options.directory)?;
            Ok(RunSummary {
                files_modified: restored,
                ..Default::default()
            })
        }
    }
}

/// 执行替换流程：构建规则集与匹配器，并行扫描+替换，收集被影响文件并汇总结果。
///
/// `progress` 在处理过程中被多次调用以上报 [`ProgressUpdate`]。
fn execute_replace<F>(options: &TrtOptions, progress: F) -> CctResult<RunSummary>
where
    F: Fn(ProgressUpdate) + Sync,
{
    // 1. 构建规则集与匹配器。
    let rule_set = RuleSet::build(
        options.old_text.clone(),
        options.new_text.clone(),
        options.rules_file.as_deref(),
        options.case_sensitive,
        options.use_regex,
    )?;
    let matcher = Matcher::compile(&rule_set)?;

    // 2. 准备备份（若启用）。
    let backup_session = if options.backup_enabled {
        Some(backup::BackupSession::new(&options.directory)?)
    } else {
        None
    };

    // 3. 并行扫描 + 替换，原子聚合统计；收集被影响文件。
    let scanned = AtomicU64::new(0);
    let modified = AtomicU64::new(0);
    let skipped = AtomicU64::new(0);
    let failed = AtomicU64::new(0);
    let replacements = AtomicU64::new(0);
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let affected: Mutex<Vec<AffectedFile>> = Mutex::new(Vec::new());

    // 备份回调：将原文件归档（线程安全）。
    let backup_cb = backup_session.as_ref().map(|s| {
        let root = options.directory.clone();
        move |p: &std::path::Path| s.add_file(&root, p)
    });

    scanner::walk_files(&options.directory, |path| {
        scanned.fetch_add(1, Ordering::Relaxed);

        let outcome = replacer::process_file(path, &matcher, backup_cb.as_ref());
        match outcome.status {
            FileStatus::Modified => {
                modified.fetch_add(1, Ordering::Relaxed);
                replacements.fetch_add(outcome.replacements, Ordering::Relaxed);
                // 收集被影响文件（路径相对目标目录，便于展示与备份层级一致）。
                let rel = outcome
                    .path
                    .strip_prefix(&options.directory)
                    .unwrap_or(&outcome.path)
                    .to_path_buf();
                if let Ok(mut a) = affected.lock() {
                    a.push(AffectedFile {
                        path: rel,
                        replacements: outcome.replacements,
                    });
                }
            }
            FileStatus::Unchanged => {}
            FileStatus::SkippedBinary => {
                skipped.fetch_add(1, Ordering::Relaxed);
            }
            FileStatus::Failed(reason) => {
                failed.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut f) = failures.lock() {
                    f.push(format!("{}: {}", outcome.path.display(), reason));
                }
            }
        }

        // 上报进度。
        progress(ProgressUpdate {
            scanned: scanned.load(Ordering::Relaxed),
            modified: modified.load(Ordering::Relaxed),
        });
    });

    // 4. 收尾备份归档（写入 manifest 并关闭 ZIP）。
    let backup_path = match backup_session {
        Some(session) => session.finalize(modified.load(Ordering::Relaxed))?,
        None => None,
    };

    // 打印失败明细到 stderr（仅命令行可见；GUI 通过 files_failed 计数感知）。
    if let Ok(f) = failures.lock() {
        for line in f.iter() {
            eprintln!("失败：{line}");
        }
    }

    let affected_files = affected.into_inner().unwrap_or_default();
    Ok(RunSummary {
        files_scanned: scanned.load(Ordering::Relaxed),
        files_modified: affected_files.len() as u64,
        files_skipped_binary: skipped.load(Ordering::Relaxed),
        files_failed: failed.load(Ordering::Relaxed),
        total_replacements: replacements.load(Ordering::Relaxed),
        backup_path,
        affected_files,
    })
}

/// 将运行摘要打印到 stdout，以被影响文件为核心（需求 1，FR-001/FR-003/FR-004）。
fn print_summary(summary: &RunSummary) {
    // 首要信息：被影响文件数 + 总替换次数。
    println!(
        "被影响 {} 个文件，共替换 {} 处。",
        summary.files_modified, summary.total_replacements
    );
    // 次要信息：扫描/跳过/失败。
    println!(
        "（扫描 {}，跳过二进制 {}，失败 {}）",
        summary.files_scanned, summary.files_skipped_binary, summary.files_failed
    );
    if let Some(p) = &summary.backup_path {
        println!("备份已生成：{}", p.display());
    }
}
