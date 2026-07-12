mod common;

use std::fs;

use common::{assert_error, repository, run_axi, run_jj, successful_output};

fn change_id(directory: &std::path::Path, revision: &str) -> String {
    let output = run_jj(
        directory,
        &["log", "-r", revision, "--no-graph", "-T", "change_id"],
    );
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn squash_is_editor_free_and_moves_full_content_to_the_parent() {
    let repo = repository();
    assert!(
        run_jj(repo.path(), &["describe", "-m", "parent"])
            .status
            .success()
    );
    assert!(
        run_jj(repo.path(), &["new", "-m", "source"])
            .status
            .success()
    );
    fs::write(repo.path().join("file"), "content\n").unwrap();
    let source = change_id(repo.path(), "@");

    let missing_message = assert_error(
        run_axi(repo.path(), &["squash", &source]),
        "squash_message_required",
    );
    assert!(missing_message.contains("source_change_id:"));

    let output = successful_output(repo.path(), &["squash", &source, "--message", "combined"]);
    assert!(output.contains("kind: squash"));
    assert!(output.contains("abandoned: true"));
    assert!(output.contains("description: \"combined\\n\""));
    assert!(output.contains("rebased_descendant_count:"));
    assert_eq!(
        fs::read_to_string(repo.path().join("file")).unwrap(),
        "content\n"
    );
    let hidden = run_jj(repo.path(), &["log", "-r", &source, "--no-graph"]);
    assert!(!hidden.status.success());
}

#[test]
fn abandon_rewrites_current_state_and_retries_idempotently() {
    let repo = repository();
    assert!(
        run_jj(repo.path(), &["describe", "-m", "remove me"])
            .status
            .success()
    );
    let source = change_id(repo.path(), "@");
    assert!(
        run_jj(repo.path(), &["bookmark", "set", "local", "-r", "@"])
            .status
            .success()
    );

    let first = successful_output(repo.path(), &["abandon", &source]);
    assert!(first.contains("kind: abandon"));
    assert!(first.contains("action: abandoned"));
    assert!(first.contains("affected_bookmarks[1]: local"));
    assert!(first.contains("current_change:"));

    let second = successful_output(repo.path(), &["abandon", &source]);
    assert!(second.contains("action: unchanged"));
    assert!(second.contains("rebased_descendant_count: 0"));
    assert!(second.contains("affected_bookmarks: []"));
}
