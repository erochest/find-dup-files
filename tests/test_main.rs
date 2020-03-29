use std::process::Command;

use assert_cmd::prelude::*;

#[test]
fn test_run() {
    Command::cargo_bin(env!["CARGO_PKG_NAME"])
        .unwrap()
        .arg("--help")
        .assert()
        .success();
}
