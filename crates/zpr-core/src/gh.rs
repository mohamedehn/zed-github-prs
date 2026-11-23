use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

/// Runs `gh <args>` and returns stdout. Optional `stdin` is piped to the process.
fn run_gh(args: &[&str], stdin: Option<&str>) -> Result<String> {
    let mut child = Command::new("gh")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("GitHub CLI (`gh`) is not installed or not in PATH")?;
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .context("no stdin")?
            .write_all(input.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!(
            "gh failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// GET a (paginated) endpoint; pages are merged into one JSON array.
pub fn get_all(endpoint: &str) -> Result<Vec<Value>> {
    let out = run_gh(&["api", "--paginate", "--slurp", endpoint], None)?;
    let pages: Vec<Value> = serde_json::from_str(&out)?;
    Ok(pages
        .into_iter()
        .flat_map(|page| match page {
            Value::Array(items) => items,
            other => vec![other],
        })
        .collect())
}

pub fn get(endpoint: &str) -> Result<Value> {
    Ok(serde_json::from_str(&run_gh(&["api", endpoint], None)?)?)
}

pub fn post(endpoint: &str, body: &Value) -> Result<Value> {
    let out = run_gh(
        &["api", "-X", "POST", endpoint, "--input", "-"],
        Some(&body.to_string()),
    )?;
    Ok(serde_json::from_str(&out).unwrap_or(Value::Null))
}

/// Resolves "owner/name": the explicit value, else the repository of the current directory.
pub fn resolve_repo(explicit: Option<&str>) -> Result<String> {
    if let Some(repo) = explicit {
        if repo.split('/').count() != 2 || repo.contains(char::is_whitespace) {
            bail!("repo must look like owner/name");
        }
        return Ok(repo.to_owned());
    }
    let out = run_gh(
        &[
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "-q",
            ".nameWithOwner",
        ],
        None,
    )
    .context("Could not detect the repository; pass `repo` as owner/name")?;
    Ok(out.trim().to_owned())
}
