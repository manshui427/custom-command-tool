//! 备份归档：替换前对被修改文件做结构化 ZIP 备份。
//!
//! 对应 spec 的 FR-011/FR-012/FR-013/FR-013a（参见 research.md §8，data-model.md 实体 8/9）：
//! - 仅对实际被修改的文件备份，全部汇集为单个 ZIP；
//! - ZIP 存放于"目标目录同级"的 `backup/` 目录，文件名 `backup_yyyyMMddHHmmss.zip`；
//! - ZIP 内部保留相对目标目录的原始层级，可解压直接覆盖还原；
//! - ZIP 内写入清单 `.cct-manifest.json`，记录来源目录元数据，供撤销按目录定位。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use zip::write::{SimpleFileOptions, ZipWriter};

use crate::error::{CctError, CctResult};

/// 备份归档内的清单条目文件名。
pub const MANIFEST_NAME: &str = ".cct-manifest.json";

/// 备份清单：记录来源目录等元数据（参见 data-model.md 实体 9）。
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupManifest {
    /// 来源目标目录的规范化绝对路径（撤销时按此匹配 -d）。
    pub source_directory: String,
    /// 创建时间戳（yyyyMMddHHmmss，与文件名一致）。
    pub created_at: String,
    /// 生成工具版本。
    pub tool_version: String,
    /// 归档内被备份文件数。
    pub file_count: u64,
}

/// 一次替换的备份会话：累积被修改文件到一个 ZIP，最后写清单并关闭。
pub struct BackupSession {
    /// 备份 ZIP 的最终路径。
    zip_path: PathBuf,
    /// 来源目录的规范化路径。
    source_dir: String,
    /// 创建时间戳。
    created_at: String,
    /// 线程安全的 ZIP 写入器；为 None 表示尚无文件写入或已关闭。
    writer: Mutex<Option<ZipWriter<fs::File>>>,
}

/// 在 `backup_dir` 下为给定时间戳生成唯一的备份 ZIP 路径。
///
/// 首选 `backup_{timestamp}.zip`（FR-012 规定的格式）；若该文件已存在（同一秒内多次备份），
/// 则依次尝试 `backup_{timestamp}_001.zip`、`_002.zip`……直至找到不存在的名字，避免互相覆盖。
fn unique_backup_path(backup_dir: &Path, timestamp: &str) -> PathBuf {
    let base = backup_dir.join(format!("backup_{timestamp}.zip"));
    if !base.exists() {
        return base;
    }
    for seq in 1..=u32::MAX {
        let candidate = backup_dir.join(format!("backup_{timestamp}_{seq:03}.zip"));
        if !candidate.exists() {
            return candidate;
        }
    }
    // 理论上不可达；兜底返回基础名。
    base
}

impl BackupSession {
    /// 为目标目录创建备份会话。备份目录位于目标目录的**同级**。
    ///
    /// 此时即创建 ZIP 文件句柄，后续被修改文件在覆盖前写入。
    pub fn new(target_dir: &Path) -> CctResult<Self> {
        let canonical = fs::canonicalize(target_dir).unwrap_or_else(|_| target_dir.to_path_buf());

        // 备份目录 = 目标目录父目录下的 backup/。
        let parent = canonical.parent().unwrap_or(&canonical);
        let backup_dir = parent.join("backup");
        fs::create_dir_all(&backup_dir).map_err(CctError::Io)?;

        let created_at = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();

        // 生成唯一的备份文件路径：保留 spec 规定的 backup_yyyyMMddHHmmss.zip 主格式（FR-012），
        // 但当同一秒内对不同目录/多次备份产生同名文件时，追加序号后缀避免互相覆盖
        // （满足 spec 边界"同一目录连续多次替换产生多个备份"）。
        let zip_path = unique_backup_path(&backup_dir, &created_at);

        let file = fs::File::create(&zip_path).map_err(CctError::Io)?;
        let writer = ZipWriter::new(file);

        Ok(BackupSession {
            zip_path,
            source_dir: canonical.to_string_lossy().to_string(),
            created_at,
            writer: Mutex::new(Some(writer)),
        })
    }

    /// 将 `file` 的原始内容加入备份归档。
    ///
    /// `root` 为目标目录，用于计算 ZIP 内部的相对路径（保留原始层级，FR-013）。
    /// 本方法在文件被覆盖**之前**调用（先备份后改）。线程安全。
    pub fn add_file(&self, root: &Path, file: &Path) -> std::io::Result<()> {
        // 计算相对路径；失败则退化为文件名。
        let rel = file.strip_prefix(root).unwrap_or(file);
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        let content = fs::read(file)?;

        let mut guard = self
            .writer
            .lock()
            .map_err(|_| std::io::Error::other("备份写入器锁中毒"))?;
        if let Some(writer) = guard.as_mut() {
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            writer.start_file(rel_str, options)?;
            writer.write_all(&content)?;
        }
        Ok(())
    }

    /// 收尾：写入清单并关闭 ZIP。
    ///
    /// `file_count` 为实际被修改（已备份）的文件数。
    /// 若 `file_count` 为 0（无文件被修改），则删除空备份 ZIP 并返回 None。
    pub fn finalize(self, file_count: u64) -> CctResult<Option<PathBuf>> {
        let mut guard = self
            .writer
            .lock()
            .map_err(|_| CctError::Backup("备份写入器锁中毒".into()))?;

        let mut writer = match guard.take() {
            Some(w) => w,
            None => return Ok(None),
        };

        // 无文件被备份：关闭并删除空 ZIP。
        if file_count == 0 {
            drop(writer);
            let _ = fs::remove_file(&self.zip_path);
            return Ok(None);
        }

        // 写入清单。
        let manifest = BackupManifest {
            source_directory: self.source_dir.clone(),
            created_at: self.created_at.clone(),
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            file_count,
        };
        let json =
            serde_json::to_vec_pretty(&manifest).map_err(|e| CctError::Backup(e.to_string()))?;
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        writer
            .start_file(MANIFEST_NAME, options)
            .map_err(|e| CctError::Backup(e.to_string()))?;
        writer
            .write_all(&json)
            .map_err(|e| CctError::Backup(e.to_string()))?;

        writer
            .finish()
            .map_err(|e| CctError::Backup(e.to_string()))?;
        Ok(Some(self.zip_path))
    }
}
