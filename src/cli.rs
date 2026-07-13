use clap::error::ErrorKind;
use clap::{ArgAction, Parser, Subcommand, ValueEnum};

#[derive(Debug)]
pub enum ParsedCli {
    Help(clap::Error),
    Command(CommandInput),
    InvalidArgument {
        argument: &'static str,
        constraint: &'static str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandInput {
    New {
        message: Option<String>,
    },
    Describe {
        change: String,
        message: String,
    },
    Checkpoint {
        message: String,
    },
    Finish {
        change: String,
        message: Option<String>,
        bookmark: Option<String>,
    },
    Inspect,
    Log {
        limit: usize,
        conflicted: bool,
        fields: Vec<LogField>,
    },
    Show {
        change: String,
        full: bool,
    },
    Diff {
        change: Option<String>,
        full: bool,
        hunks: bool,
    },
    Partition {
        change: String,
        spec_file: String,
        dry_run: bool,
        details: bool,
    },
    Split {
        change: String,
        hunks: Vec<HunkSpec>,
        into: String,
    },
    Move {
        from: String,
        to: String,
        hunks: Vec<HunkSpec>,
    },
    Absorb {
        dry_run: bool,
    },
    Reorder {
        sequence: Vec<String>,
    },
    Operations {
        limit: usize,
    },
    Undo {
        to: Option<String>,
    },
    BookmarkList {
        limit: usize,
        after: Option<String>,
        name: Option<String>,
    },
    BookmarkSet {
        name: String,
        target: String,
        allow_backwards: bool,
    },
    BookmarkPush {
        name: String,
        remote: Option<String>,
    },
    PrStatus {
        number: u64,
        repository: Option<String>,
    },
    Skill,
    SetupSkill {
        output: String,
        force: bool,
    },
    Squash {
        change: String,
        destination: Option<String>,
        message: Option<String>,
    },
    Abandon {
        change: String,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct HunkSpec {
    pub path: String,
    pub lines: HunkRange,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum HunkRange {
    Lines { start: u32, end: u32 },
    Deletion { at: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum LogField {
    #[value(name = "commit_id")]
    CommitId,
    #[value(name = "parent_commit_ids")]
    ParentCommitIds,
}

#[derive(Parser)]
#[command(disable_help_subcommand = true, version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    New {
        #[arg(long)]
        message: Option<String>,
    },
    Describe {
        change: String,
        #[arg(long)]
        message: String,
    },
    Checkpoint {
        #[arg(long)]
        message: String,
    },
    #[command(
        long_about = "Finish a change.\n\nSteps:\n  Description  Runs when --message is supplied; otherwise the stored description must be non-empty.\n  Readiness    Always; validates every change that publication would introduce.\n  Bookmark     Runs when --bookmark is supplied; creates or fast-forwards that exact name.\n  Push         Runs when --bookmark is supplied; pushes only that name.\n\nDescription and bookmark updates are one local operation committed before push.\nA push failure keeps that local desired state and returns a structured partial result.\nThe push remote is git.push, otherwise the sole configured remote, otherwise origin.\nWith no bookmark, finish is a readiness-only success after applying an optional message; it creates no private \"finished\" metadata.\nWith a bookmark, do not generate or infer a name and do not infer a remote from tracking."
    )]
    Finish {
        change: String,
        #[arg(long)]
        message: Option<String>,
        #[arg(long)]
        bookmark: Option<String>,
    },
    Inspect,
    Log {
        #[arg(long, default_value_t = 20, value_parser = parse_limit)]
        limit: usize,
        #[arg(long)]
        conflicted: bool,
        #[arg(long, value_delimiter = ',', action = ArgAction::Append)]
        fields: Vec<LogField>,
    },
    Show {
        change: String,
        #[arg(long)]
        full: bool,
    },
    Diff {
        change: Option<String>,
        #[arg(long)]
        full: bool,
        #[arg(long)]
        hunks: bool,
    },
    Partition {
        change: String,
        #[arg(long = "spec-file")]
        spec_file: String,
        #[arg(long = "dry-run")]
        dry_run: bool,
        #[arg(long)]
        details: bool,
    },
    Split {
        change: String,
        #[arg(long)]
        hunks: String,
        #[arg(long = "into")]
        into: String,
    },
    Move {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        hunks: String,
    },
    Absorb {
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
    Reorder {
        #[arg(long)]
        sequence: String,
    },
    Squash {
        change: String,
        #[arg(long = "into")]
        destination: Option<String>,
        #[arg(long)]
        message: Option<String>,
    },
    Abandon {
        change: String,
    },
    Operations {
        #[arg(long, default_value_t = 20, value_parser = parse_limit)]
        limit: usize,
    },
    Undo {
        #[arg(long)]
        to: Option<String>,
    },
    Bookmark {
        #[command(subcommand)]
        command: BookmarkCommand,
    },
    Pr {
        #[command(subcommand)]
        command: PrCommand,
    },
    Skill,
    Setup {
        #[command(subcommand)]
        command: SetupCommand,
    },
}

#[derive(Subcommand)]
enum SetupCommand {
    Skill {
        #[arg(long)]
        output: String,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum PrCommand {
    Status {
        #[arg(value_parser = parse_positive_u64)]
        number: u64,
        #[arg(long = "repo")]
        repository: Option<String>,
    },
}

#[derive(Subcommand)]
enum BookmarkCommand {
    List {
        #[arg(long, default_value_t = 100, value_parser = parse_limit)]
        limit: usize,
        #[arg(long)]
        after: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    Set {
        name: String,
        #[arg(long = "to")]
        target: String,
        #[arg(long = "allow-backwards")]
        allow_backwards: bool,
    },
    Push {
        name: String,
        #[arg(long)]
        remote: Option<String>,
    },
}

pub fn parse<I, T>(args: I) -> ParsedCli
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    match Cli::try_parse_from(args) {
        Ok(Cli { command: None }) => ParsedCli::Command(CommandInput::Inspect),
        Ok(Cli {
            command: Some(command),
        }) => ParsedCli::Command(match command {
            Command::New { message } => CommandInput::New { message },
            Command::Describe { change, message } => CommandInput::Describe { change, message },
            Command::Checkpoint { message } => CommandInput::Checkpoint { message },
            Command::Finish {
                change,
                message,
                bookmark,
            } => CommandInput::Finish {
                change,
                message,
                bookmark,
            },
            Command::Inspect => CommandInput::Inspect,
            Command::Log {
                limit,
                conflicted,
                fields,
            } => CommandInput::Log {
                limit,
                conflicted,
                fields,
            },
            Command::Show { change, full } => CommandInput::Show { change, full },
            Command::Diff {
                change,
                full,
                hunks,
            } => CommandInput::Diff {
                change,
                full,
                hunks,
            },
            Command::Partition {
                change,
                spec_file,
                dry_run,
                details,
            } => CommandInput::Partition {
                change,
                spec_file,
                dry_run,
                details,
            },
            Command::Split {
                change,
                hunks,
                into,
            } => match parse_hunk_specs(&hunks) {
                Ok(hunks) => CommandInput::Split {
                    change,
                    hunks,
                    into,
                },
                Err(()) => {
                    return ParsedCli::InvalidArgument {
                        argument: "hunks",
                        constraint: "post_image_hunk_spec",
                    };
                }
            },
            Command::Move { from, to, hunks } => match parse_hunk_specs(&hunks) {
                Ok(hunks) => CommandInput::Move { from, to, hunks },
                Err(()) => {
                    return ParsedCli::InvalidArgument {
                        argument: "hunks",
                        constraint: "post_image_hunk_spec",
                    };
                }
            },
            Command::Absorb { dry_run } => CommandInput::Absorb { dry_run },
            Command::Squash {
                change,
                destination,
                message,
            } => CommandInput::Squash {
                change,
                destination,
                message,
            },
            Command::Abandon { change } => CommandInput::Abandon { change },
            Command::Reorder { sequence } => match parse_reorder_sequence(&sequence) {
                Ok(sequence) => CommandInput::Reorder { sequence },
                Err(()) => {
                    return ParsedCli::InvalidArgument {
                        argument: "sequence",
                        constraint: "at_least_two_revisions_oldest_to_newest",
                    };
                }
            },
            Command::Operations { limit } => CommandInput::Operations { limit },
            Command::Undo { to } => CommandInput::Undo { to },
            Command::Bookmark {
                command: BookmarkCommand::List { limit, after, name },
            } => {
                if after.is_some() && name.is_some() {
                    return ParsedCli::InvalidArgument {
                        argument: "after",
                        constraint: "cannot_combine_with_name",
                    };
                }
                CommandInput::BookmarkList { limit, after, name }
            }
            Command::Bookmark {
                command:
                    BookmarkCommand::Set {
                        name,
                        target,
                        allow_backwards,
                    },
            } => CommandInput::BookmarkSet {
                name,
                target,
                allow_backwards,
            },
            Command::Bookmark {
                command: BookmarkCommand::Push { name, remote },
            } => CommandInput::BookmarkPush { name, remote },
            Command::Pr {
                command: PrCommand::Status { number, repository },
            } => CommandInput::PrStatus { number, repository },
            Command::Skill => CommandInput::Skill,
            Command::Setup {
                command: SetupCommand::Skill { output, force },
            } => CommandInput::SetupSkill { output, force },
        }),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            ParsedCli::Help(error)
        }
        Err(_) => ParsedCli::InvalidArgument {
            argument: "command_line",
            constraint: "valid_command_syntax",
        },
    }
}

fn parse_positive_u64(value: &str) -> Result<u64, String> {
    let number = value
        .parse::<u64>()
        .map_err(|_| "must be a positive integer".to_owned())?;
    if number == 0 {
        return Err("must be greater than zero".to_owned());
    }
    Ok(number)
}

fn parse_limit(value: &str) -> Result<usize, String> {
    let limit = value
        .parse::<usize>()
        .map_err(|_| "must be a positive integer".to_owned())?;

    if limit == 0 {
        return Err("must be greater than zero".to_owned());
    }

    Ok(limit)
}

fn parse_hunk_specs(value: &str) -> Result<Vec<HunkSpec>, ()> {
    let entries = split_hunk_entries(value)?;
    let mut hunks = Vec::with_capacity(entries.len());

    for entry in entries {
        let separator = last_unescaped_colon(entry)?;
        let path = decode_hunk_component(&entry[..separator])?;
        let range = decode_hunk_component(&entry[separator + 1..])?;
        if !valid_hunk_path(&path) {
            return Err(());
        }

        let lines = parse_hunk_range(&range)?;
        let spec = HunkSpec { path, lines };
        if hunks.contains(&spec) {
            return Err(());
        }
        hunks.push(spec);
    }

    Ok(hunks)
}

fn split_hunk_entries(value: &str) -> Result<Vec<&str>, ()> {
    if value.is_empty() {
        return Err(());
    }

    let mut entries = Vec::new();
    let mut entry_start = 0;
    let mut chars = value.char_indices();
    while let Some((index, character)) = chars.next() {
        match character {
            '\\' => match chars.next() {
                Some((_, '\\' | ',' | ':')) => {}
                _ => return Err(()),
            },
            ',' => {
                if index == entry_start {
                    return Err(());
                }
                entries.push(&value[entry_start..index]);
                entry_start = index + character.len_utf8();
            }
            _ => {}
        }
    }

    if entry_start == value.len() {
        return Err(());
    }
    entries.push(&value[entry_start..]);
    Ok(entries)
}

fn last_unescaped_colon(value: &str) -> Result<usize, ()> {
    let mut last = None;
    let mut chars = value.char_indices();
    while let Some((index, character)) = chars.next() {
        match character {
            '\\' => match chars.next() {
                Some((_, '\\' | ',' | ':')) => {}
                _ => return Err(()),
            },
            ':' => last = Some(index),
            _ => {}
        }
    }
    last.ok_or(())
}

fn decode_hunk_component(value: &str) -> Result<String, ()> {
    let mut decoded = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character == '\\' {
            match chars.next() {
                Some(escaped) if matches!(escaped, '\\' | ',' | ':') => {
                    decoded.push(escaped);
                }
                _ => return Err(()),
            }
        } else {
            decoded.push(character);
        }
    }
    Ok(decoded)
}

pub(crate) fn parse_manifest_hunk(path: &str, lines: &str) -> Result<HunkSpec, ()> {
    if !valid_hunk_path(path) {
        return Err(());
    }
    Ok(HunkSpec {
        path: path.to_owned(),
        lines: parse_hunk_range(lines)?,
    })
}

fn valid_hunk_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn parse_hunk_range(value: &str) -> Result<HunkRange, ()> {
    if let Some((start, end)) = value.split_once('-') {
        if end.contains('-') {
            return Err(());
        }

        let start = parse_positive_u32(start)?;
        let end = parse_u32(end)?;
        if end == 0 {
            return Ok(HunkRange::Deletion { at: start });
        }
        if start > end {
            return Err(());
        }
        Ok(HunkRange::Lines { start, end })
    } else {
        let line = parse_positive_u32(value)?;
        Ok(HunkRange::Lines {
            start: line,
            end: line,
        })
    }
}

fn parse_positive_u32(value: &str) -> Result<u32, ()> {
    let value = parse_u32(value)?;
    if value == 0 {
        return Err(());
    }
    Ok(value)
}

fn parse_u32(value: &str) -> Result<u32, ()> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    value.parse::<u32>().map_err(|_| ())
}

fn parse_reorder_sequence(value: &str) -> Result<Vec<String>, ()> {
    let sequence = value
        .split(',')
        .map(|expression| {
            expression
                .trim_matches(|character: char| character.is_ascii_whitespace())
                .to_owned()
        })
        .collect::<Vec<_>>();

    if sequence.len() < 2 || sequence.iter().any(String::is_empty) {
        return Err(());
    }
    Ok(sequence)
}

#[cfg(test)]
mod tests {
    use super::{CommandInput, HunkRange, HunkSpec, ParsedCli, parse};

    fn parse_command(args: &[&str]) -> CommandInput {
        match parse(args.iter().copied()) {
            ParsedCli::Command(command) => command,
            other => panic!("expected command, got {other:?}"),
        }
    }

    #[test]
    fn parses_split_hunks_and_escaped_path_bytes() {
        assert_eq!(
            parse_command(&[
                "jj-axi",
                "split",
                "@",
                "--hunks",
                r"dir/a\,b\:c.txt:2-4,deleted.txt:8-0",
                "--into",
                "description",
            ]),
            CommandInput::Split {
                change: "@".to_owned(),
                hunks: vec![
                    HunkSpec {
                        path: "dir/a,b:c.txt".to_owned(),
                        lines: HunkRange::Lines { start: 2, end: 4 },
                    },
                    HunkSpec {
                        path: "deleted.txt".to_owned(),
                        lines: HunkRange::Deletion { at: 8 },
                    },
                ],
                into: "description".to_owned(),
            }
        );
    }

    #[test]
    fn parses_move_absorb_and_reorder() {
        assert_eq!(
            parse_command(&[
                "jj-axi",
                "move",
                "--from",
                "left",
                "--to",
                "right",
                "--hunks",
                "file.txt:1",
            ]),
            CommandInput::Move {
                from: "left".to_owned(),
                to: "right".to_owned(),
                hunks: vec![HunkSpec {
                    path: "file.txt".to_owned(),
                    lines: HunkRange::Lines { start: 1, end: 1 },
                }],
            }
        );
        assert_eq!(
            parse_command(&["jj-axi", "absorb", "--dry-run"]),
            CommandInput::Absorb { dry_run: true }
        );
        assert_eq!(
            parse_command(&["jj-axi", "reorder", "--sequence", "id1, id2"]),
            CommandInput::Reorder {
                sequence: vec!["id1".to_owned(), "id2".to_owned()],
            }
        );
    }

    #[test]
    fn malformed_m3_values_have_structured_detail() {
        assert_invalid(
            &[
                "jj-axi",
                "split",
                "@",
                "--hunks",
                "file.txt:0",
                "--into",
                "x",
            ],
            "hunks",
            "post_image_hunk_spec",
        );
        assert_invalid(
            &["jj-axi", "reorder", "--sequence", "only"],
            "sequence",
            "at_least_two_revisions_oldest_to_newest",
        );
        assert_invalid(
            &["jj-axi", "split", "@", "--into", "x"],
            "command_line",
            "valid_command_syntax",
        );
    }

    fn assert_invalid(args: &[&str], expected_argument: &str, expected_constraint: &str) {
        match parse(args.iter().copied()) {
            ParsedCli::InvalidArgument {
                argument,
                constraint,
            } => {
                assert_eq!(argument, expected_argument);
                assert_eq!(constraint, expected_constraint);
            }
            other => panic!("expected invalid argument, got {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_hunk_paths_ranges_and_duplicates() {
        for hunks in [
            "",
            "file.txt",
            "/file.txt:1",
            "./file.txt:1",
            "dir//file.txt:1",
            "dir/../file.txt:1",
            "file.txt:0",
            "file.txt:2-1",
            "file.txt:1-2-3",
            "file.txt:1\\q",
            "file.txt:1\\",
            "file.txt:1,file.txt:1-1",
        ] {
            assert!(super::parse_hunk_specs(hunks).is_err(), "{hunks:?}");
        }
    }
}
