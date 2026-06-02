//! US3 集成测试：主命令探索与帮助。
//!
//! 覆盖 spec 的 US3 验收场景与 SC-008：`-ls`/`--ls` 列出子命令、`-h` 帮助、
//! 无子命令时等同列表、未知子命令报错。

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn 列出子命令() {
    // 规格要求的单横线 -ls 形式。
    Command::cargo_bin("cct")
        .unwrap()
        .arg("-ls")
        .assert()
        .success()
        .stdout(predicate::str::contains("trt"))
        .stdout(predicate::str::contains("文本替换工具"));
}

#[test]
fn 无子命令等同列表() {
    Command::cargo_bin("cct")
        .unwrap()
        .assert()
        .success()
        .stdout(predicate::str::contains("trt"))
        .stdout(predicate::str::contains("文本替换工具"));
}

#[test]
fn 帮助输出成功() {
    Command::cargo_bin("cct")
        .unwrap()
        .arg("-h")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage"));
}

#[test]
fn 未知子命令报错() {
    Command::cargo_bin("cct")
        .unwrap()
        .arg("nonexistent-subcommand")
        .assert()
        .failure();
}
