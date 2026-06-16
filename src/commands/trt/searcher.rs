//! 单文件查找处理。
//!
//! 查找模式下对每个文件检测是否包含搜索模式：
//! - 二进制文件直接跳过；
//! - 读取文件内容，通过 Matcher 统计匹配次数；
//! - 不修改任何文件。

use std::path::{Path, PathBuf};

use super::matcher::Matcher;
use super::scanner;

/// 单个文件的查找结果状态。
#[derive(Debug)]
pub enum SearchFileStatus {
    /// 文件包含匹配。
    Hit,
    /// 文件不包含匹配。
    NoHit,
    /// 检测为二进制，已跳过。
    SkippedBinary,
    /// 处理失败（含原因）。
    Failed(String),
}

/// 单个文件的查找结果。
#[derive(Debug)]
pub struct SearchFileOutcome {
    /// 文件路径。
    pub path: PathBuf,
    /// 处理状态。
    pub status: SearchFileStatus,
    /// 该文件内的匹配次数（仅 Hit 时有意义）。
    pub match_count: u64,
}

/// 对单个文件执行查找，返回匹配结果。
///
/// 不修改任何文件，仅检测并统计匹配次数。
pub fn search_file(path: &Path, matcher: &Matcher) -> SearchFileOutcome {
    if scanner::is_binary(path) {
        return SearchFileOutcome {
            path: path.to_path_buf(),
            status: SearchFileStatus::SkippedBinary,
            match_count: 0,
        };
    }

    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return SearchFileOutcome {
            path: path.to_path_buf(),
            status: SearchFileStatus::Failed(e.to_string()),
            match_count: 0,
        },
    };

    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => {
            return SearchFileOutcome {
                path: path.to_path_buf(),
                status: SearchFileStatus::SkippedBinary,
                match_count: 0,
            };
        }
    };

    let count = matcher.count_matches(&text);
    SearchFileOutcome {
        path: path.to_path_buf(),
        status: if count > 0 {
            SearchFileStatus::Hit
        } else {
            SearchFileStatus::NoHit
        },
        match_count: count,
    }
}