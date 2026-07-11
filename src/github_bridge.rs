use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::error::AppError;
use crate::model::{PrChecks, PrStatusData};

const QUERY: &str = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){pullRequest(number:$number){number url state isDraft mergeable reviewDecision headRefName headRefOid baseRefName commits(last:1){nodes{commit{statusCheckRollup{contexts(first:100){nodes{__typename ... on CheckRun{status conclusion} ... on StatusContext{state}} pageInfo{hasNextPage endCursor}}}}}}}}}";

#[derive(Clone, Debug)]
struct RepositoryIdentity {
    host: String,
    owner: String,
    name: String,
}

impl RepositoryIdentity {
    fn parse(value: &str) -> Result<Self, AppError> {
        let parts: Vec<_> = value.split('/').collect();
        let (host, owner, name) = match parts.as_slice() {
            [owner, name] if !owner.is_empty() && !name.is_empty() => ("github.com", *owner, *name),
            [host, owner, name] if !host.is_empty() && !owner.is_empty() && !name.is_empty() => {
                (*host, *owner, *name)
            }
            _ => {
                return Err(AppError::InvalidArgument {
                    argument: "repo",
                    constraint: "github_repository_identity",
                });
            }
        };
        Ok(Self {
            host: host.to_owned(),
            owner: owner.to_owned(),
            name: name.trim_end_matches(".git").to_owned(),
        })
    }

    fn display(&self) -> String {
        format!("{}/{}/{}", self.host, self.owner, self.name)
    }
}

pub(crate) fn pr_status(
    _cwd: &Path,
    number: u64,
    repository: Option<&str>,
) -> Result<PrStatusData, AppError> {
    let repository = repository.ok_or(AppError::InvalidArgument {
        argument: "repo",
        constraint: "required_until_repository_inference",
    })?;
    let identity = RepositoryIdentity::parse(repository)?;
    let mut command = Command::new("gh");
    command
        .args(["api", "graphql", "-f", &format!("query={QUERY}")])
        .args(["-F", &format!("owner={}", identity.owner)])
        .args(["-F", &format!("name={}", identity.name)])
        .args(["-F", &format!("number={number}")])
        .env("GH_PROMPT_DISABLED", "1");
    if identity.host != "github.com" {
        command.args(["--hostname", &identity.host]);
    }
    let output = command.output().map_err(|_| AppError::GithubCliNotFound)?;
    if !output.status.success() {
        return Err(AppError::GithubApiUnavailable { retryable: true });
    }
    let root: Value =
        serde_json::from_slice(&output.stdout).map_err(|_| AppError::GithubResponseInvalid)?;
    let pr_value = root
        .pointer("/data/repository/pullRequest")
        .ok_or(AppError::GithubResponseInvalid)?;
    let pr = pr_value
        .as_object()
        .ok_or(AppError::GithubResponseInvalid)?;
    let contexts = pr_value
        .pointer("/commits/nodes/0/commit/statusCheckRollup/contexts")
        .and_then(Value::as_object);
    if contexts
        .and_then(|value| value.get("pageInfo"))
        .and_then(|value| value.get("hasNextPage"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(AppError::GithubResponseInvalid);
    }
    let nodes = contexts
        .and_then(|value| value.get("nodes"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut checks = PrChecks {
        total: nodes.len() as u64,
        passed: 0,
        failed: 0,
        pending: 0,
        skipped: 0,
        status: String::new(),
    };
    for node in nodes {
        let kind = node.get("__typename").and_then(Value::as_str).unwrap_or("");
        let state = if kind == "CheckRun" {
            let status = node.get("status").and_then(Value::as_str).unwrap_or("");
            if status != "COMPLETED" {
                "pending"
            } else {
                normalize_check(node.get("conclusion").and_then(Value::as_str).unwrap_or(""))
            }
        } else {
            normalize_check(node.get("state").and_then(Value::as_str).unwrap_or(""))
        };
        match state {
            "passed" => checks.passed += 1,
            "failed" => checks.failed += 1,
            "skipped" => checks.skipped += 1,
            _ => checks.pending += 1,
        }
    }
    checks.status = if checks.failed > 0 {
        "failed"
    } else if checks.pending > 0 {
        "pending"
    } else if checks.total > 0 {
        "passed"
    } else {
        "none"
    }
    .to_owned();

    let state = required_string(pr, "state")?.to_ascii_lowercase();
    let draft = required_bool(pr, "isDraft")?;
    let mergeability = match required_string(pr, "mergeable")? {
        "MERGEABLE" => "mergeable",
        "CONFLICTING" => "conflicting",
        _ => "unknown",
    }
    .to_owned();
    let review = match pr.get("reviewDecision").and_then(Value::as_str) {
        Some("APPROVED") => "approved",
        Some("CHANGES_REQUESTED") => "changes_requested",
        Some("REVIEW_REQUIRED") => "review_required",
        None => "not_required",
        _ => "unknown",
    }
    .to_owned();
    let mut blocking_reasons = Vec::new();
    match state.as_str() {
        "closed" => blocking_reasons.push("pr_closed".to_owned()),
        "merged" => blocking_reasons.push("pr_merged".to_owned()),
        _ => {}
    }
    if draft {
        blocking_reasons.push("draft".to_owned());
    }
    match mergeability.as_str() {
        "conflicting" => blocking_reasons.push("merge_conflict".to_owned()),
        "unknown" => blocking_reasons.push("mergeability_unknown".to_owned()),
        _ => {}
    }
    if checks.status == "failed" {
        blocking_reasons.push("checks_failed".to_owned());
    }
    if checks.status == "pending" {
        blocking_reasons.push("checks_pending".to_owned());
    }
    match review.as_str() {
        "changes_requested" => blocking_reasons.push("changes_requested".to_owned()),
        "review_required" => blocking_reasons.push("review_required".to_owned()),
        "unknown" => blocking_reasons.push("review_unknown".to_owned()),
        _ => {}
    }
    let ready_to_merge = blocking_reasons.is_empty() && state == "open";
    Ok(PrStatusData {
        repository: identity.display(),
        number,
        url: required_string(pr, "url")?.to_owned(),
        state,
        draft,
        head_ref: required_string(pr, "headRefName")?.to_owned(),
        head_commit_id: required_string(pr, "headRefOid")?.to_owned(),
        base_ref: required_string(pr, "baseRefName")?.to_owned(),
        mergeability,
        review,
        checks,
        ready_to_merge,
        blocking_reasons,
    })
}

fn normalize_check(value: &str) -> &'static str {
    match value {
        "SUCCESS" => "passed",
        "SKIPPED" | "NEUTRAL" => "skipped",
        "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE"
        | "STALE" => "failed",
        _ => "pending",
    }
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, AppError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or(AppError::GithubResponseInvalid)
}

fn required_bool(object: &serde_json::Map<String, Value>, key: &str) -> Result<bool, AppError> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or(AppError::GithubResponseInvalid)
}
