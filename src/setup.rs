use std::fs;
use std::io::Write as _;
use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::error::AppError;
use crate::model::SetupSkillData;

pub(crate) const SKILL_BYTES: &[u8] = include_bytes!("../skills/jj-axi/SKILL.md");

pub(crate) fn setup_skill(output: &str, force: bool) -> Result<SetupSkillData, AppError> {
    let requested = Path::new(output);
    let parent = requested
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent).map_err(|_| AppError::InvalidSkillOutput {
        path: output.to_owned(),
        reason: "parent_unavailable",
    })?;
    let file_name = requested
        .file_name()
        .ok_or_else(|| AppError::InvalidSkillOutput {
            path: output.to_owned(),
            reason: "missing_file_name",
        })?;
    let destination = parent.join(file_name);
    let action = if destination.exists() {
        let current = fs::read(&destination).map_err(|_| AppError::InvalidSkillOutput {
            path: destination.display().to_string(),
            reason: "unreadable",
        })?;
        if current == SKILL_BYTES {
            "unchanged"
        } else if force {
            let permissions = fs::metadata(&destination)
                .map_err(|_| AppError::InvalidSkillOutput {
                    path: destination.display().to_string(),
                    reason: "unreadable",
                })?
                .permissions();
            atomic_write(&destination, Some(permissions))?;
            "updated"
        } else {
            return Err(AppError::SkillOutputConflict {
                path: destination.display().to_string(),
            });
        }
    } else {
        atomic_write(&destination, None)?;
        "created"
    };
    let sha256 = format!("{:x}", Sha256::digest(SKILL_BYTES));
    Ok(SetupSkillData {
        output_path: destination.display().to_string(),
        sha256,
        bytes: SKILL_BYTES.len() as u64,
        action: action.to_owned(),
    })
}

fn atomic_write(destination: &Path, permissions: Option<fs::Permissions>) -> Result<(), AppError> {
    let parent = destination.parent().ok_or(AppError::Internal)?;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(AppError::Internal)?;
    for suffix in 0..100_u32 {
        let temporary = parent.join(format!(
            ".{file_name}.jj-axi-{}-{suffix}",
            std::process::id()
        ));
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => {
                return Err(AppError::BackendFailure {
                    operation: "setup_skill",
                });
            }
        };
        let result = (|| {
            file.write_all(SKILL_BYTES)?;
            file.sync_all()?;
            if let Some(permissions) = permissions {
                file.set_permissions(permissions)?;
            }
            drop(file);
            fs::rename(&temporary, destination)?;
            Ok::<(), std::io::Error>(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(AppError::BackendFailure {
                operation: "setup_skill",
            });
        }
        return Ok(());
    }
    Err(AppError::BackendFailure {
        operation: "setup_skill",
    })
}
