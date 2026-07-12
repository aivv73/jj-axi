#![allow(dead_code)]

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const USER_NAME: &str = "jj-axi test";
const USER_EMAIL: &str = "jj-axi@example.test";

pub fn repository() -> TempDir {
    let directory = tempfile::tempdir().expect("create fixture directory");
    let output = Command::new("jj")
        .args(["git", "init", "."])
        .current_dir(directory.path())
        .env("JJ_USER", USER_NAME)
        .env("JJ_EMAIL", USER_EMAIL)
        .output()
        .expect("run jj git init");
    assert!(
        output.status.success(),
        "jj git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config = directory.path().join(".jj").join("jj-axi-test-config.toml");
    fs::write(
        config,
        format!("user.name = '{USER_NAME}'\nuser.email = '{USER_EMAIL}'\n"),
    )
    .expect("write jj config");
    directory
}

pub fn run_axi(directory: &Path, args: &[&str]) -> Output {
    let config = directory.join(".jj").join("jj-axi-test-config.toml");
    Command::new(env!("CARGO_BIN_EXE_jj-axi"))
        .args(args)
        .current_dir(directory)
        .env("JJ_CONFIG", config)
        .env("JJ_USER", USER_NAME)
        .env("JJ_EMAIL", USER_EMAIL)
        .output()
        .expect("run jj-axi")
}

pub fn run_axi_with_stdin(directory: &Path, args: &[&str], input: &[u8]) -> Output {
    let config = directory.join(".jj").join("jj-axi-test-config.toml");
    let mut child = Command::new(env!("CARGO_BIN_EXE_jj-axi"))
        .args(args)
        .current_dir(directory)
        .env("JJ_CONFIG", config)
        .env("JJ_USER", USER_NAME)
        .env("JJ_EMAIL", USER_EMAIL)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run jj-axi with stdin");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(input)
        .expect("write child stdin");
    child.wait_with_output().expect("wait for jj-axi")
}

pub fn successful_output(directory: &Path, args: &[&str]) -> String {
    let output = run_axi(directory, args);
    assert!(
        output.status.success(),
        "jj-axi {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "successful commands write no stderr"
    );
    assert!(
        !output.stdout.ends_with(b"\n"),
        "TOON document must not have a final newline"
    );
    String::from_utf8(output.stdout).expect("TOON output is UTF-8")
}

pub fn run_jj(directory: &Path, args: &[&str]) -> Output {
    Command::new("jj")
        .args(args)
        .current_dir(directory)
        .env("JJ_USER", USER_NAME)
        .env("JJ_EMAIL", USER_EMAIL)
        .output()
        .expect("run jj")
}

pub fn assert_error(output: Output, code: &str) -> String {
    assert!(!output.status.success(), "command unexpectedly succeeded");
    assert!(output.stderr.is_empty(), "errors write to stdout only");
    assert!(!output.stdout.ends_with(b"\n"), "TOON has no final newline");
    let text = String::from_utf8(output.stdout).expect("error output is UTF-8");
    assert!(
        text.contains("kind: error"),
        "missing error envelope: {text}"
    );
    assert!(
        text.contains(&format!("code: {code}")),
        "missing {code}: {text}"
    );
    text
}

pub fn jj_template(directory: &Path, revision: &str, template: &str) -> String {
    let output = run_jj(directory, &["show", revision, "-T", template]);
    assert!(
        output.status.success(),
        "jj show {:?} failed: {}",
        revision,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "jj show writes no stderr");
    String::from_utf8(output.stdout)
        .expect("jj template output is UTF-8")
        .trim_end()
        .to_owned()
}

pub fn commit_id(directory: &Path, revision: &str) -> String {
    jj_template(directory, revision, "commit_id")
}
