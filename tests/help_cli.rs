use std::process::Command;

fn help(arguments: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_jj-axi"))
        .args(arguments)
        .output()
        .expect("run jj-axi help");
    assert!(
        output.status.success(),
        "help {:?} failed: stdout={} stderr={}",
        arguments,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout)
        .expect("help is UTF-8")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn history_command_help_contains_version_matched_workflows_and_safety_contracts() {
    let cases: &[(&[&str], &[&str])] = &[
        (
            &["diff", "--help"],
            &["canonical post-image", "Do not derive line ranges"],
        ),
        (
            &["split", "--help"],
            &["diff <change> --hunks", "fail before history mutation"],
        ),
        (
            &["move", "--help"],
            &["both source and destination", "Selectors never snap"],
        ),
        (
            &["partition", "--help"],
            &[
                "source_commit_id",
                "require_empty",
                "--dry-run --details",
                "one operation and one undo",
            ],
        ),
        (
            &["absorb", "--help"],
            &["Always preview first", "ambiguous insertions"],
        ),
        (
            &["reorder", "--help"],
            &["oldest to newest", "fails rather than guessing"],
        ),
        (
            &["squash", "--help"],
            &["full-content squash", "Use move for selected hunks"],
        ),
        (
            &["abandon", "--help"],
            &["reparent its descendants", "does not reverse pushes"],
        ),
        (
            &["operations", "--help"],
            &["without mutating", "foundation operations"],
        ),
        (
            &["undo", "--help"],
            &["Divergent operation history", "does not reverse pushes"],
        ),
        (
            &["finish", "--help"],
            &[
                "validates local readiness",
                "structured partial result",
                "remote publication",
                "bookmark set",
            ],
        ),
        (
            &["bookmark", "push", "--help"],
            &["exact supplied name", "inspect it before retrying"],
        ),
    ];

    for (arguments, expected_fragments) in cases {
        let text = help(arguments);
        for fragment in *expected_fragments {
            assert!(
                text.contains(fragment),
                "help {:?} missing {fragment:?}:\n{text}",
                arguments
            );
        }
    }
}
