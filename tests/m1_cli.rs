mod common;

use common::{repository, run_axi, successful_output};
use std::fs;

#[test]
fn version_reports_the_compiled_package_version_without_a_repository() {
    let directory = tempfile::tempdir().expect("create temporary directory");
    let output = run_axi(directory.path(), &["--version"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        concat!("jj-axi ", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn no_arguments_prints_the_short_bootstrap_without_a_repository() {
    let directory = tempfile::tempdir().expect("create temporary directory");
    let output = run_axi(directory.path(), &[]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        include_bytes!("../skills/jj-axi/BOOTSTRAP.md")
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("jj-axi skill")
    );
    assert!(
        include_bytes!("../skills/jj-axi/BOOTSTRAP.md").len() * 3
            < include_bytes!("../skills/jj-axi/SKILL.md").len(),
        "bootstrap must stay substantially smaller than the full skill"
    );
}

#[test]
fn read_commands_snapshot_and_report_a_working_copy_change() {
    let directory = repository();
    fs::write(directory.path().join("alpha.txt"), "one\ntwo\n").expect("write fixture file");

    let inspect = successful_output(directory.path(), &["inspect"]);
    assert!(inspect.contains("kind: inspect"));
    assert!(inspect.contains("changed_files: 1"));
    assert!(inspect.contains("added_lines: 2"));
    assert!(inspect.contains("removed_lines: 0"));
    assert!(!inspect.contains("patch:"));

    let log = successful_output(
        directory.path(),
        &[
            "log",
            "--limit",
            "1",
            "--fields",
            "commit_id,parent_commit_ids",
        ],
    );
    assert!(log.contains("kind: log"));
    assert!(log.contains("commit_id:"));
    assert!(log.contains("parent_commit_ids[1]:"));
    assert!(log.contains("complete: true"));
    assert!(!log.contains("graph"));

    let conflicted = successful_output(directory.path(), &["log", "--conflicted"]);
    assert!(conflicted.contains("changes: []"));
    assert!(!conflicted.contains("change_id:"));

    let show = successful_output(directory.path(), &["show", "@"]);
    assert!(show.contains("kind: show"));
    assert!(show.contains("format: unified-diff-v1"));
    assert!(show.contains("+one\\n+two\\n"));
    assert!(show.contains("truncated: false"));

    let diff = successful_output(directory.path(), &["diff"]);
    assert!(diff.contains("kind: diff"));
    assert!(diff.contains("kind: working_copy"));
    assert!(diff.contains("diff --git a/alpha.txt b/alpha.txt\\n"));

    let change_diff = successful_output(directory.path(), &["diff", "@"]);
    assert!(change_diff.contains("kind: change"));
    assert!(change_diff.contains("change_id:"));
}

#[test]
fn default_diff_truncates_only_at_file_boundaries_and_full_disables_it() {
    let directory = repository();
    fs::write(
        directory.path().join("a-large.txt"),
        format!("{}\n", "x".repeat(20 * 1024)),
    )
    .expect("write large fixture file");
    fs::write(directory.path().join("z-small.txt"), "small\n").expect("write small fixture file");

    let bounded = successful_output(directory.path(), &["diff"]);
    assert!(bounded.contains("truncated: true"));
    assert!(bounded.contains("limit_bytes: 16384"));
    assert!(bounded.contains("body: \"\""));
    assert!(!bounded.contains("a-large.txt"));
    assert!(!bounded.contains("z-small.txt"));

    let full = successful_output(directory.path(), &["diff", "--full"]);
    assert!(full.contains("truncated: false"));
    assert!(full.contains("limit_bytes: null"));
    assert!(full.contains("a-large.txt"));
    assert!(full.contains("z-small.txt"));
}

#[test]
fn expected_failures_use_the_error_envelope() {
    let directory = repository();
    let invalid = run_axi(directory.path(), &["log", "--limit", "0"]);
    assert!(!invalid.status.success());
    assert!(invalid.stderr.is_empty());
    assert_eq!(
        String::from_utf8(invalid.stdout).expect("UTF-8 error output"),
        concat!(
            "schema_version: 1\n",
            "kind: error\n",
            "error:\n",
            "  code: invalid_argument\n",
            "  argument: command_line\n",
            "  constraint: valid_command_syntax"
        )
    );

    let unknown_revision = run_axi(directory.path(), &["show", "does-not-exist"]);
    assert!(!unknown_revision.status.success());
    assert!(unknown_revision.stderr.is_empty());
    let output = String::from_utf8(unknown_revision.stdout).expect("UTF-8 error output");
    assert!(output.contains("code: revision_not_found"));
    assert!(output.contains("revision: does-not-exist"));

    let outside = tempfile::tempdir().expect("create non-repository directory");
    let missing = run_axi(outside.path(), &["inspect"]);
    assert!(!missing.status.success());
    assert!(missing.stderr.is_empty());
    let output = String::from_utf8(missing.stdout).expect("UTF-8 error output");
    assert!(output.contains("kind: error"));
    assert!(output.contains("code: repository_not_found"));
}
