//! US3 集成测试：主命令探索与帮助。
//!
//! 覆盖 spec 的 US3 验收场景与 SC-008：`-ls`/`--ls` 列出子命令、`-h` 帮助、
//! 未知子命令报错。
//! 注：启用 gui 时无子命令默认打开图形界面而非打印列表，故原"无子命令等同列表"
//! 测试已移除，改为验证 `--ls` 显式列出子命令。

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
        .stdout(predicate::str::contains("文本替换"));
}

#[test]
fn 显式_ls_列出子命令() {
    // gui 模式下无参数打开窗口，需用 --ls 显式列出。
    Command::cargo_bin("cct")
        .unwrap()
        .arg("--ls")
        .assert()
        .success()
        .stdout(predicate::str::contains("trt"));
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
