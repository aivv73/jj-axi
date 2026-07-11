use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::process::Command;

fn run(cwd: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_jj-axi"))
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap()
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
