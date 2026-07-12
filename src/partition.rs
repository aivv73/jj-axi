use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::cli::{HunkSpec, parse_manifest_hunk};
use crate::error::AppError;

pub(crate) const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_PARTS: usize = 100;
const MAX_ASSIGNMENTS: usize = 10_000;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PartitionManifest {
    pub schema_version: u64,
    pub source_commit_id: String,
    pub parts: Vec<PartitionPart>,
    pub remainder: PartitionRemainder,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PartitionPart {
    pub description: String,
    pub hunks: Vec<ManifestHunk>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManifestHunk {
    pub path: String,
    pub lines: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemainderDestination {
    RemainingChange,
    WorkingCopy,
    RequireEmpty,
}

impl RemainderDestination {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::RemainingChange => "remaining_change",
            Self::WorkingCopy => "working_copy",
            Self::RequireEmpty => "require_empty",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PartitionRemainder {
    pub destination: RemainderDestination,
}

#[derive(Clone, Debug)]
pub(crate) struct LoadedManifest {
    pub manifest: PartitionManifest,
    pub sha256: String,
    pub specs: Vec<Vec<HunkSpec>>,
}

pub(crate) fn load(path: &str, cwd: &Path) -> Result<LoadedManifest, AppError> {
    let bytes = read_bounded(path, cwd)?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid("/", "utf8_json"))?;
    if text.trim().is_empty() {
        return Err(invalid("/", "non_empty_json_object"));
    }
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let manifest = serde_path_to_error::deserialize::<_, PartitionManifest>(&mut deserializer)
        .map_err(|error| {
            invalid(
                &json_pointer(&error.path().to_string()),
                &format!("json: {}", error.inner()),
            )
        })?;
    deserializer
        .end()
        .map_err(|error| invalid("/", &format!("json: {error}")))?;
    validate(manifest, sha256)
}

fn read_bounded(path: &str, cwd: &Path) -> Result<Vec<u8>, AppError> {
    let mut reader: Box<dyn Read> = if path == "-" {
        Box::new(io::stdin().lock())
    } else {
        let resolved = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            cwd.join(path)
        };
        Box::new(
            File::open(&resolved).map_err(|_| AppError::InvalidPartitionManifest {
                pointer: "/".to_owned(),
                reason: "readable_regular_file".to_owned(),
            })?,
        )
    };
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("/", "readable_input"))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(AppError::PartitionManifestTooLarge {
            limit_bytes: MAX_MANIFEST_BYTES,
        });
    }
    Ok(bytes)
}

fn validate(manifest: PartitionManifest, sha256: String) -> Result<LoadedManifest, AppError> {
    if manifest.schema_version != 1 {
        return Err(invalid("/schema_version", "supported_version_1"));
    }
    if manifest.source_commit_id.is_empty() {
        return Err(invalid("/source_commit_id", "full_commit_id"));
    }
    if manifest.parts.is_empty() || manifest.parts.len() > MAX_PARTS {
        return Err(invalid("/parts", "between_1_and_100_parts"));
    }
    let assignments: usize = manifest.parts.iter().map(|part| part.hunks.len()).sum();
    if assignments > MAX_ASSIGNMENTS {
        return Err(invalid("/parts", "at_most_10000_hunk_assignments"));
    }
    let mut specs = Vec::with_capacity(manifest.parts.len());
    for (index, part) in manifest.parts.iter().enumerate() {
        if part.description.trim().is_empty() {
            return Err(invalid(
                &format!("/parts/{index}/description"),
                "non_whitespace_description",
            ));
        }
        if part.hunks.is_empty() {
            return Err(invalid(
                &format!("/parts/{index}/hunks"),
                "at_least_one_hunk",
            ));
        }
        let mut part_specs = Vec::with_capacity(part.hunks.len());
        for (hunk_index, hunk) in part.hunks.iter().enumerate() {
            part_specs.push(parse_manifest_hunk(&hunk.path, &hunk.lines).map_err(|_| {
                invalid(
                    &format!("/parts/{index}/hunks/{hunk_index}"),
                    "canonical_post_image_hunk",
                )
            })?);
        }
        specs.push(part_specs);
    }
    Ok(LoadedManifest {
        manifest,
        sha256,
        specs,
    })
}

fn json_pointer(path: &str) -> String {
    if path.is_empty() || path == "." {
        return "/".to_owned();
    }
    let mut pointer = String::new();
    let mut chars = path.trim_start_matches('.').chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '.' => pointer.push('/'),
            '[' => {
                pointer.push('/');
                for next in chars.by_ref() {
                    if next == ']' {
                        break;
                    }
                    pointer.push(next);
                }
            }
            other => {
                if pointer.is_empty() {
                    pointer.push('/');
                }
                pointer.push(other);
            }
        }
    }
    pointer
}

fn invalid(pointer: &str, reason: &str) -> AppError {
    AppError::InvalidPartitionManifest {
        pointer: pointer.to_owned(),
        reason: reason.to_owned(),
    }
}
