use crate::{compare, gh, pulls};
use anyhow::{bail, Result};
use serde_json::{json, Value};

const TEMPLATE_PATHS: [&str; 4] = [
    ".github/pull_request_template.md",
    ".github/PULL_REQUEST_TEMPLATE.md",
    "pull_request_template.md",
    "docs/pull_request_template.md",
];

/// "feature/login-page" -> "Login page"
fn title_from_branch(branch: &str) -> String {
    let name = branch
        .rsplit('/')
        .next()
        .unwrap_or(branch)
        .replace(['-', '_'], " ");
    let mut chars = name.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Everything the "create pull request" form needs for the checked-out branch.
pub fn prepare(repo: &str, base: Option<&str>) -> Result<Value> {
    let head = compare::current_branch()?;
    let default_base = compare::default_branch(repo)?;
    let base = base
        .filter(|name| !name.is_empty())
        .map_or_else(|| default_base.clone(), str::to_owned);
    if head == base {
        bail!("you are on `{base}`, the target branch; switch to the branch you want to open a pull request from");
    }
    let pushed = gh::get(&format!(
        "repos/{repo}/git/ref/heads/{}",
        compare::encode(&head)
    ))
    .is_ok();
    let existing = pulls::list_open(repo)?
        .iter()
        .find(|pr| pr.head == head && pr.base == base)
        .map(|pr| pr.number);
    // `None` when the target branch is not available locally (the commits are then unknown).
    let known_subjects = compare::local_ref(&base)
        .and_then(|base_ref| compare::commit_subjects(&base_ref, &head))
        .ok();
    let subjects = known_subjects.clone().unwrap_or_default();
    let title = match subjects.as_slice() {
        [only] => only.clone(),
        _ => title_from_branch(&head),
    };
    let body = TEMPLATE_PATHS
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_else(|| {
            subjects
                .iter()
                .map(|subject| format!("- {subject}"))
                .collect::<Vec<_>>()
                .join("\n")
        });
    Ok(json!({
        "head": head, "base": base, "default_base": default_base,
        "title": title, "body": body,
        "pushed": pushed, "existing": existing,
        "commit_count": known_subjects.map(|subjects| subjects.len()),
    }))
}

pub fn push_current_branch() -> Result<String> {
    let head = compare::current_branch()?;
    compare::push(&head)?;
    Ok(head)
}

/// Opens a pull request from the checked-out branch (the head is never taken from the client).
pub fn create_pull(repo: &str, title: &str, body: &str, base: &str, draft: bool) -> Result<Value> {
    if title.trim().is_empty() {
        bail!("a title is required");
    }
    compare::validate_ref(base)?;
    let head = compare::current_branch()?;
    let created = gh::post(
        &format!("repos/{repo}/pulls"),
        &json!({ "title": title.trim(), "body": body, "head": head, "base": base, "draft": draft }),
    )?;
    Ok(json!({ "number": created["number"], "url": created["html_url"] }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_titles_from_branch_names() {
        assert_eq!(title_from_branch("feature/login-page"), "Login page");
        assert_eq!(title_from_branch("fix_bug"), "Fix bug");
        assert_eq!(title_from_branch(""), "");
    }
}
