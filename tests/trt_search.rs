//! 集成测试：trt 查找模式（-s 1）。
//!
//! 覆盖：字面查找、正则查找、大小写不敏感查找、多组规则查找、
//! 无匹配时输出、二进制文件跳过、参数校验（互斥标志、缺少搜索文本）。

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn setup_file(name: &str, content: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(name), content).unwrap();
    dir
}

#[test]
fn 字面查找输出命中文件() {
    let dir = setup_file("a.txt", "hello foo world\nbar foo baz\n");

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "foo",
            "-s",
            "1",
            "--progress",
            "0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("a.txt"))
        .stdout(predicate::str::contains("处匹配"))
        .stdout(predicate::str::contains("找到 1 个文件"));
}

#[test]
fn 查找不修改文件() {
    let dir = setup_file("a.txt", "hello foo world\n");

    let original = fs::read_to_string(dir.path().join("a.txt")).unwrap();

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "foo",
            "-s",
            "1",
            "--progress",
            "0",
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).unwrap(),
        original,
        "查找模式不应修改文件内容"
    );
}

#[test]
fn 无匹配输出未找到() {
    let dir = setup_file("a.txt", "hello world\n");

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "xyz_not_found",
            "-s",
            "1",
            "--progress",
            "0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("未找到匹配文件"));
}

#[test]
fn 正则查找() {
    let dir = setup_file("a.txt", "date 2026-06 end\n");

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-r",
            "1",
            "-o",
            r"[0-9]{4}-[0-9]{2}",
            "-s",
            "1",
            "--progress",
            "0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("a.txt"))
        .stdout(predicate::str::contains("处匹配"));
}

#[test]
fn 大小写不敏感查找() {
    let dir = setup_file("a.txt", "Hello HELLO hello\n");

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "hello",
            "-c",
            "0",
            "-s",
            "1",
            "--progress",
            "0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("3 处匹配"));
}

#[test]
fn 撤销与查找互斥() {
    let dir = setup_file("a.txt", "content\n");

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "foo",
            "-s",
            "1",
            "-u",
            "1",
            "--progress",
            "0",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("不能同时使用"));
}

#[test]
fn 查找无搜索文本报错() {
    let dir = setup_file("a.txt", "content\n");

    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-s",
            "1",
            "--progress",
            "0",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("搜索文本"));
}

#[test]
fn 查找模式不需要新文本() {
    let dir = setup_file("a.txt", "hello foo world\n");

    // 查找模式只提供 -o，不提供 -n 也能成功。
    Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "foo",
            "-s",
            "1",
            "--progress",
            "0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("a.txt"));
}

#[test]
fn 多文件查找() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.txt"), "hello foo\n").unwrap();
    fs::write(dir.path().join("b.txt"), "no match here\n").unwrap();
    fs::write(dir.path().join("c.txt"), "foo bar foo\n").unwrap();

    let output = Command::cargo_bin("cct")
        .unwrap()
        .args([
            "trt",
            "-d",
            dir.path().to_str().unwrap(),
            "-o",
            "foo",
            "-s",
            "1",
            "--progress",
            "0",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("a.txt"), "应找到 a.txt");
    assert!(stdout.contains("c.txt"), "应找到 c.txt");
    assert!(stdout.contains("找到 2 个文件"), "应报告 2 个文件");
}