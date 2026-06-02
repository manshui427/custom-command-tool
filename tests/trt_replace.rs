//! US1 集成测试：trt 文本替换核心功能。
//!
//! 覆盖 spec 的 US1 验收场景：单组替换、多组规则单遍无级联、正则捕获组、大小写不敏感、
//! 目标目录不存在报错。

use std::fs;

use assert_cmd::Command;
use tempfile::TempDir;

/// 在临时目录写入一个文本文件，返回临时目录句柄。
fn setup_file(name: &str, content: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(name), content).unwrap();
    dir
}

/// 读取临时目录下某文件的内容。
fn read_file(dir: &TempDir, name: &str) -> String {
    fs::read_to_string(dir.path().join(name)).unwrap()
}

#[test]
fn 单组字面替换() {
    let dir = setup_file("a.txt", "hello foo world\nfoo bar foo\n");

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "foo",
            "-n",
            "REPLACED",
            "-b",
            "0",
            "--progress",
            "0",
        ])
        .assert()
        .success();

    assert_eq!(
        read_file(&dir, "a.txt"),
        "hello REPLACED world\nREPLACED bar REPLACED\n"
    );
}

#[test]
fn 多组规则单遍无级联() {
    // foo->bar, bar->X：foo 替换出的 bar 不应被再次替换为 X。
    let dir = setup_file("e.txt", "foo bar\n");
    let rules = dir.path().join("rules.tsv");
    fs::write(&rules, "foo\tbar\nbar\tX\n").unwrap();

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "--rules",
            rules.to_str().unwrap(),
            "-b",
            "0",
            "--progress",
            "0",
        ])
        .assert()
        .success();

    assert_eq!(read_file(&dir, "e.txt"), "bar X\n");
}

#[test]
fn 正则捕获组替换() {
    // yyyy-mm -> mm/yyyy
    let dir = setup_file("d.txt", "date 2026-06 end\n");

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-r",
            "1",
            "-o",
            r"([0-9]{4})-([0-9]{2})",
            "-n",
            "${2}/${1}",
            "-b",
            "0",
            "--progress",
            "0",
        ])
        .assert()
        .success();

    assert_eq!(read_file(&dir, "d.txt"), "date 06/2026 end\n");
}

#[test]
fn 大小写不敏感替换() {
    let dir = setup_file("c.txt", "Hello HELLO hello\n");

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "hello",
            "-n",
            "hi",
            "-c",
            "0",
            "-b",
            "0",
            "--progress",
            "0",
        ])
        .assert()
        .success();

    assert_eq!(read_file(&dir, "c.txt"), "hi hi hi\n");
}

#[test]
fn 目标目录不存在应报错() {
    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            "/path/does/not/exist/cct_xyz",
            "-o",
            "a",
            "-n",
            "b",
            "--progress",
            "0",
        ])
        .assert()
        .failure();
}

#[test]
fn 空旧文本应报错() {
    let dir = setup_file("a.txt", "content\n");

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "",
            "-n",
            "x",
            "--progress",
            "0",
        ])
        .assert()
        .failure();
}
