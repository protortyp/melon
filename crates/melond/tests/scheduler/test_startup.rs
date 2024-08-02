use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

#[test]
fn test_startup() {
    let mut cmd = Command::cargo_bin("melond").unwrap();

    let assert = cmd.timeout(Duration::from_secs(2)).assert();
    assert.stdout(predicate::str::contains("Starting scheduler on"));
}
