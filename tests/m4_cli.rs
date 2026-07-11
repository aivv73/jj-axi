mod common;

use std::fs;

use common::{repository, run_jj, successful_output};

#[test]
fn operations_is_read_only_and_exposes_topology_and_policy() {
    let repo = repository();
    let before = run_jj(
        repo.path(),
        &["op", "log", "--no-graph", "-n", "1", "-T", "id"],
    );
    let before = String::from_utf8(before.stdout).unwrap();

    let first = successful_output(repo.path(), &["operations", "--limit", "2"]);
    let second = successful_output(repo.path(), &["operations", "--limit", "2"]);

    assert_eq!(first, second);
    assert!(first.contains("parent_operation_ids"));
    assert!(first.contains("kind: foundation"));
    assert!(first.contains("undo_candidate: false"));
    assert!(first.contains("current: true"));
    let after = run_jj(
        repo.path(),
        &["op", "log", "--no-graph", "-n", "1", "-T", "id"],
    );
    assert_eq!(before, String::from_utf8(after.stdout).unwrap());
}

#[test]
fn bare_undo_reverts_latest_mutation_but_preserves_newer_file_content() {
    let repo = repository();
    fs::write(repo.path().join("file"), "one\n").unwrap();
    assert!(
        run_jj(repo.path(), &["describe", "-m", "described"])
            .status
            .success()
    );
    fs::write(repo.path().join("file"), "two\n").unwrap();

    let output = successful_output(repo.path(), &["undo"]);

    assert!(output.contains("selection: latest_mutation"));
    assert!(output.contains("action: restored"));
    let description = run_jj(
        repo.path(),
        &["log", "-r", "@", "--no-graph", "-T", "description"],
    );
    assert!(description.status.success());
    assert_eq!(String::from_utf8(description.stdout).unwrap(), "");
    assert_eq!(
        fs::read_to_string(repo.path().join("file")).unwrap(),
        "two\n"
    );

    let second = common::run_axi(repo.path(), &["undo"]);
    common::assert_error(second, "nothing_to_undo");
    assert_eq!(
        fs::read_to_string(repo.path().join("file")).unwrap(),
        "two\n"
    );
}

#[test]
fn explicit_current_operation_is_idempotent() {
    let repo = repository();
    let output = successful_output(repo.path(), &["operations", "--limit", "1"]);
    let id = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("- operation_id: "))
        .unwrap()
        .trim_matches('"')
        .to_owned();

    let undo = successful_output(repo.path(), &["undo", "--to", &id]);
    assert!(undo.contains("action: unchanged"));
    assert!(undo.contains("selection: explicit"));
}
