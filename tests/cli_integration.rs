//! CLI integration tests using assert_cmd.

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
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
fn cli_version_shows_version() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("synoplayer"));
}

#[test]
fn cli_no_args_shows_help() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn cli_unknown_command_fails() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("nonexistent")
        .assert()
        .failure();
}

#[test]
fn cli_config_show_works() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("config")
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains("[server]"));
}

#[test]
fn cli_cache_status_works() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("cache")
        .arg("status")
        .assert()
        .success();
}

// --- Stage 5: Ratings & Favorites ---

#[test]
fn cli_rate_requires_args() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("rate")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn cli_rate_help() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("rate")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Rate a song"));
}

#[test]
fn cli_favorite_requires_song_id() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("favorite")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn cli_unfavorite_requires_song_id() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("unfavorite")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn cli_favorites_help() {
    Command::cargo_bin("synoplayer")
        .unwrap()
        .arg("favorites")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("List favorites"));
}
