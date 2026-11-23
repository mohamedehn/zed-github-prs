use crate::{gh, models::PullSummary};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashSet;

pub fn list_open(repo: &str) -> Result<Vec<PullSummary>> {
    let pulls = gh::get_all(&format!("repos/{repo}/pulls?state=open&per_page=100"))?;
    Ok(pulls.iter().map(PullSummary::from_json).collect())
}

pub fn details(repo: &str, number: u64) -> Result<Value> {
    let pr = gh::get(&format!("repos/{repo}/pulls/{number}"))?;
    Ok(json!({
        "summary": PullSummary::from_json(&pr),
        "state": pr["state"],
        "body": pr["body"],
        "mergeable_state": pr["mergeable_state"],
        "changed_files": pr["changed_files"],
        "additions": pr["additions"],
        "deletions": pr["deletions"],
        "head_sha": pr["head"]["sha"],
    }))
}

/// CI check runs of the PR head commit.
pub fn checks(repo: &str, number: u64) -> Result<Value> {
    let pr = gh::get(&format!("repos/{repo}/pulls/{number}"))?;
    let sha = pr["head"]["sha"].as_str().unwrap_or_default();
    if sha.is_empty() || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("unexpected head sha");
    }
    let runs = gh::get(&format!(
        "repos/{repo}/commits/{sha}/check-runs?per_page=100"
    ))?;
    let runs = runs["check_runs"].as_array().cloned().unwrap_or_default();
    Ok(Value::Array(
        runs.iter()
            .map(|run| {
                json!({
                    "name": run["name"], "status": run["status"],
                    "conclusion": run["conclusion"], "url": run["html_url"],
                })
            })
            .collect(),
    ))
}

/// Changed files with their patch (patch is absent for binary or very large files).
pub fn files(repo: &str, number: u64) -> Result<Vec<Value>> {
    gh::get_all(&format!("repos/{repo}/pulls/{number}/files?per_page=100"))
}

/// Lines a review comment can attach to: (side, line) where side is "LEFT" or "RIGHT".
pub fn commentable_lines(patch: &str) -> HashSet<(&'static str, u32)> {
    let (mut left, mut right) = (0u32, 0u32);
    let mut lines = HashSet::new();
    for line in patch.lines() {
        if let Some(header) = line.strip_prefix("@@ -") {
            let start = |s: &str| s.split(',').next().and_then(|n| n.parse::<u32>().ok());
            let mut parts = header.split_whitespace();
            left = parts.next().and_then(start).unwrap_or(0);
            right = parts
                .next()
                .and_then(|p| start(p.trim_start_matches('+')))
                .unwrap_or(0);
        } else if line.starts_with('+') {
            lines.insert(("RIGHT", right));
            right += 1;
        } else if line.starts_with('-') {
            lines.insert(("LEFT", left));
            left += 1;
        } else if line.starts_with(' ') {
            lines.insert(("LEFT", left));
            lines.insert(("RIGHT", right));
            left += 1;
            right += 1;
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "@@ -10,3 +10,4 @@ fn x\n ctx\n-old\n+new\n+added\n tail";

    #[test]
    fn maps_added_removed_and_context_lines() {
        let lines = commentable_lines(PATCH);
        assert!(lines.contains(&("RIGHT", 11)));
        assert!(lines.contains(&("RIGHT", 12)));
        assert!(lines.contains(&("LEFT", 11)));
        assert!(lines.contains(&("RIGHT", 13)));
        assert!(!lines.contains(&("RIGHT", 50)));
    }
}
