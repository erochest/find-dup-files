use std::process::Command;

use assert_cmd::prelude::*;
use spectral::prelude::*;

#[test]
fn test_help() {
    Command::cargo_bin(env!["CARGO_PKG_NAME"])
        .unwrap()
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn test_takes_directory() {
    let command = Command::cargo_bin(env!["CARGO_PKG_NAME"])
        .unwrap()
        .args(&["-vv", "--directory", "tests/fixtures", "--action", "list"])
        .assert();

    let output = command.success();
    let output = output.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.as_ref();

    assert_that(&stderr).contains("/a");
    assert_that(&stderr).contains("/sub/b");
}
