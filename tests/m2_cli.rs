mod common;

use common::{
    assert_error, commit_id, jj_template, repository, run_axi, run_jj, successful_output,
};
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn run_git(directory: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(directory)
        .output()
        .expect("run git")
}

fn bare_remote() -> TempDir {
    let remote = tempfile::tempdir().expect("create bare remote directory");
    let output = Command::new("git")
        .args(["init", "--bare", "."])
        .current_dir(remote.path())
        .output()
        .expect("initialize bare remote");
    assert!(
        output.status.success(),
        "git init --bare failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    remote
}

fn add_origin(directory: &Path, remote: &Path) {
    let remote = remote.to_str().expect("remote path is UTF-8");
    let output = run_jj(directory, &["git", "remote", "add", "origin", remote]);
    assert!(
        output.status.success(),
        "jj git remote add failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config_path = directory.join(".jj").join("jj-axi-test-config.toml");
    let mut config = fs::OpenOptions::new()
        .append(true)
        .open(config_path)
        .expect("open jj test config");
    writeln!(config, "git.push = 'origin'").expect("configure push remote");
}

fn show_ref(remote: &Path, bookmark: &str) -> bool {
    let ref_name = format!("refs/heads/{bookmark}");
    run_git(remote, &["show-ref", "--verify", "--quiet", &ref_name])
        .status
        .success()
}
fn remote_ref(remote: &Path, bookmark: &str) -> String {
    let ref_name = format!("refs/heads/{bookmark}");
    let output = run_git(remote, &["rev-parse", "--verify", &ref_name]);
    assert!(
        output.status.success(),
        "git rev-parse {:?} failed: {}",
        ref_name,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git ref output is UTF-8")
        .trim()
        .to_owned()
}

fn signature(directory: &Path, revision: &str) -> String {
    jj_template(
        directory,
        revision,
        r#"if(signature, signature.status() ++ ":" ++ signature.key(), "unsigned")"#,
    )
}

#[test]
fn new_supports_message_and_empty_child() {
    let directory = repository();

    let with_message = successful_output(directory.path(), &["new", "--message", "  café  "]);
    assert!(with_message.starts_with("schema_version: 1\nkind: new\n"));
    assert!(with_message.contains("current_change:\n"));
    assert!(with_message.contains("description: \"  café  \\n\""));
    assert_ne!(
        commit_id(directory.path(), "@"),
        commit_id(directory.path(), "@-"),
        "new must create a child change"
    );
    let new_diff = run_jj(directory.path(), &["diff", "-r", "@"]);
    assert!(new_diff.status.success());
    assert!(
        new_diff.stdout.is_empty(),
        "new child must preserve its parent tree"
    );

    let without_message = successful_output(directory.path(), &["new"]);
    assert!(without_message.starts_with("schema_version: 1\nkind: new\n"));
    assert!(without_message.contains("description: \"\""));
    let status = run_jj(directory.path(), &["status"]);
    assert!(String::from_utf8_lossy(&status.stdout).contains("no changes."));
}

#[test]
fn describe_normalizes_and_is_idempotent() {
    let directory = repository();

    let first = successful_output(
        directory.path(),
        &["describe", "@", "--message", "  first\n\nsecond  "],
    );
    assert!(first.contains("kind: describe"));
    assert!(first.contains("changed: true"));
    assert!(first.contains("description: \"  first\\n\\nsecond  \\n\""));

    let second = successful_output(
        directory.path(),
        &["describe", "@", "--message", "  first\n\nsecond  "],
    );
    assert!(second.contains("kind: describe"));
    assert!(second.contains("changed: false"));
    assert!(second.contains("description: \"  first\\n\\nsecond  \\n\""));
}

#[test]
fn checkpoint_snapshots_tree_opens_empty_child_and_preserves_bookmarks() {
    let directory = repository();
    fs::write(directory.path().join("snapshot.txt"), "snapshot\n").expect("write snapshot");
    let bookmark = run_jj(directory.path(), &["bookmark", "set", "keep", "-r", "@"]);
    assert!(bookmark.status.success(), "set bookmark failed");

    let output = successful_output(
        directory.path(),
        &["checkpoint", "--message", "  snapshot message  "],
    );
    assert!(output.starts_with("schema_version: 1\nkind: checkpoint\n"));
    assert!(output.contains("checkpoint:\n"));
    assert!(output.contains("description: \"  snapshot message  \\n\""));
    assert!(output.contains("current_change:\n"));
    assert!(output.contains("description: \"\""));

    let status = run_jj(directory.path(), &["status"]);
    assert!(String::from_utf8_lossy(&status.stdout).contains("no changes."));
    let current_diff = run_jj(directory.path(), &["diff", "-r", "@"]);
    assert!(current_diff.status.success());
    assert!(current_diff.stdout.is_empty(), "new child must be empty");
    let checkpoint_diff = run_jj(directory.path(), &["diff", "-r", "@-"]);
    assert!(checkpoint_diff.status.success());
    assert!(
        String::from_utf8_lossy(&checkpoint_diff.stdout).contains("snapshot.txt"),
        "checkpoint must contain the snapped file"
    );
    assert_eq!(
        commit_id(directory.path(), "keep"),
        commit_id(directory.path(), "@-"),
        "bookmark must remain on checkpoint"
    );
}

#[test]
fn finish_without_bookmark_requires_readiness_then_skips_publication() {
    let directory = repository();

    let not_ready = assert_error(
        run_axi(directory.path(), &["finish", "@"]),
        "change_not_ready",
    );
    assert!(not_ready.contains("reasons[1]: empty_description"));

    let finished = successful_output(
        directory.path(),
        &["finish", "@", "--message", "ready change"],
    );
    assert!(finished.starts_with("schema_version: 1\nkind: finish\n"));
    assert!(finished.contains("description_action: updated"));
    assert!(finished.contains("publication:\n    status: skipped"));
    assert!(!finished.contains("bookmark:"));
}

#[test]
fn finish_reports_readiness_reasons_in_stable_order() {
    let directory = repository();
    successful_output(directory.path(), &["new", "--message", "left"]);
    fs::write(directory.path().join("conflict.txt"), "left\n").expect("write left side");
    let left = run_jj(directory.path(), &["bookmark", "set", "left", "-r", "@"]);
    assert!(left.status.success(), "set left bookmark failed");

    let branched = run_jj(directory.path(), &["new", "root()", "-m", "right"]);
    assert!(branched.status.success(), "create right branch failed");
    fs::write(directory.path().join("conflict.txt"), "right\n").expect("write right side");
    let right = run_jj(directory.path(), &["bookmark", "set", "right", "-r", "@"]);
    assert!(right.status.success(), "set right bookmark failed");
    let merged = run_jj(directory.path(), &["new", "left", "right"]);
    assert!(merged.status.success(), "create merge child failed");

    let not_ready = assert_error(
        run_axi(directory.path(), &["finish", "@"]),
        "change_not_ready",
    );
    assert!(not_ready.contains("reasons[2]: empty_description,conflicted"));
}

#[test]
fn finish_bookmark_pushes_and_retries_as_noop() {
    let directory = repository();
    let remote = bare_remote();
    add_origin(directory.path(), remote.path());
    successful_output(
        directory.path(),
        &["describe", "@", "--message", "publish me"],
    );

    let first = successful_output(directory.path(), &["finish", "@", "--bookmark", "main"]);
    assert!(first.contains("publication:\n    status: complete"));
    assert!(first.contains("bookmark: main"));
    assert!(first.contains("remote: origin"));
    assert!(first.contains("local_action: created"));
    assert!(first.contains("remote_action: created"));
    assert!(show_ref(remote.path(), "main"), "finish must push bookmark");

    let second = successful_output(directory.path(), &["finish", "@", "--bookmark", "main"]);
    assert!(second.contains("description_action: unchanged"));
    assert!(second.contains("local_action: unchanged"));
    assert!(second.contains("remote_action: unchanged"));
    assert!(
        show_ref(remote.path(), "main"),
        "retry must preserve remote bookmark"
    );
}

#[test]
fn finish_bookmark_signing_rewrites_target_and_preserves_out_of_range_ancestor() {
    let directory = repository();
    let remote = bare_remote();
    add_origin(directory.path(), remote.path());

    successful_output(
        directory.path(),
        &["describe", "@", "--message", "unsigned ancestor"],
    );
    successful_output(directory.path(), &["finish", "@", "--bookmark", "base"]);
    let ancestor_id = commit_id(directory.path(), "base");
    assert_eq!(signature(directory.path(), "base"), "unsigned");

    successful_output(directory.path(), &["new", "--message", "signed target"]);
    let unsigned_target_id = commit_id(directory.path(), "@");
    assert_eq!(signature(directory.path(), "@"), "unsigned");

    let config_path = directory.path().join(".jj").join("jj-axi-test-config.toml");
    let mut config = fs::OpenOptions::new()
        .append(true)
        .open(config_path)
        .expect("open jj test config");
    writeln!(config, "signing.backend = 'test'").expect("append signing backend");
    writeln!(config, "signing.key = 'impeccable'").expect("append signing key");
    writeln!(config, "git.sign-on-push = true").expect("append sign-on-push setting");

    let finished = successful_output(directory.path(), &["finish", "@", "--bookmark", "main"]);
    assert!(finished.contains("publication:\n    status: complete"));
    assert!(finished.contains("bookmark: main"));

    let local_target_id = commit_id(directory.path(), "main");
    let pushed_target_id = remote_ref(remote.path(), "main");
    assert_ne!(
        local_target_id, unsigned_target_id,
        "signing must rewrite the outgoing target"
    );
    assert_eq!(
        local_target_id, pushed_target_id,
        "local bookmark must point at the pushed signed target"
    );
    assert_eq!(signature(directory.path(), "main"), "good:impeccable");
    assert_eq!(
        commit_id(directory.path(), "base"),
        ancestor_id,
        "signing must not rewrite the out-of-range ancestor"
    );
    assert_eq!(
        signature(directory.path(), "base"),
        "unsigned",
        "signing must not sign the out-of-range ancestor"
    );

    let status = run_jj(directory.path(), &["status"]);
    assert!(status.status.success(), "jj status failed");
    assert!(
        String::from_utf8_lossy(&status.stdout).contains("no changes."),
        "finish must leave a fresh working copy: {}",
        String::from_utf8_lossy(&status.stdout)
    );
    assert_eq!(
        commit_id(directory.path(), "@"),
        local_target_id,
        "fresh working copy must be the signed target"
    );
}

#[test]
fn finish_rejects_backward_bookmark_move() {
    let directory = repository();
    let remote = bare_remote();
    add_origin(directory.path(), remote.path());
    successful_output(directory.path(), &["describe", "@", "--message", "base"]);
    successful_output(directory.path(), &["finish", "@", "--bookmark", "main"]);
    successful_output(directory.path(), &["new", "--message", "child"]);
    successful_output(directory.path(), &["finish", "@", "--bookmark", "main"]);

    let rejected = assert_error(
        run_axi(directory.path(), &["finish", "@-", "--bookmark", "main"]),
        "bookmark_move_rejected",
    );
    assert!(rejected.contains("bookmark: main"));
    assert!(rejected.contains("reason: backwards_or_sideways"));
}

#[test]
fn unreachable_remote_finish_partial_retains_local_state() {
    let directory = repository();
    let remote = bare_remote();
    add_origin(directory.path(), remote.path());
    successful_output(directory.path(), &["describe", "@", "--message", "offline"]);

    let missing_remote = remote.path().join("does-not-exist.git");
    let missing_remote = missing_remote.to_str().expect("missing path is UTF-8");
    let changed = run_jj(
        directory.path(),
        &["git", "remote", "set-url", "origin", missing_remote],
    );
    assert!(changed.status.success(), "set unreachable remote failed");

    let partial = assert_error(
        run_axi(directory.path(), &["finish", "@", "--bookmark", "main"]),
        "finish_partial",
    );
    assert!(partial.contains("bookmark: main"));
    assert!(partial.contains("remote: origin"));
    assert!(partial.contains("description_action: unchanged"));
    assert!(partial.contains("local_action: created"));
    assert!(partial.contains("remote_state: unknown"));
    assert!(partial.contains("reason: transport_or_authentication"));
    assert!(
        !show_ref(remote.path(), "main"),
        "unreachable push cannot update remote"
    );
    let local = run_jj(directory.path(), &["bookmark", "list"]);
    assert!(local.status.success());
    assert!(String::from_utf8_lossy(&local.stdout).contains("main"));
    let restored = remote.path().to_str().expect("remote path is UTF-8");
    let restored = run_jj(
        directory.path(),
        &["git", "remote", "set-url", "origin", restored],
    );
    assert!(restored.status.success(), "restore remote failed");
    let retry = successful_output(directory.path(), &["finish", "@", "--bookmark", "main"]);
    assert!(retry.contains("local_action: unchanged"));
    assert!(retry.contains("remote_action: created"));
    assert!(
        show_ref(remote.path(), "main"),
        "retry must publish after restore"
    );
}

#[test]
fn invalid_bookmark_rewritability_and_readiness_use_structured_errors() {
    let directory = repository();

    let invalid = assert_error(
        run_axi(
            directory.path(),
            &["finish", "@", "--bookmark", "bad/name?"],
        ),
        "invalid_argument",
    );
    assert!(invalid.contains("argument: bookmark"));
    assert!(invalid.contains("constraint: valid_bookmark_name"));

    let root = assert_error(
        run_axi(
            directory.path(),
            &["describe", "root()", "--message", "root message"],
        ),
        "change_not_rewritable",
    );
    assert!(root.contains("reason: root"));

    let root_finish = assert_error(
        run_axi(directory.path(), &["finish", "root()"]),
        "change_not_ready",
    );
    assert!(root_finish.contains("empty_description"));

    let empty = assert_error(
        run_axi(directory.path(), &["finish", "@"]),
        "change_not_ready",
    );
    assert!(empty.contains("reasons[1]: empty_description"));
}
