mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::{repository, run_jj, successful_output};

fn add_bare_origin(directory: &Path) -> tempfile::TempDir {
    let remote = tempfile::tempdir().unwrap();
    assert!(
        Command::new("git")
            .args(["init", "--bare", "."])
            .current_dir(remote.path())
            .status()
            .unwrap()
            .success()
    );
    assert!(
        run_jj(
            directory,
            &[
                "git",
                "remote",
                "add",
                "origin",
                remote.path().to_str().unwrap(),
            ],
        )
        .status
        .success()
    );
    remote
}

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
fn bookmark_list_exposes_cached_local_state_without_mutating_operations() {
    let repo = repository();
    assert!(
        run_jj(repo.path(), &["describe", "-m", "listed"])
            .status
            .success()
    );
    assert!(
        run_jj(repo.path(), &["bookmark", "set", "feature", "-r", "@"])
            .status
            .success()
    );
    let before = run_jj(
        repo.path(),
        &["op", "log", "--no-graph", "-n", "1", "-T", "id"],
    );

    let output = successful_output(repo.path(), &["bookmark", "list", "--name", "feature"]);

    assert!(output.contains("kind: bookmark_list"));
    assert!(output.contains("remote_data_source: local_tracking_state"));
    assert!(output.contains("name: feature"));
    assert!(output.contains("local:"));
    assert!(output.contains("present: true"));
    assert!(output.contains("conflicted: false"));
    assert!(output.contains("added_change_ids[1]"));
    assert!(output.contains("removed_change_ids: []"));
    assert!(output.contains("remotes: []"));
    let missing = successful_output(repo.path(), &["bookmark", "list", "--name", "missing"]);
    assert!(missing.contains("bookmarks: []"));

    let after = run_jj(
        repo.path(),
        &["op", "log", "--no-graph", "-n", "1", "-T", "id"],
    );
    assert_eq!(before.stdout, after.stdout);
}

#[test]
fn bookmark_list_pages_grouped_names_with_an_exclusive_cursor() {
    let repo = repository();
    for name in ["alpha", "beta", "gamma"] {
        assert!(
            run_jj(repo.path(), &["bookmark", "set", name, "-r", "@"])
                .status
                .success()
        );
    }

    let first = successful_output(repo.path(), &["bookmark", "list", "--limit", "2"]);
    assert!(first.contains("name: alpha"));
    assert!(first.contains("name: beta"));
    assert!(!first.contains("name: gamma"));
    assert!(first.contains("truncated: true"));
    assert!(first.contains("next_after: beta"));

    let second = successful_output(
        repo.path(),
        &["bookmark", "list", "--limit", "2", "--after", "beta"],
    );
    assert!(!second.contains("name: alpha"));
    assert!(!second.contains("name: beta"));
    assert!(second.contains("name: gamma"));
    assert!(second.contains("truncated: false"));
    assert!(second.contains("next_after: null"));

    common::assert_error(
        common::run_axi(
            repo.path(),
            &["bookmark", "list", "--name", "alpha", "--after", "beta"],
        ),
        "invalid_argument",
    );
}

#[test]
fn bookmark_list_computes_cached_commit_topology_against_remote() {
    let repo = repository();
    let _remote = add_bare_origin(repo.path());
    assert!(
        run_jj(repo.path(), &["describe", "-m", "published"])
            .status
            .success()
    );
    successful_output(repo.path(), &["finish", "@", "--bookmark", "feature"]);
    assert!(
        run_jj(repo.path(), &["new", "-m", "local"])
            .status
            .success()
    );
    assert!(
        run_jj(repo.path(), &["bookmark", "set", "feature", "-r", "@"])
            .status
            .success()
    );

    let output = successful_output(repo.path(), &["bookmark", "list", "--name", "feature"]);

    assert!(output.contains("remote: origin"));
    assert!(output.contains("tracking: true"));
    assert!(output.contains("comparison_status: available"));
    assert!(output.contains("ahead: 1"));
    assert!(output.contains("behind: 0"));

    assert!(
        run_jj(repo.path(), &["bookmark", "delete", "feature"])
            .status
            .success()
    );
    let remote_only = successful_output(repo.path(), &["bookmark", "list", "--name", "feature"]);
    assert!(remote_only.contains("name: feature"));
    assert!(remote_only.contains("comparison_status: local_missing"));
    assert!(remote_only.contains("ahead: null"));
    assert!(remote_only.contains("behind: null"));
}

#[test]
fn bookmark_set_is_safe_idempotent_and_supports_explicit_backward_moves() {
    let repo = repository();
    assert!(
        run_jj(repo.path(), &["describe", "-m", "base"])
            .status
            .success()
    );

    let created = successful_output(repo.path(), &["bookmark", "set", "main", "--to", "@"]);
    assert!(created.contains("kind: bookmark_set"));
    assert!(created.contains("name: main"));
    assert!(created.contains("action: created"));
    assert!(created.contains("target_change_id:"));
    assert!(created.contains("target_commit_id:"));

    let unchanged = successful_output(repo.path(), &["bookmark", "set", "main", "--to", "@"]);
    assert!(unchanged.contains("action: unchanged"));

    assert!(
        run_jj(repo.path(), &["new", "-m", "child"])
            .status
            .success()
    );
    let moved = successful_output(repo.path(), &["bookmark", "set", "main", "--to", "@"]);
    assert!(moved.contains("action: moved"));

    common::assert_error(
        common::run_axi(repo.path(), &["bookmark", "set", "main", "--to", "@-"]),
        "bookmark_move_rejected",
    );
    let backwards = successful_output(
        repo.path(),
        &["bookmark", "set", "main", "--to", "@-", "--allow-backwards"],
    );
    assert!(backwards.contains("action: moved"));
}

#[test]
fn bookmark_push_publishes_exact_name_and_is_idempotent() {
    let repo = repository();
    let remote = add_bare_origin(repo.path());
    assert!(
        run_jj(repo.path(), &["describe", "-m", "ready"])
            .status
            .success()
    );
    successful_output(repo.path(), &["bookmark", "set", "feature", "--to", "@"]);

    let first = successful_output(
        repo.path(),
        &["bookmark", "push", "feature", "--remote", "origin"],
    );
    assert!(first.contains("kind: bookmark_push"));
    assert!(first.contains("name: feature"));
    assert!(first.contains("remote: origin"));
    assert!(first.contains("action: created"));
    assert!(first.contains("target_change_id:"));
    assert!(first.contains("target_commit_id:"));
    assert!(
        Command::new("git")
            .args(["show-ref", "--verify", "--quiet", "refs/heads/feature"])
            .current_dir(remote.path())
            .status()
            .unwrap()
            .success()
    );

    let second = successful_output(repo.path(), &["bookmark", "push", "feature"]);
    assert!(second.contains("action: unchanged"));
}

#[test]
fn bookmark_push_reuses_readiness_and_has_command_specific_partial_errors() {
    let repo = repository();
    let remote = add_bare_origin(repo.path());
    successful_output(repo.path(), &["bookmark", "set", "feature", "--to", "@"]);
    common::assert_error(
        common::run_axi(repo.path(), &["bookmark", "push", "feature"]),
        "change_not_ready",
    );

    assert!(
        run_jj(repo.path(), &["describe", "-m", "ready"])
            .status
            .success()
    );
    remote.close().unwrap();
    let partial = common::assert_error(
        common::run_axi(repo.path(), &["bookmark", "push", "feature"]),
        "bookmark_push_partial",
    );
    assert!(partial.contains("remote_state: unknown"));
    assert!(partial.contains("reason: transport_or_authentication"));
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
