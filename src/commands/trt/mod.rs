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

pub mod args;
pub mod backup;
pub mod matcher;
pub mod replacer;
pub mod rules;
pub mod scanner;
pub mod undo;

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::CctResult;

use args::{TrtArgs, TrtMode, TrtOptions};
use matcher::Matcher;
use replacer::FileStatus;
use rules::RuleSet;

/// 一次运行的结果摘要（参见 data-model.md 实体 7）。
#[derive(Debug, Default)]
pub struct RunSummary {
    /// 已扫描文件总数。
    pub files_scanned: u64,
    /// 被修改文件数。
    pub files_modified: u64,
    /// 跳过的二进制文件数。
    pub files_skipped_binary: u64,
    /// 处理失败文件数。
    pub files_failed: u64,
    /// 总替换次数。
    pub total_replacements: u64,
    /// 生成的备份归档路径（未备份则 None）。
    pub backup_path: Option<std::path::PathBuf>,
}

/// trt 子命令入口。解析参数后分发到替换或撤销流程。
pub fn run(args: TrtArgs) -> CctResult<()> {
    let options = args.into_options()?;

    match options.mode {
        TrtMode::Replace => {
            let summary = run_replace(&options)?;
            print_summary(&summary);
        }
        TrtMode::Undo => {
            let restored = undo::run_undo(&options.directory)?;
            println!("已撤销：还原 {} 个文件，已删除对应备份。", restored);
        }
    }
    Ok(())
}

/// 执行替换流程：构建规则集与匹配器，并行扫描+替换，汇总结果。
fn run_replace(options: &TrtOptions) -> CctResult<RunSummary> {
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

    // 3. 并行扫描 + 替换，原子聚合统计。
    let scanned = AtomicU64::new(0);
    let modified = AtomicU64::new(0);
    let skipped = AtomicU64::new(0);
    let failed = AtomicU64::new(0);
    let replacements = AtomicU64::new(0);
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());

    let progress = crate::util::progress::Progress::new(options.show_progress);

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

        progress.set_message(format!(
            "已扫描 {}，已修改 {}",
            scanned.load(Ordering::Relaxed),
            modified.load(Ordering::Relaxed)
        ));
    });

    progress.finish();

    // 4. 收尾备份归档（写入 manifest 并关闭 ZIP）。
    let backup_path = match backup_session {
        Some(session) => session.finalize(modified.load(Ordering::Relaxed))?,
        None => None,
    };

    // 打印失败明细到 stderr。
    if let Ok(f) = failures.lock() {
        for line in f.iter() {
            eprintln!("失败：{line}");
        }
    }

    Ok(RunSummary {
        files_scanned: scanned.load(Ordering::Relaxed),
        files_modified: modified.load(Ordering::Relaxed),
        files_skipped_binary: skipped.load(Ordering::Relaxed),
        files_failed: failed.load(Ordering::Relaxed),
        total_replacements: replacements.load(Ordering::Relaxed),
        backup_path,
    })
}

/// 将运行摘要打印到 stdout（FR-021）。
fn print_summary(summary: &RunSummary) {
    println!(
        "已扫描 {} 个文件，修改 {} 个，跳过二进制 {} 个，失败 {} 个，共替换 {} 处。",
        summary.files_scanned,
        summary.files_modified,
        summary.files_skipped_binary,
        summary.files_failed,
        summary.total_replacements
    );
    if let Some(p) = &summary.backup_path {
        println!("备份已生成：{}", p.display());
    }
}
