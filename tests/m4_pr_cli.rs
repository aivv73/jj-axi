mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;

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
            "#!/bin/sh\n[ \"$GH_PROMPT_DISABLED\" = 1 ] || exit 91\n/bin/cat '{}'\n",
            response.display()
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(script, permissions).unwrap();
    directory
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
