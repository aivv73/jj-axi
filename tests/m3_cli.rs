mod common;

use common::{assert_error, repository, run_axi, run_jj, successful_output};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Output;

fn jj_ok(directory: &Path, args: &[&str]) -> String {
    let output = run_jj(directory, args);
    assert!(
        output.status.success(),
        "jj {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "jj {:?} wrote stderr: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("jj output is UTF-8")
}

fn change_id(directory: &Path, description: &str) -> String {
    let value = jj_ok(
        directory,
        &[
            "log",
            "--no-graph",
            "-r",
            &format!("description(substring:\"{description}\")"),
            "-T",
            "change_id",
        ],
    )
    .trim()
    .to_owned();
    value.chars().take(32).collect()
}

fn write(directory: &Path, name: &str, content: &[u8]) {
    fs::write(directory.join(name), content).expect("write fixture file");
}

fn changed_lines(patch: &str) -> Vec<&str> {
    patch
        .lines()
        .filter(|line| {
            (line.starts_with('-') || line.starts_with('+'))
                && !line.starts_with("---")
                && !line.starts_with("+++")
        })
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
struct Snapshot {
    operation: String,
    commits: String,
    bookmarks: String,
    diff: String,
    files: Vec<(PathBuf, Vec<u8>)>,
}

fn snapshot(directory: &Path, files: &[&str]) -> Snapshot {
    let status = run_jj(directory, &["status"]);
    assert!(
        status.status.success(),
        "jj status failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    Snapshot {
        operation: jj_ok(
            directory,
            &["op", "log", "--no-graph", "-n", "1", "-T", "self.id()"],
        ),
        commits: jj_ok(
            directory,
            &[
                "log",
                "--no-graph",
                "-r",
                "all()",
                "-T",
                "commit_id ++ \"\\n\"",
            ],
        ),
        bookmarks: jj_ok(directory, &["bookmark", "list"]),
        diff: jj_ok(directory, &["diff", "--git"]),
        files: files
            .iter()
            .map(|file| {
                let path = directory.join(file);
                (path.clone(), fs::read(&path).unwrap_or_default())
            })
            .collect(),
    }
}

fn assert_error_clean(output: Output, code: &str) -> String {
    assert_error(output, code)
}

#[test]
fn split_and_move_route_post_image_hunks() {
    let directory = repository();
    write(
        directory.path(),
        "sample.txt",
        b"one\ntwo\nthree\nfour\nfive\n",
    );
    assert!(
        run_jj(directory.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(directory.path(), &["new", "-m", "mixed"])
            .status
            .success()
    );
    write(
        directory.path(),
        "sample.txt",
        b"one\nTWO\nthree\nfour\nFIVE\n",
    );
    assert!(
        run_jj(directory.path(), &["bookmark", "create", "keep"])
            .status
            .success()
    );
    let original_id = jj_ok(
        directory.path(),
        &["log", "--no-graph", "-r", "@", "-T", "change_id"],
    )
    .trim()
    .to_owned();

    let split = successful_output(
        directory.path(),
        &[
            "split",
            "@",
            "--hunks",
            "sample.txt:2",
            "--into",
            "selected edit",
        ],
    );
    assert!(split.starts_with("schema_version: 1\nkind: split\n"));
    assert!(split.contains("description: \"selected edit\\n\""));
    assert!(split.contains("description: \"mixed\\n\""));
    assert!(split.contains("lines: \"2\""));
    assert_eq!(change_id(directory.path(), "selected edit"), original_id);
    assert_eq!(
        jj_ok(
            directory.path(),
            &["log", "--no-graph", "-r", "keep", "-T", "change_id"]
        )
        .trim(),
        change_id(directory.path(), "selected edit")
    );

    let selected_patch = jj_ok(directory.path(), &["diff", "-r", "@-", "--git"]);
    let remaining_patch = jj_ok(directory.path(), &["diff", "-r", "@", "--git"]);
    assert_eq!(changed_lines(&selected_patch), vec!["-two", "+TWO"]);
    assert_eq!(changed_lines(&remaining_patch), vec!["-five", "+FIVE"]);

    let source_id = change_id(directory.path(), "mixed");
    let moved = successful_output(
        directory.path(),
        &[
            "move",
            "--from",
            "@",
            "--to",
            "@-",
            "--hunks",
            "sample.txt:5",
        ],
    );
    assert!(moved.starts_with("schema_version: 1\nkind: move\n"));
    assert!(moved.contains("lines: \"5\""));
    let destination_patch = jj_ok(directory.path(), &["diff", "-r", "@-", "--git"]);
    assert_eq!(
        changed_lines(&destination_patch),
        vec!["-two", "+TWO", "-five", "+FIVE"]
    );
    assert!(jj_ok(directory.path(), &["diff", "-r", "@", "--summary"]).is_empty());
    assert_eq!(change_id(directory.path(), "mixed"), source_id);
}

#[test]
fn split_supports_full_selection_and_deletion_boundaries() {
    let directory = repository();
    write(directory.path(), "sample.txt", b"one\ntwo\nthree\n");
    assert!(
        run_jj(directory.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(directory.path(), &["new", "-m", "delete"])
            .status
            .success()
    );
    write(directory.path(), "sample.txt", b"one\nthree\n");
    let deletion = successful_output(
        directory.path(),
        &[
            "split",
            "@",
            "--hunks",
            "sample.txt:2-0",
            "--into",
            "deleted",
        ],
    );
    assert!(deletion.contains("kind: split"));
    assert!(deletion.contains("lines: 2-0"));
    assert_eq!(
        changed_lines(&jj_ok(directory.path(), &["diff", "-r", "@-", "--git"])),
        vec!["-two"]
    );
    assert!(jj_ok(directory.path(), &["diff", "-r", "@", "--summary"]).is_empty());

    let full = repository();
    write(full.path(), "all.txt", b"old\n");
    assert!(
        run_jj(full.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(run_jj(full.path(), &["new", "-m", "all"]).status.success());
    write(full.path(), "all.txt", b"new\n");
    let output = successful_output(
        full.path(),
        &[
            "split",
            "@",
            "--hunks",
            "all.txt:1",
            "--into",
            "all selected",
        ],
    );
    assert!(output.contains("description: \"all selected\\n\""));
    assert!(jj_ok(full.path(), &["diff", "-r", "@", "--summary"]).is_empty());
}

#[test]
fn move_supports_ancestor_descendant_and_unrelated_destinations() {
    let ancestor = repository();
    write(ancestor.path(), "base.txt", b"base\n");
    assert!(
        run_jj(ancestor.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(ancestor.path(), &["new", "-m", "source"])
            .status
            .success()
    );
    write(ancestor.path(), "source.txt", b"source\n");
    let source = change_id(ancestor.path(), "source");
    assert!(
        run_jj(ancestor.path(), &["new", "-m", "destination"])
            .status
            .success()
    );
    write(ancestor.path(), "destination.txt", b"destination\n");
    let destination = change_id(ancestor.path(), "destination");
    successful_output(
        ancestor.path(),
        &[
            "move",
            "--from",
            &source,
            "--to",
            &destination,
            "--hunks",
            "source.txt:1",
        ],
    );
    assert!(jj_ok(ancestor.path(), &["diff", "-r", &source, "--summary"]).is_empty());
    assert!(jj_ok(ancestor.path(), &["diff", "-r", &destination, "--git"]).contains("source.txt"));

    let descendant = repository();
    write(descendant.path(), "base.txt", b"base\n");
    assert!(
        run_jj(descendant.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(descendant.path(), &["new", "-m", "destination"])
            .status
            .success()
    );
    write(descendant.path(), "destination.txt", b"destination\n");
    let destination = change_id(descendant.path(), "destination");
    assert!(
        run_jj(descendant.path(), &["new", "-m", "source"])
            .status
            .success()
    );
    write(descendant.path(), "source.txt", b"source\n");
    let source = change_id(descendant.path(), "source");
    successful_output(
        descendant.path(),
        &[
            "move",
            "--from",
            &source,
            "--to",
            &destination,
            "--hunks",
            "source.txt:1",
        ],
    );
    assert!(jj_ok(descendant.path(), &["diff", "-r", &source, "--summary"]).is_empty());
    assert!(
        jj_ok(descendant.path(), &["diff", "-r", &destination, "--git"]).contains("source.txt")
    );

    let unrelated = repository();
    write(unrelated.path(), "base.txt", b"base\n");
    assert!(
        run_jj(unrelated.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(unrelated.path(), &["new", "-m", "source"])
            .status
            .success()
    );
    write(unrelated.path(), "source.txt", b"source\n");
    let source = change_id(unrelated.path(), "source");
    assert!(
        run_jj(unrelated.path(), &["new", "root()", "-m", "destination"])
            .status
            .success()
    );
    write(unrelated.path(), "destination.txt", b"destination\n");
    let destination = change_id(unrelated.path(), "destination");
    successful_output(
        unrelated.path(),
        &[
            "move",
            "--from",
            &source,
            "--to",
            &destination,
            "--hunks",
            "source.txt:1",
        ],
    );
    assert!(jj_ok(unrelated.path(), &["diff", "-r", &source, "--summary"]).is_empty());
    assert!(jj_ok(unrelated.path(), &["diff", "-r", &destination, "--git"]).contains("source.txt"));
}

#[test]
fn invalid_hunks_are_structured_and_do_not_mutate_history() {
    let directory = repository();
    write(directory.path(), "sample.txt", b"one\ntwo\nthree\nfour\n");
    assert!(
        run_jj(directory.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(directory.path(), &["new", "-m", "edit"])
            .status
            .success()
    );
    write(directory.path(), "sample.txt", b"one\nTWO\nTHREE\nfour\n");

    let before = snapshot(directory.path(), &["sample.txt"]);
    let stale = assert_error_clean(
        run_axi(
            directory.path(),
            &["split", "@", "--hunks", "sample.txt:1", "--into", "stale"],
        ),
        "invalid_hunk_selection",
    );
    assert!(stale.contains("reason: range_not_hunk"));
    assert!(stale.contains("nearest_hunks"));
    assert_eq!(snapshot(directory.path(), &["sample.txt"]), before);

    let duplicate = assert_error_clean(
        run_axi(
            directory.path(),
            &[
                "split",
                "@",
                "--hunks",
                "sample.txt:2,sample.txt:2-2",
                "--into",
                "duplicate",
            ],
        ),
        "invalid_argument",
    );
    assert!(duplicate.contains("argument: hunks"));
    assert_eq!(snapshot(directory.path(), &["sample.txt"]), before);

    let wrong_deletion = assert_error_clean(
        run_axi(
            directory.path(),
            &[
                "move",
                "--from",
                "@",
                "--to",
                "@-",
                "--hunks",
                "sample.txt:2-0",
            ],
        ),
        "invalid_hunk_selection",
    );
    assert!(wrong_deletion.contains("reason: range_not_hunk"));
    assert_eq!(snapshot(directory.path(), &["sample.txt"]), before);
}

#[cfg(unix)]
#[test]
fn binary_and_metadata_changes_fail_before_history_mutation() {
    let binary = repository();
    write(binary.path(), "base.txt", b"base\n");
    assert!(
        run_jj(binary.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(binary.path(), &["new", "-m", "binary"])
            .status
            .success()
    );
    write(binary.path(), "payload.bin", b"left\0right");
    let binary_before = snapshot(binary.path(), &["payload.bin"]);
    let binary_error = assert_error_clean(
        run_axi(
            binary.path(),
            &["split", "@", "--hunks", "payload.bin:1", "--into", "binary"],
        ),
        "invalid_hunk_selection",
    );
    assert!(binary_error.contains("reason: unsupported_content"));
    assert_eq!(snapshot(binary.path(), &["payload.bin"]), binary_before);

    let metadata = repository();
    write(metadata.path(), "script.sh", b"echo hi\n");
    fs::set_permissions(
        metadata.path().join("script.sh"),
        fs::Permissions::from_mode(0o644),
    )
    .expect("set initial mode");
    assert!(
        run_jj(metadata.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(metadata.path(), &["new", "-m", "metadata"])
            .status
            .success()
    );
    fs::set_permissions(
        metadata.path().join("script.sh"),
        fs::Permissions::from_mode(0o755),
    )
    .expect("set changed mode");
    let metadata_before = snapshot(metadata.path(), &["script.sh"]);
    let metadata_error = assert_error_clean(
        run_axi(
            metadata.path(),
            &["split", "@", "--hunks", "script.sh:1", "--into", "metadata"],
        ),
        "invalid_hunk_selection",
    );
    assert!(metadata_error.contains("reason: metadata_change"));
    assert_eq!(snapshot(metadata.path(), &["script.sh"]), metadata_before);
}

#[test]
fn split_preserves_unselected_conflict_status_in_remaining_change() {
    let directory = repository();
    write(directory.path(), "conflict.txt", b"base\n");
    write(directory.path(), "good.txt", b"base\n");
    assert!(
        run_jj(directory.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(directory.path(), &["new", "-m", "left"])
            .status
            .success()
    );
    write(directory.path(), "conflict.txt", b"LEFT\n");
    let left = change_id(directory.path(), "left");
    assert!(
        run_jj(directory.path(), &["new", "root()", "-m", "right"])
            .status
            .success()
    );
    write(directory.path(), "conflict.txt", b"RIGHT\n");
    let right = change_id(directory.path(), "right");
    assert!(
        run_jj(directory.path(), &["new", &left, &right, "-m", "merge"],)
            .status
            .success()
    );
    write(directory.path(), "good.txt", b"GOOD\n");

    let output = successful_output(
        directory.path(),
        &["split", "@", "--hunks", "good.txt:1", "--into", "selected"],
    );
    assert!(output.contains("selected:"));
    assert!(output.contains("remaining:"));
    assert!(output.contains("remaining:\n    change_id:"));
    let remaining_start = output.find("remaining:").expect("remaining field");
    assert!(output[remaining_start..].contains("conflicted: true"));
}

#[test]
fn absorb_dry_run_matches_apply_plan_and_preserves_state() {
    let directory = repository();
    write(
        directory.path(),
        "sample.txt",
        b"one\ntwo\nthree\nfour\nfive\n",
    );
    assert!(
        run_jj(directory.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(directory.path(), &["new", "-m", "owner"])
            .status
            .success()
    );
    write(
        directory.path(),
        "sample.txt",
        b"one\nOWNER\nthree\nfour\nfive\n",
    );
    assert!(
        run_jj(directory.path(), &["new", "-m", "source"])
            .status
            .success()
    );
    write(
        directory.path(),
        "sample.txt",
        b"one\nSOURCE\nthree\nfour\nfive\n",
    );

    let before = snapshot(directory.path(), &["sample.txt"]);
    let dry = successful_output(directory.path(), &["absorb", "--dry-run"]);
    assert!(dry.contains("kind: absorb"));
    assert!(dry.contains("dry_run: true"));
    assert!(dry.contains("changed: true"));
    let owner_id = change_id(directory.path(), "owner");
    assert!(dry.contains(&format!("destination_change_id: {owner_id}")));
    let after_dry = snapshot(directory.path(), &["sample.txt"]);
    assert_eq!(before, after_dry);

    let apply = successful_output(directory.path(), &["absorb"]);
    assert!(apply.contains("dry_run: false"));
    assert!(apply.contains("source_action: rewritten"));
    let owner_diff = jj_ok(
        directory.path(),
        &["diff", "-r", "description(substring:\"owner\")", "--git"],
    );
    assert_eq!(changed_lines(&owner_diff), vec!["-two", "+SOURCE"]);
    assert!(
        jj_ok(
            directory.path(),
            &[
                "diff",
                "-r",
                "description(substring:\"source\")",
                "--summary",
            ],
        )
        .is_empty()
    );
    assert_eq!(dry.replace("dry_run: true", "dry_run: false"), apply);
}

#[test]
fn absorb_reports_unmoved_new_files() {
    let directory = repository();
    write(directory.path(), "base.txt", b"base\n");
    assert!(
        run_jj(directory.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(directory.path(), &["new", "-m", "source"])
            .status
            .success()
    );
    write(directory.path(), "new.txt", b"new\n");

    let output = successful_output(directory.path(), &["absorb", "--dry-run"]);
    assert!(output.contains("kind: absorb"));
    assert!(output.contains("changed: false"));
    assert!(output.contains("source_action: unchanged"));
    assert!(output.contains("path: new.txt"));
    assert!(output.contains("reason: no_unambiguous_destination"));
}

#[cfg(unix)]
#[test]
fn absorb_reports_stable_symlink_skip_reason() {
    let directory = repository();
    write(directory.path(), "base.txt", b"base\n");
    assert!(
        run_jj(directory.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(directory.path(), &["new", "-m", "source"])
            .status
            .success()
    );
    std::os::unix::fs::symlink("base.txt", directory.path().join("link"))
        .expect("create symlink fixture");

    let output = successful_output(directory.path(), &["absorb", "--dry-run"]);
    assert!(output.contains("path: link"));
    assert!(output.contains("reason: symlink"));
}

#[test]
fn absorb_reports_ambiguous_insertions_and_conflict_skips() {
    let ambiguous = repository();
    write(ambiguous.path(), "f.txt", b"one\ntwo\n");
    assert!(
        run_jj(ambiguous.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(ambiguous.path(), &["new", "-m", "owner-one"])
            .status
            .success()
    );
    write(ambiguous.path(), "f.txt", b"ONE\ntwo\n");
    assert!(
        run_jj(ambiguous.path(), &["new", "-m", "owner-two"])
            .status
            .success()
    );
    write(ambiguous.path(), "f.txt", b"ONE\nTWO\n");
    assert!(
        run_jj(ambiguous.path(), &["new", "-m", "source"])
            .status
            .success()
    );
    write(ambiguous.path(), "f.txt", b"ONE\nINSERT\nTWO\n");
    let ambiguous_output = successful_output(ambiguous.path(), &["absorb", "--dry-run"]);
    assert!(ambiguous_output.contains("changed: false"));
    assert!(ambiguous_output.contains("path: f.txt"));
    assert!(ambiguous_output.contains("lines: \"2\""));
    assert!(ambiguous_output.contains("reason: no_unambiguous_destination"));

    let conflict = repository();
    write(conflict.path(), "f.txt", b"base\n");
    assert!(
        run_jj(conflict.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(conflict.path(), &["new", "-m", "left"])
            .status
            .success()
    );
    write(conflict.path(), "f.txt", b"LEFT\n");
    let left = change_id(conflict.path(), "left");
    assert!(
        run_jj(conflict.path(), &["new", "root()", "-m", "right"])
            .status
            .success()
    );
    write(conflict.path(), "f.txt", b"RIGHT\n");
    let right = change_id(conflict.path(), "right");
    assert!(
        run_jj(conflict.path(), &["new", &left, &right, "-m", "merge"],)
            .status
            .success()
    );
    assert!(
        run_jj(conflict.path(), &["new", "-m", "source"])
            .status
            .success()
    );
    write(conflict.path(), "f.txt", b"RESOLVED\n");
    let conflict_output = successful_output(conflict.path(), &["absorb", "--dry-run"]);
    assert!(conflict_output.contains("changed: false"));
    assert!(conflict_output.contains("path: f.txt"));
    assert!(conflict_output.contains("reason: conflict"));
}

#[test]
fn reorder_is_deterministic_and_idempotent() {
    let directory = repository();
    write(directory.path(), "base.txt", b"base\n");
    assert!(
        run_jj(directory.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(directory.path(), &["new", "-m", "oldest"])
            .status
            .success()
    );
    write(directory.path(), "oldest.txt", b"oldest\n");
    assert!(
        run_jj(directory.path(), &["new", "-m", "middle"])
            .status
            .success()
    );
    write(directory.path(), "middle.txt", b"middle\n");
    assert!(
        run_jj(directory.path(), &["new", "-m", "newest"])
            .status
            .success()
    );
    write(directory.path(), "newest.txt", b"newest\n");

    let oldest = change_id(directory.path(), "oldest");
    let middle = change_id(directory.path(), "middle");
    let newest = change_id(directory.path(), "newest");
    let output = successful_output(
        directory.path(),
        &[
            "reorder",
            "--sequence",
            &format!("{oldest},{newest},{middle}"),
        ],
    );
    assert!(output.starts_with("schema_version: 1\nkind: reorder\n"));
    assert!(output.contains("changed: true"));
    let log = jj_ok(
        directory.path(),
        &[
            "log",
            "--no-graph",
            "-r",
            &format!("{oldest}::{middle}"),
            "-T",
            "description",
        ],
    );
    let descriptions: Vec<_> = log
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::trim)
        .collect();
    assert_eq!(descriptions, vec!["middle", "newest", "oldest"]);

    let idempotent = successful_output(
        directory.path(),
        &[
            "reorder",
            "--sequence",
            &format!("{oldest},{newest},{middle}"),
        ],
    );
    assert!(idempotent.contains("changed: false"));
}

#[test]
fn reorder_reports_conflicts_and_rejects_non_linear_or_merge_shapes() {
    let conflict = repository();
    write(conflict.path(), "shared.txt", b"base\n");
    assert!(
        run_jj(conflict.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(conflict.path(), &["new", "-m", "a"])
            .status
            .success()
    );
    write(conflict.path(), "shared.txt", b"A\n");
    assert!(
        run_jj(conflict.path(), &["new", "-m", "b"])
            .status
            .success()
    );
    write(conflict.path(), "shared.txt", b"B\n");
    let a = change_id(conflict.path(), "a");
    let b = change_id(conflict.path(), "b");
    let reordered = successful_output(
        conflict.path(),
        &["reorder", "--sequence", &format!("{b},{a}")],
    );
    assert!(reordered.contains("kind: reorder"));
    assert!(reordered.contains("status:\n        conflicted: true"));

    let non_linear = repository();
    write(non_linear.path(), "base.txt", b"base\n");
    assert!(
        run_jj(non_linear.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(non_linear.path(), &["new", "-m", "left"])
            .status
            .success()
    );
    write(non_linear.path(), "left.txt", b"left\n");
    let left = change_id(non_linear.path(), "left");
    assert!(
        run_jj(non_linear.path(), &["new", "root()", "-m", "right"])
            .status
            .success()
    );
    write(non_linear.path(), "right.txt", b"right\n");
    let right = change_id(non_linear.path(), "right");
    let non_linear_before = snapshot(non_linear.path(), &["right.txt"]);
    let non_linear_error = assert_error_clean(
        run_axi(
            non_linear.path(),
            &["reorder", "--sequence", &format!("{left},{right}")],
        ),
        "invalid_history_shape",
    );
    assert!(non_linear_error.contains("reason: non_linear"));
    assert_eq!(
        snapshot(non_linear.path(), &["right.txt"]),
        non_linear_before
    );

    let merge = repository();
    write(merge.path(), "base.txt", b"base\n");
    assert!(
        run_jj(merge.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(merge.path(), &["new", "-m", "left"])
            .status
            .success()
    );
    write(merge.path(), "left.txt", b"left\n");
    let left = change_id(merge.path(), "left");
    assert!(
        run_jj(merge.path(), &["new", "root()", "-m", "right"])
            .status
            .success()
    );
    write(merge.path(), "right.txt", b"right\n");
    let right = change_id(merge.path(), "right");
    assert!(
        run_jj(merge.path(), &["new", &left, &right, "-m", "merge"],)
            .status
            .success()
    );
    let merge_id = change_id(merge.path(), "merge");
    let merge_before = snapshot(merge.path(), &["left.txt", "right.txt"]);
    let merge_error = assert_error_clean(
        run_axi(
            merge.path(),
            &["reorder", "--sequence", &format!("{left},{merge_id}")],
        ),
        "invalid_history_shape",
    );
    assert!(merge_error.contains("reason: merge_commit"));
    assert_eq!(
        snapshot(merge.path(), &["left.txt", "right.txt"]),
        merge_before
    );
}

#[test]
fn reorder_rebases_omitted_side_descendants() {
    let directory = repository();
    write(directory.path(), "base.txt", b"base\n");
    assert!(
        run_jj(directory.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(directory.path(), &["new", "-m", "oldest"])
            .status
            .success()
    );
    write(directory.path(), "oldest.txt", b"oldest\n");
    assert!(
        run_jj(directory.path(), &["new", "-m", "middle"])
            .status
            .success()
    );
    write(directory.path(), "middle.txt", b"middle\n");
    let middle = change_id(directory.path(), "middle");
    assert!(
        run_jj(directory.path(), &["new", "-m", "side"])
            .status
            .success()
    );
    write(directory.path(), "side.txt", b"side\n");
    let side = change_id(directory.path(), "side");
    assert!(
        run_jj(directory.path(), &["new", &middle, "-m", "newest"])
            .status
            .success()
    );
    write(directory.path(), "newest.txt", b"newest\n");
    let oldest = change_id(directory.path(), "oldest");
    let newest = change_id(directory.path(), "newest");
    successful_output(
        directory.path(),
        &[
            "reorder",
            "--sequence",
            &format!("{oldest},{newest},{middle}"),
        ],
    );
    let side_range = jj_ok(
        directory.path(),
        &[
            "log",
            "--no-graph",
            "-r",
            &format!("{middle}::{side}"),
            "-T",
            "description",
        ],
    );
    assert!(side_range.contains("middle"));
    assert!(side_range.contains("side"));
    assert!(jj_ok(directory.path(), &["diff", "-r", &side, "--summary"]).contains("side.txt"));
}

#[test]
fn history_shape_errors_and_output_contracts_are_stable() {
    let directory = repository();
    let _root_commit_id = common::commit_id(directory.path(), "root()");
    write(directory.path(), "base.txt", b"base\n");
    assert!(
        run_jj(directory.path(), &["describe", "-m", "base"])
            .status
            .success()
    );
    assert!(
        run_jj(directory.path(), &["new", "-m", "one"])
            .status
            .success()
    );
    write(directory.path(), "one.txt", b"one\n");
    assert!(
        run_jj(directory.path(), &["new", "-m", "two"])
            .status
            .success()
    );
    write(directory.path(), "two.txt", b"two\n");
    assert!(
        run_jj(directory.path(), &["new", "-m", "three"])
            .status
            .success()
    );
    write(directory.path(), "three.txt", b"three\n");
    let one = change_id(directory.path(), "one");
    let two = change_id(directory.path(), "two");
    let three = change_id(directory.path(), "three");

    let before = snapshot(directory.path(), &["three.txt"]);
    let duplicate = assert_error_clean(
        run_axi(
            directory.path(),
            &["reorder", "--sequence", &format!("{one},{one}")],
        ),
        "invalid_history_shape",
    );
    assert!(duplicate.contains("reason: duplicate_change"));
    assert_eq!(snapshot(directory.path(), &["three.txt"]), before);

    let non_contiguous = assert_error_clean(
        run_axi(
            directory.path(),
            &["reorder", "--sequence", &format!("{one},{three}")],
        ),
        "invalid_history_shape",
    );
    assert!(non_contiguous.contains("reason: non_contiguous"));
    assert_eq!(snapshot(directory.path(), &["three.txt"]), before);

    let same_change = assert_error_clean(
        run_axi(
            directory.path(),
            &["move", "--from", &two, "--to", &two, "--hunks", "two.txt:1"],
        ),
        "invalid_history_shape",
    );
    assert!(same_change.contains("reason: same_change"));
    assert_eq!(snapshot(directory.path(), &["three.txt"]), before);

    let immutable = assert_error_clean(
        run_axi(
            directory.path(),
            &[
                "move",
                "--from",
                &two,
                "--to",
                "root()",
                "--hunks",
                "two.txt:1",
            ],
        ),
        "change_not_rewritable",
    );
    assert!(immutable.contains("reason: root"));
    assert_eq!(snapshot(directory.path(), &["three.txt"]), before);

    let root = assert_error_clean(
        run_axi(
            directory.path(),
            &["reorder", "--sequence", &format!("root(),{one}")],
        ),
        "change_not_rewritable",
    );
    assert!(root.contains("reason: root"));
    assert_eq!(snapshot(directory.path(), &["three.txt"]), before);
}
