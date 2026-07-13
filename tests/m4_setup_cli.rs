use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::process::Command;

fn run(cwd: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_jj-axi"))
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap()
}

#[test]
fn skill_prints_exact_canonical_bytes_without_a_repository() {
    let directory = tempfile::tempdir().unwrap();
    let output = run(directory.path(), &["skill"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, include_bytes!("../skills/jj-axi/SKILL.md"));
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn setup_skill_creates_exact_canonical_bytes_and_retries_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("SKILL.md");
    let path = output_path.to_str().unwrap();

    let created = run(directory.path(), &["setup", "skill", "--output", path]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stdout)
    );
    let text = String::from_utf8(created.stdout).unwrap();
    assert!(text.contains("kind: setup_skill"));
    assert!(text.contains("name: jj-axi"));
    assert!(text.contains("action: created"));
    assert!(text.contains("sha256:"));
    assert!(text.contains("bytes:"));
    assert_eq!(
        fs::read(&output_path).unwrap(),
        include_bytes!("../skills/jj-axi/SKILL.md")
    );

    let unchanged = run(directory.path(), &["setup", "skill", "--output", path]);
    assert!(unchanged.status.success());
    assert!(
        String::from_utf8(unchanged.stdout)
            .unwrap()
            .contains("action: unchanged")
    );
}

#[test]
fn setup_skill_rejects_unsafe_destinations_and_missing_parents() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("target.md");
    fs::write(&target, b"target").unwrap();
    let link = directory.path().join("link.md");
    symlink(&target, &link).unwrap();
    let link_output = run(
        directory.path(),
        &[
            "setup",
            "skill",
            "--output",
            link.to_str().unwrap(),
            "--force",
        ],
    );
    assert!(!link_output.status.success());
    assert!(
        String::from_utf8(link_output.stdout)
            .unwrap()
            .contains("code: invalid_skill_output")
    );
    assert_eq!(fs::read(&target).unwrap(), b"target");

    let destination_directory = directory.path().join("skill-dir");
    fs::create_dir(&destination_directory).unwrap();
    let directory_output = run(
        directory.path(),
        &[
            "setup",
            "skill",
            "--output",
            destination_directory.to_str().unwrap(),
        ],
    );
    assert!(!directory_output.status.success());
    assert!(
        String::from_utf8(directory_output.stdout)
            .unwrap()
            .contains("code: invalid_skill_output")
    );

    let missing = directory.path().join("missing").join("SKILL.md");
    let missing_output = run(
        directory.path(),
        &["setup", "skill", "--output", missing.to_str().unwrap()],
    );
    assert!(!missing_output.status.success());
    assert!(!missing.parent().unwrap().exists());
}

#[test]
fn setup_skill_protects_differences_and_force_preserves_permissions() {
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("SKILL.md");
    fs::write(&output_path, b"local edits").unwrap();
    fs::set_permissions(&output_path, fs::Permissions::from_mode(0o640)).unwrap();
    let path = output_path.to_str().unwrap();

    let conflict = run(directory.path(), &["setup", "skill", "--output", path]);
    assert!(!conflict.status.success());
    assert!(
        String::from_utf8(conflict.stdout)
            .unwrap()
            .contains("code: skill_output_conflict")
    );
    assert_eq!(fs::read(&output_path).unwrap(), b"local edits");

    let updated = run(
        directory.path(),
        &["setup", "skill", "--output", path, "--force"],
    );
    assert!(updated.status.success());
    assert!(
        String::from_utf8(updated.stdout)
            .unwrap()
            .contains("action: updated")
    );
    assert_eq!(
        fs::metadata(&output_path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(
        fs::read(&output_path).unwrap(),
        include_bytes!("../skills/jj-axi/SKILL.md")
    );
}
