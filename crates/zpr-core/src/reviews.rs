use crate::{gh, models::ReviewComment, pulls};
use anyhow::{bail, Result};
use serde_json::json;

pub fn submit_review(
    repo: &str,
    number: u64,
    event: &str,
    body: &str,
    comments: Vec<ReviewComment>,
) -> Result<()> {
    if !matches!(event, "COMMENT" | "APPROVE" | "REQUEST_CHANGES") {
        bail!("event must be COMMENT, APPROVE or REQUEST_CHANGES");
    }
    if !comments.is_empty() {
        let files = pulls::files(repo, number)?;
        for comment in &comments {
            let patch = files
                .iter()
                .find(|f| f["filename"] == comment.path.as_str())
                .and_then(|f| f["patch"].as_str());
            let Some(patch) = patch else {
                bail!("{} is not in this PR or has no patch", comment.path);
            };
            let side = if comment.side == "LEFT" {
                "LEFT"
            } else {
                "RIGHT"
            };
            if !pulls::commentable_lines(patch).contains(&(side, comment.line)) {
                bail!(
                    "{}:{} ({side}) is not a line in the PR diff",
                    comment.path,
                    comment.line
                );
            }
        }
    }
    gh::post(
        &format!("repos/{repo}/pulls/{number}/reviews"),
        &json!({ "event": event, "body": body, "comments": comments }),
    )?;
    Ok(())
}
