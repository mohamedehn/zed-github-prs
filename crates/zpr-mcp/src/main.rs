use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, IsTerminal, Write};
use zpr_core::{compare, gh::resolve_repo, models::ReviewComment, pulls, reviews};

mod ui;

const REPO_PROP: &str =
    "Repository as owner/name. Defaults to the repository of the working directory.";

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
    })
}

fn tool_list() -> Value {
    let repo = json!({ "type": "string", "description": REPO_PROP });
    let number = json!({ "type": "integer", "description": "Pull request number" });
    json!([
        tool("list_pull_requests", "List open pull requests.", json!({ "repo": repo }), &[]),
        tool(
            "get_pull_request",
            "Get a pull request: description, branches, stats, mergeable state.",
            json!({ "repo": repo, "number": number }),
            &["number"],
        ),
        tool(
            "list_changed_files",
            "List files changed by a pull request with status and +/- counts.",
            json!({ "repo": repo, "number": number }),
            &["number"],
        ),
        tool(
            "get_file_patch",
            "Get the unified diff of one changed file.",
            json!({ "repo": repo, "number": number, "path": { "type": "string" } }),
            &["number", "path"],
        ),
        tool(
            "submit_review",
            "Submit a review. APPROVE and REQUEST_CHANGES act on GitHub: ask the user for explicit confirmation first. \
             Inline comments must target lines present in the diff (side RIGHT = new code, LEFT = old code).",
            json!({
                "repo": repo,
                "number": number,
                "event": { "type": "string", "enum": ["COMMENT", "APPROVE", "REQUEST_CHANGES"] },
                "body": { "type": "string" },
                "comments": { "type": "array", "items": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "line": { "type": "integer" },
                        "side": { "type": "string", "enum": ["LEFT", "RIGHT"] },
                        "body": { "type": "string" }
                    },
                    "required": ["path", "line", "body"]
                }}
            }),
            &["number", "event"],
        ),
        tool(
            "open_pr_ui",
            "Open the visual review UI (diffs, inline comments, threads, branch comparison) in the browser. \
             With no arguments it detects the checked-out branch: it opens that branch's pull request, or the \"create pull request\" form (title, description, target branch) when there is none. \
             Pass `target` to choose the branch the pull request goes into. \
             Pass `number` to open a pull request, or `base` and `head` to compare two named branches.",
            json!({
                "repo": repo,
                "number": number,
                "target": { "type": "string", "description": "Branch the pull request goes into (e.g. dev); default is the repository's default branch" },
                "base": { "type": "string", "description": "Base branch for a comparison" },
                "head": { "type": "string", "description": "Head branch for a comparison" },
                "local": { "type": "boolean", "description": "Compare with the local git checkout (unpushed branches)" }
            }),
            &[],
        ),
    ])
}

fn call_tool(name: &str, args: &Value) -> Result<Value> {
    let repo = resolve_repo(args["repo"].as_str())?;
    let number = || args["number"].as_u64().context("`number` is required");
    match name {
        "list_pull_requests" => Ok(json!(pulls::list_open(&repo)?)),
        "get_pull_request" => pulls::details(&repo, number()?),
        "list_changed_files" => {
            let files = pulls::files(&repo, number()?)?;
            Ok(Value::Array(
                files
                    .iter()
                    .map(|f| {
                        json!({
                            "filename": f["filename"], "status": f["status"],
                            "additions": f["additions"], "deletions": f["deletions"],
                        })
                    })
                    .collect(),
            ))
        }
        "get_file_patch" => {
            let path = args["path"].as_str().context("`path` is required")?;
            let files = pulls::files(&repo, number()?)?;
            let file = files
                .iter()
                .find(|f| f["filename"] == path)
                .context("file not in this PR")?;
            Ok(json!({ "filename": path, "patch": file["patch"] }))
        }
        "submit_review" => {
            let comments: Vec<ReviewComment> = args["comments"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .map(|c| ReviewComment {
                    path: c["path"].as_str().unwrap_or_default().to_owned(),
                    line: c["line"].as_u64().unwrap_or(0) as u32,
                    side: c["side"].as_str().unwrap_or("RIGHT").to_owned(),
                    body: c["body"].as_str().unwrap_or_default().to_owned(),
                })
                .collect();
            let event = args["event"].as_str().context("`event` is required")?;
            reviews::submit_review(
                &repo,
                number()?,
                event,
                args["body"].as_str().unwrap_or(""),
                comments,
            )?;
            Ok(json!({ "submitted": event }))
        }
        "open_pr_ui" => {
            let hash = match (
                args["number"].as_u64(),
                args["base"].as_str(),
                args["head"].as_str(),
            ) {
                (Some(number), _, _) => format!("/pr/{number}"),
                (None, None, None) => current_branch_hash(&repo, args["target"].as_str())?,
                (None, Some(base), Some(head)) => {
                    let local = if args["local"].as_bool() == Some(true) {
                        "&local=1"
                    } else {
                        ""
                    };
                    format!(
                        "/compare?base={}&head={}{local}",
                        compare::encode(base),
                        compare::encode(head)
                    )
                }
                _ => "/".to_owned(),
            };
            open_ui(&repo, &hash)
        }
        _ => bail!("unknown tool {name}"),
    }
}

/// Page for the checked-out branch: its open pull request (into `target` when given),
/// else the "create pull request" form. The PR list is used outside a git repo or on the
/// default branch.
fn current_branch_hash(repo: &str, target: Option<&str>) -> Result<String> {
    let branch = match compare::current_branch() {
        Ok(branch) => branch,
        Err(_) if target.is_none() => return Ok("/".to_owned()),
        Err(e) => return Err(e),
    };
    let open_prs = pulls::list_open(repo)?;
    let existing = open_prs
        .iter()
        .find(|pr| pr.head == branch && target.is_none_or(|base| pr.base == base));
    if let Some(pr) = existing {
        return Ok(format!("/pr/{}", pr.number));
    }
    match target {
        Some(base) => Ok(format!("/new?base={}", compare::encode(base))),
        None if branch == compare::default_branch(repo)? => Ok("/".to_owned()),
        None => Ok("/new".to_owned()),
    }
}

fn open_ui(repo: &str, hash: &str) -> Result<Value> {
    let server = ui::start()?;
    let url = ui::page_url(server, repo, hash);
    ui::open_browser(&url);
    Ok(json!({ "opened": url }))
}

fn handle(request: &Value) -> Option<Value> {
    let id = request.get("id")?.clone();
    let result = match request["method"].as_str()? {
        "initialize" => Ok(json!({
            "protocolVersion": request["params"]["protocolVersion"].as_str().unwrap_or("2024-11-05"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "zpr-mcp", "version": env!("CARGO_PKG_VERSION") },
        })),
        "tools/list" => Ok(json!({ "tools": tool_list() })),
        "tools/call" => {
            let params = &request["params"];
            let outcome = call_tool(
                params["name"].as_str().unwrap_or_default(),
                &params["arguments"],
            );
            Ok(match outcome {
                Ok(value) => json!({ "content": [{ "type": "text", "text": value.to_string() }] }),
                Err(e) => {
                    json!({ "isError": true, "content": [{ "type": "text", "text": format!("{e:#}") }] })
                }
            })
        }
        "ping" => Ok(json!({})),
        other => Err(format!("method not found: {other}")),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(message) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": message } })
        }
    })
}

/// `zpr [target-branch] [--repo owner/name]`: run only the review UI, without MCP.
fn run_standalone_ui(args: &[String]) -> Result<()> {
    let mut target = None;
    let mut repo = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--ui" => {}
            "--repo" => repo = Some(rest.next().context("--repo needs owner/name")?.as_str()),
            "--target" => target = Some(rest.next().context("--target needs a branch name")?),
            name if !name.starts_with('-') => target = Some(arg),
            flag => bail!("unknown option {flag}"),
        }
    }
    let repo = resolve_repo(repo)?;
    let hash = current_branch_hash(&repo, target.map(String::as_str))?;
    let url = open_ui(&repo, &hash)?["opened"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    eprintln!("Review UI running at {url}\nPress Ctrl+C to stop.");
    loop {
        std::thread::park();
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    // Zed starts the server with stdin piped and no arguments; anything else is the CLI.
    if args.len() > 1 || std::io::stdin().is_terminal() {
        return run_standalone_ui(&args[1..]);
    }
    let mut out = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let Ok(request) = serde_json::from_str::<Value>(&line?) else {
            continue;
        };
        if let Some(response) = handle(&request) {
            writeln!(out, "{response}")?;
            out.flush()?;
        }
    }
    Ok(())
}
