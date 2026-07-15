mod common;

use std::fs;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::{Command, Stdio};

use common::repository;
use tempfile::TempDir;

fn fake_gh(json: &str) -> TempDir {
    let directory = tempfile::tempdir().unwrap();
    let script = directory.path().join("gh");
    let response = directory.path().join("response.json");
    fs::write(&response, json).unwrap();
    fs::write(
        &script,
        format!(
            "#!/bin/sh\n[ \"$GH_PROMPT_DISABLED\" = 1 ] || exit 91\nwhile IFS= read -r line || [ -n \"$line\" ]; do printf '%s\\n' \"$line\"; done < '{}'\n",
            response.display()
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(script, permissions).unwrap();
    directory
}

fn failing_gh(message: &str) -> TempDir {
    let directory = tempfile::tempdir().unwrap();
    let script = directory.path().join("gh");
    fs::write(
        &script,
        format!("#!/bin/sh\nprintf '%s' '{}' >&2\nexit 1\n", message),
    )
    .unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(script, permissions).unwrap();
    directory
}

fn paginated_gh(first_page: &str, second_page: &str) -> (TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let script = directory.path().join("gh");
    let calls = directory.path().join("calls");
    let count = directory.path().join("count");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\" >> '{calls}'; done\nprintf '%s\\n' --- >> '{calls}'\nif [ -f '{count}' ]; then IFS= read -r page < '{count}'; else page=0; fi\npage=$((page + 1))\nprintf '%s\\n' \"$page\" > '{count}'\nif [ \"$page\" -eq 1 ]; then printf '%s\\n' '{first_page}'; else printf '%s\\n' '{second_page}'; fi\n",
            calls = calls.display(),
            count = count.display(),
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(script, permissions).unwrap();
    (directory, calls)
}

fn run_with_gh(repo: &Path, gh: &Path, args: &[&str]) -> std::process::Output {
    let config = repo.join(".jj").join("jj-axi-test-config.toml");
    Command::new(env!("CARGO_BIN_EXE_jj-axi"))
        .args(args)
        .current_dir(repo)
        .env("PATH", gh)
        .env("JJ_CONFIG", config)
        .output()
        .unwrap()
}

const READY_JSON: &str = r#"{"data":{"repository":{"pullRequest":{"number":7,"url":"https://github.com/acme/project/pull/7","state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","reviewDecision":"APPROVED","headRefName":"feature","headRefOid":"abc123","baseRefName":"main","commits":{"nodes":[{"commit":{"statusCheckRollup":{"contexts":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}]}}}}}"#;

#[test]
fn explicit_pr_status_derives_readiness_and_check_counts() {
    let repo = repository();
    let gh = fake_gh(
        r#"{"data":{"repository":{"pullRequest":{"number":7,"url":"https://github.com/acme/project/pull/7","state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","reviewDecision":"APPROVED","headRefName":"feature","headRefOid":"abc123","baseRefName":"main","commits":{"nodes":[{"commit":{"statusCheckRollup":{"contexts":{"nodes":[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"},{"__typename":"StatusContext","state":"PENDING"},{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SKIPPED"}],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}]}}}}}"#,
    );

    let output = run_with_gh(
        repo.path(),
        gh.path(),
        &["pr", "status", "7", "--repo", "acme/project"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("kind: pr_status"));
    assert!(text.contains("repository: github.com/acme/project"));
    assert!(text.contains("state: open"));
    assert!(text.contains("passed: 1"));
    assert!(text.contains("pending: 1"));
    assert!(text.contains("skipped: 1"));
    assert!(text.contains("status: pending"));
    assert!(text.contains("ready_to_merge: false"));
    assert!(text.contains("blocking_reasons[1]: checks_pending"));
}

#[test]
fn pr_status_normalizes_auth_failures_without_leaking_stderr() {
    let repo = repository();
    let gh = failing_gh("authentication token secret-value required");
    let output = run_with_gh(
        repo.path(),
        gh.path(),
        &["pr", "status", "7", "--repo", "acme/project"],
    );
    assert!(!output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("code: github_auth_required"));
    assert!(!text.contains("secret-value"));
}

#[test]
fn pr_status_does_not_infer_a_lookalike_github_host() {
    let repo = repository();
    assert!(
        common::run_jj(
            repo.path(),
            &[
                "git",
                "remote",
                "add",
                "origin",
                "https://notgithub.attacker.invalid/acme/project.git",
            ],
        )
        .status
        .success()
    );
    let gh = fake_gh(READY_JSON);
    let output = run_with_gh(repo.path(), gh.path(), &["pr", "status", "7"]);
    assert!(!output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("code: github_repository_not_found"));
}

#[test]
fn pr_status_infers_a_unique_github_remote_identity() {
    let repo = repository();
    assert!(
        common::run_jj(
            repo.path(),
            &[
                "git",
                "remote",
                "add",
                "origin",
                "git@github.com:acme/project.git"
            ],
        )
        .status
        .success()
    );
    let gh = fake_gh(READY_JSON);
    let output = run_with_gh(repo.path(), gh.path(), &["pr", "status", "7"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("repository: github.com/acme/project")
    );
}

#[test]
fn pr_status_rejects_file_shaped_repository_components_before_running_gh() {
    let repo = repository();
    let gh = failing_gh("gh must not run");
    let output = run_with_gh(
        repo.path(),
        gh.path(),
        &["pr", "status", "7", "--repo", "acme/@secret"],
    );

    common::assert_error(output, "invalid_argument");

    assert!(
        common::run_jj(
            repo.path(),
            &[
                "git",
                "remote",
                "add",
                "origin",
                "https://github.com/acme/@secret.git",
            ],
        )
        .status
        .success()
    );
    let inferred = run_with_gh(repo.path(), gh.path(), &["pr", "status", "7"]);
    common::assert_error(inferred, "github_repository_not_found");
}

#[test]
fn pr_status_uses_raw_fields_and_a_deliberate_process_environment() {
    let repo = repository();
    let directory = tempfile::tempdir().unwrap();
    let script = directory.path().join("gh");
    let invocation = directory.path().join("invocation");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\npwd -P > '{invocation}'\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\" >> '{invocation}'; done\nif IFS= read -r unexpected; then exit 92; fi\nprintf '%s\\n' '{READY_JSON}'\n",
            invocation = invocation.display(),
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(script, permissions).unwrap();

    let config = repo.path().join(".jj").join("jj-axi-test-config.toml");
    let mut child = Command::new(env!("CARGO_BIN_EXE_jj-axi"))
        .args(["pr", "status", "7", "--repo", "acme/project"])
        .current_dir(repo.path())
        .env("PATH", directory.path())
        .env("JJ_CONFIG", config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"caller input must not reach gh\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );

    let invocation = fs::read_to_string(invocation).unwrap();
    let child_cwd = invocation.lines().next().unwrap();
    assert_eq!(
        fs::canonicalize(child_cwd).unwrap(),
        fs::canonicalize(repo.path()).unwrap()
    );
    assert!(invocation.contains("\n-f\nowner=acme\n-f\nname=project\n-F\nnumber=7\n"));
}

#[test]
fn pr_status_maps_a_null_pull_request_to_not_found() {
    let repo = repository();
    let gh = fake_gh(r#"{"data":{"repository":{"pullRequest":null}}}"#);
    let output = run_with_gh(
        repo.path(),
        gh.path(),
        &["pr", "status", "7", "--repo", "acme/project"],
    );

    common::assert_error(output, "pull_request_not_found");
}

#[test]
fn pr_status_rejects_checks_from_a_different_head_during_pagination() {
    let repo = repository();
    let first = r#"{"data":{"repository":{"pullRequest":{"number":7,"url":"https://github.com/acme/project/pull/7","state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","reviewDecision":"APPROVED","headRefName":"feature","headRefOid":"first-head","baseRefName":"main","commits":{"nodes":[{"commit":{"statusCheckRollup":{"contexts":{"nodes":[],"pageInfo":{"hasNextPage":true,"endCursor":"@cursor"}}}}}]}}}}}"#;
    let second = r#"{"data":{"repository":{"pullRequest":{"number":7,"url":"https://github.com/acme/project/pull/7","state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","reviewDecision":"APPROVED","headRefName":"feature","headRefOid":"second-head","baseRefName":"main","commits":{"nodes":[{"commit":{"statusCheckRollup":{"contexts":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}]}}}}}"#;
    let (gh, calls) = paginated_gh(first, second);
    let output = run_with_gh(
        repo.path(),
        gh.path(),
        &["pr", "status", "7", "--repo", "acme/project"],
    );

    common::assert_error(output, "github_response_invalid");
    let calls = fs::read_to_string(calls).unwrap();
    assert!(calls.contains("\n-f\nafter=@cursor\n"));
    assert!(!calls.contains("\n-F\nafter=@cursor\n"));
}
