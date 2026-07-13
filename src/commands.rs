use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use crate::cli::{CommandInput, ParsedCli};
use crate::error::AppError;
use crate::github_bridge;
use crate::jj_bridge::JjBridge;
use crate::model::{Response, ResponseData, ResponseKind};
use crate::partition;
use crate::setup;
use crate::toon::{ToonValue, render};

pub(crate) async fn run(parsed: ParsedCli, cwd: &Path) -> ExitCode {
    match parsed {
        ParsedCli::Help(error) => match error.print() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        },
        ParsedCli::InvalidArgument {
            argument,
            constraint,
        } => emit_error(AppError::InvalidArgument {
            argument,
            constraint,
        }),
        ParsedCli::Command(CommandInput::Bootstrap) => emit_bytes(setup::BOOTSTRAP_BYTES),
        ParsedCli::Command(CommandInput::Skill {
            full: false,
            output: None,
            force: false,
        }) => emit_bytes(setup::SKILL_BYTES),
        ParsedCli::Command(CommandInput::Skill {
            full: true,
            output: None,
            force: false,
        }) => emit_bytes(setup::AGENT_REFERENCE_BYTES),
        ParsedCli::Command(command) => match execute(command, cwd).await {
            Ok(response) => emit_success(response),
            Err(error) => emit_error(error),
        },
    }
}

async fn execute(command: CommandInput, cwd: &Path) -> Result<Response, AppError> {
    if let CommandInput::Skill {
        full: false,
        output: Some(output),
        force,
    } = &command
    {
        return Ok(Response {
            kind: ResponseKind::SetupSkill,
            data: ResponseData::SetupSkill(setup::setup_skill(output, *force)?),
        });
    }
    if let CommandInput::PrStatus { number, repository } = &command {
        let remote_urls = if repository.is_none() {
            JjBridge::git_remote_urls(cwd).await?
        } else {
            Vec::new()
        };
        return Ok(Response {
            kind: ResponseKind::PrStatus,
            data: ResponseData::PrStatus(github_bridge::pr_status(
                cwd,
                *number,
                repository.as_deref(),
                &remote_urls,
            )?),
        });
    }
    if let CommandInput::Operations { limit } = command {
        return Ok(Response {
            kind: ResponseKind::Operations,
            data: ResponseData::Operations(JjBridge::operations(cwd, limit).await?),
        });
    }
    if let CommandInput::BookmarkList { limit, after, name } = &command {
        return Ok(Response {
            kind: ResponseKind::BookmarkList,
            data: ResponseData::BookmarkList(
                JjBridge::bookmark_list(cwd, *limit, after.as_deref(), name.as_deref()).await?,
            ),
        });
    }
    if let CommandInput::Partition {
        change,
        spec_file,
        dry_run,
        details,
    } = &command
    {
        let loaded = partition::load(spec_file, cwd)?;
        let mut bridge = JjBridge::open(cwd).await?;
        let data = if *dry_run {
            bridge.preview_partition(change, &loaded, *details).await?
        } else {
            bridge.apply_partition(change, &loaded, *details).await?
        };
        return Ok(Response {
            kind: ResponseKind::Partition,
            data: ResponseData::Partition(data),
        });
    }
    let undo_source_ids = if matches!(&command, CommandInput::Undo { .. }) {
        let operation_data = JjBridge::operations(cwd, usize::MAX).await?;
        let mut operation_ids: Vec<_> = operation_data
            .operations
            .into_iter()
            .filter(|operation| operation.current)
            .map(|operation| operation.operation_id)
            .collect();
        operation_ids.sort();
        if matches!(&command, CommandInput::Undo { to: None }) && operation_ids.len() > 1 {
            return Err(AppError::OperationHistoryDiverged { operation_ids });
        }
        Some(operation_ids)
    } else {
        None
    };

    let mut bridge = JjBridge::open(cwd).await?;
    match command {
        CommandInput::New { message } => Ok(Response {
            kind: ResponseKind::New,
            data: ResponseData::New(bridge.create_change(message.as_deref()).await?),
        }),
        CommandInput::Describe { change, message } => Ok(Response {
            kind: ResponseKind::Describe,
            data: ResponseData::Describe(bridge.describe_change(&change, &message).await?),
        }),
        CommandInput::Checkpoint { message } => Ok(Response {
            kind: ResponseKind::Checkpoint,
            data: ResponseData::Checkpoint(bridge.checkpoint(&message).await?),
        }),
        CommandInput::Finish {
            change,
            message,
            bookmark,
        } => Ok(Response {
            kind: ResponseKind::Finish,
            data: ResponseData::Finish(
                bridge
                    .finish_change(&change, message.as_deref(), bookmark.as_deref())
                    .await?,
            ),
        }),
        CommandInput::Inspect => Ok(Response {
            kind: ResponseKind::Inspect,
            data: ResponseData::Inspect(bridge.inspect().await?),
        }),
        CommandInput::Log {
            limit,
            conflicted,
            fields,
        } => Ok(Response {
            kind: ResponseKind::Log,
            data: ResponseData::Log(bridge.log(limit, conflicted, &fields).await?),
        }),
        CommandInput::Show { change, full } => Ok(Response {
            kind: ResponseKind::Show,
            data: ResponseData::Show(bridge.show(&change, full).await?),
        }),
        CommandInput::Diff {
            change,
            full,
            hunks,
        } => Ok(Response {
            kind: ResponseKind::Diff,
            data: ResponseData::Diff(bridge.diff(change.as_deref(), full, hunks).await?),
        }),
        CommandInput::Partition { .. } => unreachable!("partition returns before mutable bridge"),
        CommandInput::Split {
            change,
            hunks,
            into,
        } => Ok(Response {
            kind: ResponseKind::Split,
            data: ResponseData::Split(bridge.split_change(&change, &hunks, &into).await?),
        }),
        CommandInput::Move { from, to, hunks } => Ok(Response {
            kind: ResponseKind::Move,
            data: ResponseData::Move(bridge.move_hunks(&from, &to, &hunks).await?),
        }),
        CommandInput::Absorb { dry_run } => Ok(Response {
            kind: ResponseKind::Absorb,
            data: ResponseData::Absorb(bridge.absorb(dry_run).await?),
        }),
        CommandInput::Squash {
            change,
            destination,
            message,
        } => Ok(Response {
            kind: ResponseKind::Squash,
            data: ResponseData::Squash(
                bridge
                    .squash(&change, destination.as_deref(), message.as_deref())
                    .await?,
            ),
        }),
        CommandInput::Abandon { change } => Ok(Response {
            kind: ResponseKind::Abandon,
            data: ResponseData::Abandon(bridge.abandon(&change).await?),
        }),
        CommandInput::Reorder { sequence } => Ok(Response {
            kind: ResponseKind::Reorder,
            data: ResponseData::Reorder(bridge.reorder(&sequence).await?),
        }),
        CommandInput::BookmarkSet {
            name,
            target,
            allow_backwards,
        } => Ok(Response {
            kind: ResponseKind::BookmarkSet,
            data: ResponseData::BookmarkSet(
                bridge.set_bookmark(&name, &target, allow_backwards).await?,
            ),
        }),
        CommandInput::BookmarkPush { name, remote } => Ok(Response {
            kind: ResponseKind::BookmarkPush,
            data: ResponseData::BookmarkPush(bridge.push_bookmark(&name, remote.as_deref()).await?),
        }),
        CommandInput::Undo { to } => Ok(Response {
            kind: ResponseKind::Undo,
            data: ResponseData::Undo(
                bridge
                    .undo(to.as_deref(), undo_source_ids.unwrap_or_default())
                    .await?,
            ),
        }),
        CommandInput::Operations { .. }
        | CommandInput::BookmarkList { .. }
        | CommandInput::PrStatus { .. }
        | CommandInput::Bootstrap
        | CommandInput::Skill { .. } => {
            unreachable!("handled before repository synchronization")
        }
    }
}

fn emit_bytes(bytes: &[u8]) -> ExitCode {
    let mut stdout = std::io::stdout().lock();
    if stdout.write_all(bytes).is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn emit_success(response: Response) -> ExitCode {
    emit(response.to_toon_value(), ExitCode::SUCCESS)
}

fn emit_error(error: AppError) -> ExitCode {
    let envelope = ToonValue::Object(vec![
        ("schema_version", ToonValue::UInt(1)),
        ("kind", ToonValue::String("error".to_owned())),
        ("error", error.to_toon_value()),
    ]);
    emit(envelope, ExitCode::FAILURE)
}

fn emit(value: ToonValue, success: ExitCode) -> ExitCode {
    let output = render(&value);
    let mut stdout = std::io::stdout().lock();
    if stdout.write_all(output.as_bytes()).is_err() {
        return ExitCode::FAILURE;
    }
    success
}
