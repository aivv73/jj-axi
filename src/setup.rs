use std::fs;
use std::io::Write as _;
use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::error::AppError;
use crate::model::{SetupSkillAction, SetupSkillData};

pub(crate) const SKILL_BYTES: &[u8] = include_bytes!("../skills/jj-axi/SKILL.md");
pub(crate) const AGENT_REFERENCE_BYTES: &[u8] = include_bytes!("../docs/agent-reference.md");

pub(crate) fn skill_body() -> &'static [u8] {
    const FRONTMATTER_END: &[u8] = b"\n---\n\n";
    SKILL_BYTES
        .windows(FRONTMATTER_END.len())
        .position(|window| window == FRONTMATTER_END)
        .map_or(SKILL_BYTES, |position| {
            &SKILL_BYTES[position + FRONTMATTER_END.len()..]
        })
}

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
    let action = if let Some(action) = existing_skill_action(&destination, force)? {
        action
    } else {
        match atomic_write(&destination, None)? {
            AtomicWriteOutcome::Published => SetupSkillAction::Created,
            AtomicWriteOutcome::AlreadyExists => existing_skill_action(&destination, false)?
                .ok_or(AppError::BackendFailure {
                    operation: "setup_skill",
                })?,
        }
    };
    let sha256 = format!("{:x}", Sha256::digest(SKILL_BYTES));
    Ok(SetupSkillData {
        output_path: destination.display().to_string(),
        sha256,
        bytes: SKILL_BYTES.len() as u64,
        action,
    })
}

fn existing_skill_action(
    destination: &Path,
    force: bool,
) -> Result<Option<SetupSkillAction>, AppError> {
    let metadata = match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(AppError::InvalidSkillOutput {
                path: destination.display().to_string(),
                reason: "symlink",
            });
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(AppError::InvalidSkillOutput {
                path: destination.display().to_string(),
                reason: "not_regular_file",
            });
        }
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err(AppError::InvalidSkillOutput {
                path: destination.display().to_string(),
                reason: "metadata_unavailable",
            });
        }
    };
    let current = fs::read(destination).map_err(|_| AppError::InvalidSkillOutput {
        path: destination.display().to_string(),
        reason: "unreadable",
    })?;
    if current == SKILL_BYTES {
        Ok(Some(SetupSkillAction::Unchanged))
    } else if force {
        atomic_write(destination, Some(metadata.permissions()))?;
        Ok(Some(SetupSkillAction::Updated))
    } else {
        Err(AppError::SkillOutputConflict {
            path: destination.display().to_string(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AtomicWriteOutcome {
    Published,
    AlreadyExists,
}

fn atomic_write(
    destination: &Path,
    permissions: Option<fs::Permissions>,
) -> Result<AtomicWriteOutcome, AppError> {
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
            if let Some(permissions) = &permissions {
                file.set_permissions(permissions.clone())?;
            }
            drop(file);
            if permissions.is_some() {
                fs::rename(&temporary, destination)?;
            } else {
                fs::hard_link(&temporary, destination)?;
                fs::remove_file(&temporary)?;
            }
            Ok::<(), std::io::Error>(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            if permissions.is_none() && error.kind() == std::io::ErrorKind::AlreadyExists {
                return Ok(AtomicWriteOutcome::AlreadyExists);
            }
            return Err(AppError::BackendFailure {
                operation: "setup_skill",
            });
        }
        return Ok(AtomicWriteOutcome::Published);
    }
    Err(AppError::BackendFailure {
        operation: "setup_skill",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_replace_publication_preserves_a_concurrently_created_file() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("SKILL.md");
        fs::write(&destination, b"concurrent contents").unwrap();

        let result = atomic_write(&destination, None);

        assert_eq!(result.unwrap(), AtomicWriteOutcome::AlreadyExists);
        assert_eq!(fs::read(destination).unwrap(), b"concurrent contents");
    }

    #[test]
    fn concurrent_identical_publication_is_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("SKILL.md");
        fs::write(&destination, SKILL_BYTES).unwrap();

        assert_eq!(
            atomic_write(&destination, None).unwrap(),
            AtomicWriteOutcome::AlreadyExists
        );
        assert_eq!(
            existing_skill_action(&destination, false).unwrap(),
            Some(SetupSkillAction::Unchanged)
        );
    }
}
