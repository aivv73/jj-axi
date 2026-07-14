use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::error::AppError;
use crate::model::{PrChecks, PrStatusData};

const QUERY: &str = "query($owner:String!,$name:String!,$number:Int!,$after:String){repository(owner:$owner,name:$name){pullRequest(number:$number){number url state isDraft mergeable reviewDecision headRefName headRefOid baseRefName commits(last:1){nodes{commit{statusCheckRollup{contexts(first:100,after:$after){nodes{__typename ... on CheckRun{status conclusion} ... on StatusContext{state}} pageInfo{hasNextPage endCursor}}}}}}}}}";
const MAX_CHECKS: usize = 1_000;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
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
    remote_urls: &[String],
) -> Result<PrStatusData, AppError> {
    let identity = if let Some(repository) = repository {
        RepositoryIdentity::parse(repository)?
    } else {
        let mut identities: Vec<_> = remote_urls
            .iter()
            .filter_map(|url| identity_from_remote(url))
            .collect();
        identities.sort();
        identities.dedup();
        match identities.as_slice() {
            [] => return Err(AppError::GithubRepositoryNotFound),
            [identity] => identity.clone(),
            _ => {
                let mut candidates: Vec<_> =
                    identities.iter().map(RepositoryIdentity::display).collect();
                candidates.truncate(3);
                return Err(AppError::GithubRepositoryAmbiguous { candidates });
            }
        }
    };
    let (pr_value, nodes) = fetch_pr_pages(&identity, number)?;
    let pr = pr_value
        .as_object()
        .ok_or(AppError::GithubResponseInvalid)?;
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

fn fetch_pr_pages(
    identity: &RepositoryIdentity,
    number: u64,
) -> Result<(Value, Vec<Value>), AppError> {
    let mut after: Option<String> = None;
    let mut seen_cursors = HashSet::new();
    let mut first_pr = None;
    let mut nodes = Vec::new();
    loop {
        let mut command = Command::new("gh");
        command
            .args(["api", "graphql", "-f", &format!("query={QUERY}")])
            .args(["-F", &format!("owner={}", identity.owner)])
            .args(["-F", &format!("name={}", identity.name)])
            .args(["-F", &format!("number={number}")])
            .env("GH_PROMPT_DISABLED", "1");
        if let Some(cursor) = &after {
            command.args(["-F", &format!("after={cursor}")]);
        }
        if identity.host != "github.com" {
            command.args(["--hostname", &identity.host]);
        }
        let output = command.output().map_err(|_| AppError::GithubCliNotFound)?;
        if !output.status.success() {
            return Err(classify_gh_failure(&output.stderr));
        }
        let root: Value =
            serde_json::from_slice(&output.stdout).map_err(|_| AppError::GithubResponseInvalid)?;
        if let Some(errors) = root.get("errors").and_then(Value::as_array) {
            let message = errors
                .iter()
                .filter_map(|error| error.get("message").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase();
            if message.contains("rate limit") {
                return Err(AppError::GithubRateLimited);
            }
            if message.contains("repository")
                && (message.contains("resolve") || message.contains("not found"))
            {
                return Err(AppError::GithubRepositoryNotFound);
            }
            if message.contains("pull request") || message.contains("pullrequest") {
                return Err(AppError::PullRequestNotFound { number });
            }
            return Err(AppError::GithubApiUnavailable { retryable: true });
        }
        let pr = root
            .pointer("/data/repository/pullRequest")
            .cloned()
            .ok_or(AppError::GithubResponseInvalid)?;
        if first_pr.is_none() {
            first_pr = Some(pr.clone());
        }
        let contexts = pr
            .pointer("/commits/nodes/0/commit/statusCheckRollup/contexts")
            .and_then(Value::as_object);
        if let Some(page_nodes) = contexts
            .and_then(|value| value.get("nodes"))
            .and_then(Value::as_array)
        {
            nodes.extend(page_nodes.iter().cloned());
        }
        if nodes.len() > MAX_CHECKS {
            return Err(AppError::GithubResponseTooLarge);
        }
        let page_info = contexts.and_then(|value| value.get("pageInfo"));
        let has_next = page_info
            .and_then(|value| value.get("hasNextPage"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !has_next {
            break;
        }
        let cursor = page_info
            .and_then(|value| value.get("endCursor"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or(AppError::GithubResponseInvalid)?
            .to_owned();
        if !seen_cursors.insert(cursor.clone()) {
            return Err(AppError::GithubResponseInvalid);
        }
        after = Some(cursor);
    }
    Ok((first_pr.ok_or(AppError::GithubResponseInvalid)?, nodes))
}

fn classify_gh_failure(stderr: &[u8]) -> AppError {
    let message = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    if message.contains("auth") || message.contains("login") || message.contains("token") {
        AppError::GithubAuthRequired
    } else if message.contains("rate limit") {
        AppError::GithubRateLimited
    } else {
        AppError::GithubApiUnavailable { retryable: true }
    }
}

fn identity_from_remote(url: &str) -> Option<RepositoryIdentity> {
    let (host, path) = if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    {
        rest.split_once('/')?
    } else if let Some(rest) = url.strip_prefix("ssh://") {
        let rest = rest.split_once('@').map_or(rest, |(_, value)| value);
        rest.split_once('/')?
    } else {
        let rest = url.split_once('@').map_or(url, |(_, value)| value);
        rest.split_once(':')?
    };
    if !host.eq_ignore_ascii_case("github.com") {
        return None;
    }
    let mut parts = path.trim_matches('/').split('/');
    let owner = parts.next()?;
    let name = parts.next()?.trim_end_matches(".git");
    if owner.is_empty() || name.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(RepositoryIdentity {
        host: "github.com".to_owned(),
        owner: owner.to_owned(),
        name: name.to_owned(),
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
