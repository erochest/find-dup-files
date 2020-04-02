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
fn test_walks_directory() {
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

#[test]
fn test_hashes_files() {
    let command = Command::cargo_bin(env!["CARGO_PKG_NAME"])
        .unwrap()
        .args(&["-vv", "--directory", "tests/fixtures", "--action", "hash"])
        .assert();

    let output = command.success();
    let output = output.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.as_ref();

    assert_that(&stderr).contains("i-measure-every-grief-i-meet\t36b99c2909e5ecdaf9db08544e134d7165f890f03fba29934c3eafdc67a26ec5");
    assert_that(&stderr).contains("im-nobody-who-are-you\t12cfa77c4b4d8d493fdde29cf0856b2f8f09082c5c47788ad270001a983d9dc5");
}
