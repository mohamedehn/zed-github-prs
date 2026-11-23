use crate::gh;
use anyhow::{Context, Result};
use serde_json::{json, Value};

const REVIEW_QUERY: &str = "query($owner:String!,$name:String!,$number:Int!){\
repository(owner:$owner,name:$name){pullRequest(number:$number){id \
files(first:100){nodes{path viewerViewedState}} \
reviewThreads(first:100){nodes{id isResolved isOutdated path line originalLine diffSide viewerCanResolve \
comments(first:100){nodes{databaseId author{login} body createdAt}}}}}}}";

/// PR node id, per-file viewed state and review threads.
pub fn review_data(repo: &str, number: u64) -> Result<Value> {
    let (owner, name) = repo.split_once('/').context("invalid repo")?;
    let data = gh::graphql(
        REVIEW_QUERY,
        json!({ "owner": owner, "name": name, "number": number }),
    )?;
    Ok(data["repository"]["pullRequest"].clone())
}

pub fn set_thread_resolved(thread_id: &str, resolved: bool) -> Result<()> {
    let mutation = if resolved {
        "mutation($id:ID!){resolveReviewThread(input:{threadId:$id}){clientMutationId}}"
    } else {
        "mutation($id:ID!){unresolveReviewThread(input:{threadId:$id}){clientMutationId}}"
    };
    gh::graphql(mutation, json!({ "id": thread_id }))?;
    Ok(())
}

pub fn set_file_viewed(pull_request_id: &str, path: &str, viewed: bool) -> Result<()> {
    let mutation = if viewed {
        "mutation($id:ID!,$path:String!){markFileAsViewed(input:{pullRequestId:$id,path:$path}){clientMutationId}}"
    } else {
        "mutation($id:ID!,$path:String!){unmarkFileAsViewed(input:{pullRequestId:$id,path:$path}){clientMutationId}}"
    };
    gh::graphql(mutation, json!({ "id": pull_request_id, "path": path }))?;
    Ok(())
}

/// Replies to a review thread; `comment_id` is the REST id of any comment in it.
pub fn reply(repo: &str, number: u64, comment_id: u64, body: &str) -> Result<()> {
    gh::post(
        &format!("repos/{repo}/pulls/{number}/comments/{comment_id}/replies"),
        &json!({ "body": body }),
    )?;
    Ok(())
}
