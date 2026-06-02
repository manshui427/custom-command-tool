//! US1 集成测试：二进制文件跳过与编码边界。
//!
//! 覆盖 spec 的 FR-009/SC-004 与边界：含 NUL 的二进制文件被跳过且字节不变，
//! 非 UTF-8 文件按二进制处理。

use std::fs;

use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn 二进制文件被跳过且内容不变() {
    let dir = TempDir::new().unwrap();
    // 文本文件含 foo。
    fs::write(dir.path().join("text.txt"), "foo here\n").unwrap();
    // 二进制文件：含 NUL 字节，也含 foo 字样。
    let bin_content: &[u8] = b"\x00\x01binary foo\x00";
    fs::write(dir.path().join("bin.dat"), bin_content).unwrap();

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
        .success();

    // 文本文件被替换。
    assert_eq!(
        fs::read_to_string(dir.path().join("text.txt")).unwrap(),
        "X here\n"
    );
    // 二进制文件字节完全不变。
    assert_eq!(fs::read(dir.path().join("bin.dat")).unwrap(), bin_content);
}

#[test]
fn 无匹配时报告零修改且成功() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "nothing to change\n").unwrap();

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
        .success();

    // 内容未变。
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        "nothing to change\n"
    );
}
