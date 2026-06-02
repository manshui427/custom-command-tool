//! 并行目录遍历与二进制检测。
//!
//! 对应 spec 的 FR-008/FR-009/FR-018（参见 research.md §3/§4）：
//! - 使用 `ignore::WalkBuilder` 并行遍历目录，不跟随符号链接（避免越界与循环）；
//! - 边遍历边处理（不先收集全部路径），使进度可在数秒内反馈；
//! - 通过扫描文件头部是否含 NUL 字节判定二进制文件并跳过。

use std::path::Path;

use ignore::{WalkBuilder, WalkState};

/// 读取文件头部用于二进制检测的字节数。
const BINARY_SNIFF_LEN: usize = 8192;

/// 通过检测头部 NUL 字节判断文件是否为二进制。
///
/// 含 NUL 字节即判定为二进制（git、ripgrep 等采用的通用启发式）。
/// 读取失败时保守地视为二进制（跳过），避免破坏无法读取的文件。
pub fn is_binary(path: &Path) -> bool {
    use std::io::Read;

    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return true,
    };
    let mut buf = [0u8; BINARY_SNIFF_LEN];
    match file.read(&mut buf) {
        Ok(n) => memchr::memchr(0, &buf[..n]).is_some(),
        Err(_) => true,
    }
}

/// 并行遍历 `root` 下的所有常规文件，对每个文件调用 `handler`。
///
/// `handler` 接收文件路径，在多个工作线程中并发执行，因此必须是 `Sync`。
/// 不跟随符号链接（`follow_links(false)`）。隐藏文件与 .gitignore 规则均不启用，
/// 即遍历目录内的全部文件（替换工具应处理所有文本文件，而非遵循版本控制忽略规则）。
pub fn walk_files<F>(root: &Path, handler: F)
where
    F: Fn(&Path) + Sync,
{
    let walker = WalkBuilder::new(root)
        .follow_links(false)
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .build_parallel();

    walker.run(|| {
        let handler = &handler;
        Box::new(move |entry| {
            if let Ok(entry) = entry {
                // 仅处理常规文件。
                if entry.file_type().is_some_and(|ft| ft.is_file()) {
                    handler(entry.path());
                }
            }
            WalkState::Continue
        })
    });
}
