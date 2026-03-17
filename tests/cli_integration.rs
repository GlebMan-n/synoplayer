//! CLI integration tests using assert_cmd.

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
#[ignore]
fn cli_help_shows_usage() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage"))
        .stdout(predicate::str::contains("synoplayer"));
}

#[test]
#[ignore]
fn cli_version_shows_version() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("synoplayer"));
}

#[test]
#[ignore]
fn cli_no_args_shows_help() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
#[ignore]
fn cli_unknown_command_fails() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("nonexistent")
        .assert()
        .failure();
}

#[test]
#[ignore]
fn cli_config_show_works() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("config")
        .arg("show")
        .assert()
        .success();
}

#[test]
#[ignore]
fn cli_cache_status_works() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("cache")
        .arg("status")
        .assert()
        .success();
}
