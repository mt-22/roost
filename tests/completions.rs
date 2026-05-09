use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn completions_generates_bash() {
    Command::cargo_bin("roost")
        .unwrap()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("roost"));
}

#[test]
fn completions_generates_zsh() {
    Command::cargo_bin("roost")
        .unwrap()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("roost"));
}

#[test]
fn completions_rejects_invalid_shell() {
    Command::cargo_bin("roost")
        .unwrap()
        .args(["completions", "notashell"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unsupported shell"));
}
