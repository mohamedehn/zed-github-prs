use crate::gh;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::process::Command;

/// Rejects refs that could be read as git options or revision expressions.
pub fn validate_ref(name: &str) -> Result<()> {
    let forbidden = |c: char| c.is_whitespace() || c.is_control() || "~^:?*[\\".contains(c);
    if name.is_empty() || name.starts_with('-') || name.contains("..") || name.contains(forbidden) {
        bail!("invalid branch or ref `{name}`");
    }
    Ok(())
}

/// Percent-encodes everything except unreserved characters and `/`.
pub fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn git(args: &[&str]) -> Result<String> {
    let output = Command::new("git").args(args).output()?;
    if !output.status.success() {
        bail!(
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Compare two refs through the GitHub API (works for pushed branches, tags and SHAs).
pub fn github(repo: &str, base: &str, head: &str) -> Result<Value> {
    validate_ref(base)?;
    validate_ref(head)?;
    let result = gh::get(&format!(
        "repos/{repo}/compare/{}...{}",
        encode(base),
        encode(head)
    ))?;
    let commits: Vec<Value> = result["commits"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|c| {
            let message = c["commit"]["message"].as_str().unwrap_or_default();
            json!({
                "sha": c["sha"].as_str().unwrap_or_default().chars().take(7).collect::<String>(),
                "author": c["commit"]["author"]["name"],
                "message": message.lines().next().unwrap_or_default(),
            })
        })
        .collect();
    Ok(json!({
        "total_commits": result["total_commits"],
        "ahead_by": result["ahead_by"],
        "behind_by": result["behind_by"],
        "commits": commits,
        "files": result["files"],
    }))
}

/// Compare two refs with the local git checkout (works for unpushed branches).
pub fn local(base: &str, head: &str) -> Result<Value> {
    validate_ref(base)?;
    validate_ref(head)?;
    let diff = git(&[
        "diff",
        "--no-color",
        "--find-renames",
        "--unified=3",
        &format!("{base}...{head}"),
        "--",
    ])?;
    let log = git(&[
        "log",
        "--max-count=100",
        "--format=%h%x09%an%x09%s",
        &format!("{base}..{head}"),
        "--",
    ])?;
    let commits: Vec<Value> = log
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            Some(json!({ "sha": parts.next()?, "author": parts.next()?, "message": parts.next()? }))
        })
        .collect();
    Ok(json!({
        "total_commits": commits.len(),
        "commits": commits,
        "files": parse_git_diff(&diff),
    }))
}

/// Branch checked out in the current directory.
pub fn current_branch() -> Result<String> {
    let branch = git(&["branch", "--show-current"])?.trim().to_owned();
    if branch.is_empty() {
        bail!("no branch checked out (detached HEAD)");
    }
    Ok(branch)
}

pub fn default_branch(repo: &str) -> Result<String> {
    let info = gh::get(&format!("repos/{repo}"))?;
    Ok(info["default_branch"].as_str().unwrap_or("main").to_owned())
}

/// Local ref to compare against: `origin/<name>` when it exists (fresher), else `<name>`.
pub fn local_ref(name: &str) -> Result<String> {
    validate_ref(name)?;
    for candidate in [format!("origin/{name}"), name.to_owned()] {
        if git(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{candidate}^{{commit}}"),
        ])
        .is_ok()
        {
            return Ok(candidate);
        }
    }
    bail!("branch `{name}` not found locally or on origin; run `git fetch` first")
}

/// Commit subjects of `base..head`, oldest first.
pub fn commit_subjects(base: &str, head: &str) -> Result<Vec<String>> {
    validate_ref(base)?;
    validate_ref(head)?;
    let log = git(&[
        "log",
        "--reverse",
        "--format=%s",
        &format!("{base}..{head}"),
        "--",
    ])?;
    Ok(log.lines().map(str::to_owned).collect())
}

/// Pushes a branch to `origin` and sets its upstream. Never prompts for credentials.
pub fn push(branch: &str) -> Result<()> {
    validate_ref(branch)?;
    let output = Command::new("git")
        .args(["push", "--set-upstream", "origin", branch])
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()?;
    if !output.status.success() {
        bail!(
            "git push failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Branch names on GitHub and in the local checkout (the latter may be empty).
pub fn branches(repo: &str) -> Result<Value> {
    let remote: Vec<String> = gh::get_all(&format!("repos/{repo}/branches?per_page=100"))?
        .iter()
        .filter_map(|branch| branch["name"].as_str().map(str::to_owned))
        .collect();
    let local: Vec<String> = git(&["for-each-ref", "--format=%(refname:short)", "refs/heads"])
        .map(|out| out.lines().map(str::to_owned).collect())
        .unwrap_or_default();
    Ok(json!({ "remote": remote, "local": local }))
}

struct DiffFile {
    name: String,
    previous: Option<String>,
    status: &'static str,
    patch: Vec<String>,
    additions: u32,
    deletions: u32,
}

impl DiffFile {
    fn into_json(self) -> Value {
        let patch = (!self.patch.is_empty()).then(|| self.patch.join("\n"));
        json!({
            "filename": self.name,
            "previous_filename": self.previous,
            "status": self.status,
            "additions": self.additions,
            "deletions": self.deletions,
            "changes": self.additions + self.deletions,
            "patch": patch,
        })
    }
}

/// Splits `git diff` output into the same shape GitHub returns for changed files.
pub fn parse_git_diff(diff: &str) -> Vec<Value> {
    let mut files = Vec::new();
    let mut current: Option<DiffFile> = None;
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            files.extend(current.take().map(DiffFile::into_json));
            current = Some(DiffFile {
                name: rest
                    .rsplit_once(" b/")
                    .map_or(rest, |(_, name)| name)
                    .to_owned(),
                previous: None,
                status: "modified",
                patch: Vec::new(),
                additions: 0,
                deletions: 0,
            });
            continue;
        }
        let Some(file) = current.as_mut() else {
            continue;
        };
        if !file.patch.is_empty() {
            match line.as_bytes().first() {
                Some(b'+') => file.additions += 1,
                Some(b'-') => file.deletions += 1,
                _ => {}
            }
            file.patch.push(line.to_owned());
        } else if line.starts_with("@@") {
            file.patch.push(line.to_owned());
        } else if line.starts_with("new file mode") {
            file.status = "added";
        } else if line.starts_with("deleted file mode") {
            file.status = "removed";
        } else if let Some(previous) = line.strip_prefix("rename from ") {
            file.previous = Some(previous.to_owned());
            file.status = "renamed";
        } else if let Some(name) = line.strip_prefix("rename to ") {
            file.name = name.to_owned();
        }
    }
    files.extend(current.map(DiffFile::into_json));
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\ndiff --git a/new.rs b/new.rs\nnew file mode 100644\nindex 0..3\n--- /dev/null\n+++ b/new.rs\n@@ -0,0 +1 @@\n+hi\ndiff --git a/x b/y\nsimilarity index 100%\nrename from x\nrename to y\n";

    #[test]
    fn parses_modified_added_and_renamed_files() {
        let files = parse_git_diff(DIFF);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0]["filename"], "a.txt");
        assert_eq!(
            (
                files[0]["additions"].as_u64(),
                files[0]["deletions"].as_u64()
            ),
            (Some(1), Some(1))
        );
        assert_eq!(files[1]["status"], "added");
        assert_eq!(files[2]["filename"], "y");
        assert_eq!(files[2]["previous_filename"], "x");
        assert!(files[2]["patch"].is_null());
    }

    #[test]
    fn rejects_dangerous_refs_and_encodes_the_rest() {
        assert!(validate_ref("--output=x").is_err());
        assert!(validate_ref("a..b").is_err());
        assert!(validate_ref("feature/login").is_ok());
        assert_eq!(encode("feat/a b#1"), "feat/a%20b%231");
    }
}
