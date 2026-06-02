//! 单文件替换处理。
//!
//! 对应 spec 的 FR-008/FR-009/FR-019/FR-019a（参见 data-model.md 实体 6，research.md §6/§7）：
//! - 二进制文件直接跳过；
//! - 小于阈值的文件整体载入内存替换；
//! - 超过阈值的大文件在字面模式下走流式替换（内存恒定）；正则大文件回退为整体读入；
//! - 写回统一采用"临时文件 + 原子改名"，且在覆盖前调用备份回调（先备份后改）。

use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use crate::util::fs_atomic;

use super::matcher::Matcher;
use super::scanner;

/// 超过该字节数的文件走大文件路径（流式或回退整体）。
const LARGE_FILE_THRESHOLD: u64 = 8 * 1024 * 1024;

/// 单个文件的处理状态（参见 data-model.md 实体 6）。
#[derive(Debug)]
pub enum FileStatus {
    /// 文本文件且发生了替换。
    Modified,
    /// 文本文件但无匹配。
    Unchanged,
    /// 检测为二进制，已跳过。
    SkippedBinary,
    /// 处理失败（含原因），不影响其他文件继续。
    Failed(String),
}

/// 单个文件的处理结果。
#[derive(Debug)]
pub struct FileOutcome {
    /// 文件路径。
    pub path: PathBuf,
    /// 处理状态。
    pub status: FileStatus,
    /// 该文件内发生的替换次数（仅 Modified 时有意义）。
    pub replacements: u64,
}

/// 处理单个文件。
///
/// `backup` 为可选回调：在文件被覆盖**之前**调用，用于把原始文件归档（先备份后改）。
/// US1 阶段传 `None`；US2 接入备份后传 `Some(...)`。回调返回 `Err` 时中止该文件的替换并标记失败，
/// 以保证"无法备份则不修改"。
pub fn process_file<B>(path: &Path, matcher: &Matcher, backup: Option<&B>) -> FileOutcome
where
    B: Fn(&Path) -> std::io::Result<()> + Sync,
{
    // 二进制文件跳过（FR-009）。
    if scanner::is_binary(path) {
        return FileOutcome {
            path: path.to_path_buf(),
            status: FileStatus::SkippedBinary,
            replacements: 0,
        };
    }

    let size = match fs::metadata(path) {
        Ok(m) => m.len(),
        Err(e) => return failed(path, e.to_string()),
    };

    // 大文件 + 字面模式：流式替换路径。
    if size > LARGE_FILE_THRESHOLD && matcher.is_literal() {
        process_large_literal(path, matcher, backup)
    } else {
        process_whole(path, matcher, backup)
    }
}

/// 小文件（或正则大文件）整体读入替换。
fn process_whole<B>(path: &Path, matcher: &Matcher, backup: Option<&B>) -> FileOutcome
where
    B: Fn(&Path) -> std::io::Result<()> + Sync,
{
    // 以字节读入后尝试按 UTF-8 解码；无法解码者按二进制跳过（spec 假设）。
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => return failed(path, e.to_string()),
    };
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => {
            return FileOutcome {
                path: path.to_path_buf(),
                status: FileStatus::SkippedBinary,
                replacements: 0,
            };
        }
    };

    let (new_text, count) = matcher.replace(&text);
    if count == 0 {
        return FileOutcome {
            path: path.to_path_buf(),
            status: FileStatus::Unchanged,
            replacements: 0,
        };
    }

    // 先备份后改：覆盖前归档原始内容。
    if let Some(b) = backup
        && let Err(e) = b(path)
    {
        return failed(path, format!("备份失败：{e}"));
    }

    match fs_atomic::write_atomic(path, new_text.as_bytes()) {
        Ok(()) => FileOutcome {
            path: path.to_path_buf(),
            status: FileStatus::Modified,
            replacements: count,
        },
        Err(e) => failed(path, e.to_string()),
    }
}

/// 大文件 + 字面模式：流式替换，内存占用恒定。
///
/// 直接将替换结果流式写入同目录临时文件（不在内存中缓存整个文件），
/// 处理完成后：若发生替换则备份原文件并原子改名覆盖；若无替换则丢弃临时文件、保持原文件不动。
fn process_large_literal<B>(path: &Path, matcher: &Matcher, backup: Option<&B>) -> FileOutcome
where
    B: Fn(&Path) -> std::io::Result<()> + Sync,
{
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) => return failed(path, e.to_string()),
    };
    let reader = BufReader::new(file);

    // 同目录临时文件，保证后续 rename 原子（FR-019a）。
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty());
    let tmp = match dir {
        Some(d) => NamedTempFile::new_in(d),
        None => NamedTempFile::new(),
    };
    let mut tmp = match tmp {
        Ok(t) => t,
        Err(e) => return failed(path, e.to_string()),
    };

    let count = {
        let writer = BufWriter::new(tmp.as_file_mut());
        match matcher.stream_replace_literal(reader, writer) {
            Ok(c) => c,
            Err(e) => return failed(path, e.to_string()),
        }
    };

    if count == 0 {
        // 无替换：丢弃临时文件，原文件保持不动。
        return FileOutcome {
            path: path.to_path_buf(),
            status: FileStatus::Unchanged,
            replacements: 0,
        };
    }

    // 先备份后改。
    if let Some(b) = backup
        && let Err(e) = b(path)
    {
        return failed(path, format!("备份失败：{e}"));
    }

    // 保留原文件权限位后原子改名。
    if let Ok(meta) = fs::metadata(path) {
        let _ = tmp.as_file().set_permissions(meta.permissions());
    }
    match tmp.persist(path) {
        Ok(_) => FileOutcome {
            path: path.to_path_buf(),
            status: FileStatus::Modified,
            replacements: count,
        },
        Err(e) => failed(path, e.error.to_string()),
    }
}

/// 构造失败结果的便捷函数。
fn failed(path: &Path, reason: String) -> FileOutcome {
    FileOutcome {
        path: path.to_path_buf(),
        status: FileStatus::Failed(reason),
        replacements: 0,
    }
}
