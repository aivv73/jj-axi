use std::fs;
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
            fs::write(&destination, SKILL_BYTES).map_err(|_| AppError::BackendFailure {
                operation: "setup_skill",
            })?;
            "updated"
        } else {
            return Err(AppError::SkillOutputConflict {
                path: destination.display().to_string(),
            });
        }
    } else {
        fs::write(&destination, SKILL_BYTES).map_err(|_| AppError::BackendFailure {
            operation: "setup_skill",
        })?;
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
