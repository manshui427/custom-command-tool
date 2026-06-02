//! 撤销：还原上一次替换并删除对应备份。
//!
//! 对应 spec 的 FR-014（参见 research.md §9）：
//! - 扫描目标目录同级 `backup/` 下所有 ZIP，读取各自清单的来源目录元数据；
//! - 筛选来源目录 == 当前 `-d`（规范化路径比较）的归档，取时间戳最近的一个；
//! - 解压覆盖还原到目标目录后，删除该备份归档；
//! - 多个目标目录共享同一 `backup/` 时按来源归属区分，互不干扰。

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::error::{CctError, CctResult};
use crate::util::fs_atomic;

use super::backup::{BackupManifest, MANIFEST_NAME};

/// 执行撤销，返回还原的文件数。
pub fn run_undo(target_dir: &Path) -> CctResult<u64> {
    let canonical = fs::canonicalize(target_dir).unwrap_or_else(|_| target_dir.to_path_buf());
    let parent = canonical.parent().unwrap_or(&canonical);
    let backup_dir = parent.join("backup");

    if !backup_dir.is_dir() {
        return Err(CctError::NoBackupFound(canonical));
    }

    // 找出与目标目录匹配的、时间戳最近的备份 ZIP。
    let archive_path = find_latest_matching_backup(&backup_dir, &canonical)?
        .ok_or_else(|| CctError::NoBackupFound(canonical.clone()))?;

    // 解压覆盖还原。
    let restored = restore_archive(&archive_path, &canonical)?;

    // 还原成功后删除该备份归档。
    fs::remove_file(&archive_path).map_err(CctError::Io)?;

    Ok(restored)
}

/// 在备份目录中找出来源目录匹配且时间戳最近的归档。
fn find_latest_matching_backup(backup_dir: &Path, source: &Path) -> CctResult<Option<PathBuf>> {
    let source_str = source.to_string_lossy();
    let mut best: Option<(String, PathBuf)> = None;

    for entry in fs::read_dir(backup_dir).map_err(CctError::Io)? {
        let entry = entry.map_err(CctError::Io)?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("zip") {
            continue;
        }

        // 读取该 ZIP 内的清单。
        let manifest = match read_manifest(&path) {
            Some(m) => m,
            None => continue,
        };

        // 来源目录匹配（规范化字符串比较）。
        if manifest.source_directory != source_str {
            continue;
        }

        // 按 created_at 取最近（时间戳字符串可直接字典序比较）。
        match &best {
            Some((ts, _)) if *ts >= manifest.created_at => {}
            _ => best = Some((manifest.created_at, path)),
        }
    }

    Ok(best.map(|(_, p)| p))
}

/// 读取 ZIP 内的备份清单；无清单或解析失败返回 None。
fn read_manifest(zip_path: &Path) -> Option<BackupManifest> {
    let file = fs::File::open(zip_path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;
    let mut entry = archive.by_name(MANIFEST_NAME).ok()?;
    let mut buf = String::new();
    entry.read_to_string(&mut buf).ok()?;
    serde_json::from_str(&buf).ok()
}

/// 将归档内容解压覆盖还原到目标目录，返回还原的文件数（不含清单）。
fn restore_archive(zip_path: &Path, target_dir: &Path) -> CctResult<u64> {
    let file = fs::File::open(zip_path).map_err(CctError::Io)?;
    let mut archive = ZipArchive::new(file).map_err(|e| CctError::Backup(e.to_string()))?;

    let mut restored = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| CctError::Backup(e.to_string()))?;
        let name = entry.name().to_string();

        // 跳过清单条目本身。
        if name == MANIFEST_NAME {
            continue;
        }

        let mut content = Vec::new();
        entry.read_to_end(&mut content).map_err(CctError::Io)?;

        // 目标路径 = 目标目录 + 归档内相对路径。
        let dest = target_dir.join(&name);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(CctError::Io)?;
        }
        // 原子写回，保证还原过程崩溃安全。
        fs_atomic::write_atomic(&dest, &content).map_err(CctError::Io)?;
        restored += 1;
    }

    Ok(restored)
}
