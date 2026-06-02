//! 崩溃安全的原子文件写回。
//!
//! 实现 spec 的 FR-019a：写回采用"先写同目录临时文件、再原子改名覆盖原文件"的方式，
//! 确保程序中途崩溃（如磁盘写满）时不会产生半写损坏的文件。
//!
//! 关键点：临时文件必须与目标文件位于**同一目录**，否则 rename 可能跨文件系统而失去原子性。

use std::fs;
use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;

/// 以原子方式将 `content` 写入 `target` 路径。
///
/// 步骤：
/// 1. 在 `target` 所在目录创建临时文件；
/// 2. 写入全部内容并 flush；
/// 3. 尽力保留原文件的权限位（Unix）；
/// 4. 通过 `persist` 原子改名覆盖目标文件。
///
/// # 参数
/// - `target`：要覆盖的目标文件路径。
/// - `content`：新的文件内容（字节）。
///
/// # 错误
/// 当临时文件创建、写入或持久化失败时返回 IO 错误。
pub fn write_atomic(target: &Path, content: &[u8]) -> std::io::Result<()> {
    // 目标文件所在目录；若无父目录则使用当前目录。
    let dir = target.parent().filter(|p| !p.as_os_str().is_empty());

    let mut tmp = match dir {
        Some(d) => NamedTempFile::new_in(d)?,
        None => NamedTempFile::new()?,
    };

    tmp.write_all(content)?;
    tmp.flush()?;

    // 尽力保留原文件权限（仅在原文件存在时）。
    if let Ok(meta) = fs::metadata(target) {
        let perm = meta.permissions();
        // 忽略权限设置失败：不应因此中断替换。
        let _ = tmp.as_file().set_permissions(perm);
    }

    // persist 在多数平台上等价于原子 rename 覆盖。
    tmp.persist(target).map_err(|e| e.error)?;
    Ok(())
}
