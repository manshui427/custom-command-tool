//! US2 集成测试：备份与一键撤销。
//!
//! 覆盖 spec 的 US2 验收场景与 SC-005/SC-006：备份生成、撤销 100% 还原并删除归档、
//! `-b 0` 不备份、多目录共享同一 backup/ 按目录归属互不误还原。

use std::fs;

use assert_cmd::Command;
use tempfile::TempDir;

/// 在父临时目录下创建一个子目录作为目标目录（确保 backup/ 落在受控的同级位置）。
fn make_target(parent: &TempDir, name: &str) -> std::path::PathBuf {
    let dir = parent.path().join(name);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_replace(target: &std::path::Path, old: &str, new: &str, backup: &str) {
    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            target.to_str().unwrap(),
            "-o",
            old,
            "-n",
            new,
            "-b",
            backup,
            "--progress",
            "0",
        ])
        .assert()
        .success();
}

fn run_undo(target: &std::path::Path) {
    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            target.to_str().unwrap(),
            "-u",
            "1",
            "--progress",
            "0",
        ])
        .assert()
        .success();
}

#[test]
fn 备份生成且撤销完整还原() {
    let parent = TempDir::new().unwrap();
    let target = make_target(&parent, "proj");
    fs::write(target.join("a.txt"), "foo bar\n").unwrap();

    run_replace(&target, "foo", "REPLACED", "1");
    assert_eq!(
        fs::read_to_string(target.join("a.txt")).unwrap(),
        "REPLACED bar\n"
    );

    // 备份目录应在目标目录同级生成 zip。
    let backup_dir = parent.path().join("backup");
    let zips: Vec<_> = fs::read_dir(&backup_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "zip").unwrap_or(false))
        .collect();
    assert_eq!(zips.len(), 1, "应恰好生成一个备份 zip");

    // 撤销后还原原始内容，且备份被删除。
    run_undo(&target);
    assert_eq!(
        fs::read_to_string(target.join("a.txt")).unwrap(),
        "foo bar\n"
    );
    let zips_after: Vec<_> = fs::read_dir(&backup_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "zip").unwrap_or(false))
        .collect();
    assert_eq!(zips_after.len(), 0, "撤销后备份 zip 应被删除");
}

#[test]
fn 禁用备份时不生成归档() {
    let parent = TempDir::new().unwrap();
    let target = make_target(&parent, "proj");
    fs::write(target.join("a.txt"), "foo\n").unwrap();

    run_replace(&target, "foo", "X", "0");

    let backup_dir = parent.path().join("backup");
    // 备份目录要么不存在，要么没有 zip。
    let has_zip = backup_dir.is_dir()
        && fs::read_dir(&backup_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().map(|x| x == "zip").unwrap_or(false));
    assert!(!has_zip, "禁用备份时不应生成 zip");
}

#[test]
fn 多目录共享备份按目录归属还原() {
    // 两个目标目录共享同一父目录，因而共享同一 backup/。
    let parent = TempDir::new().unwrap();
    let bar = make_target(&parent, "bar");
    let baz = make_target(&parent, "baz");
    fs::write(bar.join("f.txt"), "foo\n").unwrap();
    fs::write(baz.join("f.txt"), "foo\n").unwrap();

    run_replace(&bar, "foo", "BAR", "1");
    run_replace(&baz, "foo", "BAZ", "1");
    assert_eq!(fs::read_to_string(bar.join("f.txt")).unwrap(), "BAR\n");
    assert_eq!(fs::read_to_string(baz.join("f.txt")).unwrap(), "BAZ\n");

    // 仅撤销 bar：bar 还原，baz 保持不变。
    run_undo(&bar);
    assert_eq!(
        fs::read_to_string(bar.join("f.txt")).unwrap(),
        "foo\n",
        "bar 应被还原"
    );
    assert_eq!(
        fs::read_to_string(baz.join("f.txt")).unwrap(),
        "BAZ\n",
        "baz 不应受影响"
    );
}
