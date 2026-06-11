//! US1 集成测试：trt 统计以"被影响的文件"为核心。
//!
//! 覆盖 spec 的 FR-001/FR-002/FR-004/FR-005 与 contracts/trt-stats-output.md：
//! 摘要以被影响文件数与总替换次数为主体；无命中显示 0 且成功退出；次要信息仍保留。

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// 在临时目录写入一个文本文件，返回临时目录句柄。
fn setup_file(name: &str, content: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(name), content).unwrap();
    dir
}

#[test]
fn 摘要以被影响文件为核心() {
    // 两个文本文件，其中只有部分包含待替换文本。
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "foo foo\n").unwrap(); // 2 处
    fs::write(dir.path().join("b.txt"), "foo here\n").unwrap(); // 1 处
    fs::write(dir.path().join("c.txt"), "nothing\n").unwrap(); // 0 处

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "foo",
            "-n",
            "X",
            "-b",
            "0",
            "--progress",
            "0",
        ])
        .assert()
        .success()
        // 首要信息：被影响 2 个文件、共替换 3 处。
        .stdout(predicate::str::contains("被影响 2 个文件"))
        .stdout(predicate::str::contains("共替换 3 处"));
}

#[test]
fn 无命中显示被影响零个文件() {
    let dir = setup_file("a.txt", "nothing to change\n");

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "absent",
            "-n",
            "x",
            "-b",
            "0",
            "--progress",
            "0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("被影响 0 个文件"));
}

#[test]
fn 次要信息仍保留() {
    // 含一个二进制文件，验证次要信息（扫描/跳过）仍出现在输出中。
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "foo\n").unwrap();
    fs::write(dir.path().join("bin.dat"), b"\x00\x01foo\x00").unwrap();

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "foo",
            "-n",
            "X",
            "-b",
            "0",
            "--progress",
            "0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("被影响 1 个文件"))
        // 次要信息：扫描总数与跳过二进制数仍可见。
        .stdout(predicate::str::contains("扫描"))
        .stdout(predicate::str::contains("跳过二进制"));
}
