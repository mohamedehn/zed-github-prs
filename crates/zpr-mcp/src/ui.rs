//! Local review UI: a tiny HTTP server bound to 127.0.0.1 serving one HTML page and a JSON API.
//! Every request must carry the random token and a loopback `Host` header.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::{hash_map::RandomState, HashMap};
use std::hash::{BuildHasher, Hasher};
use std::io::{Cursor, Read};
use std::path::{Component, Path};
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::thread;
use tiny_http::{Header, Method, Request, Response, Server};
use zpr_core::{compare, create, gh, models::ReviewComment, pulls, reviews, threads};

const INDEX_HTML: &str = include_str!("../assets/index.html");
const MAX_BODY_BYTES: u64 = 1 << 20;

type HttpResponse = Response<Cursor<Vec<u8>>>;

pub struct Ui {
    pub origin: String,
    pub token: String,
}

struct ServerState {
    token: String,
    hosts: [String; 2],
}

static UI: OnceLock<Ui> = OnceLock::new();

fn random_token() -> String {
    (0..2)
        .map(|_| {
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u8(1);
            format!("{:016x}", hasher.finish())
        })
        .collect()
}

/// Starts the server once per process and returns its address and token.
pub fn start() -> Result<&'static Ui> {
    if let Some(ui) = UI.get() {
        return Ok(ui);
    }
    let server = Server::http("127.0.0.1:0").map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let port = server.server_addr().to_ip().context("no port")?.port();
    let token = random_token();
    let ctx = Arc::new(ServerState {
        token: token.clone(),
        hosts: [format!("127.0.0.1:{port}"), format!("localhost:{port}")],
    });
    thread::spawn(move || {
        for request in server.incoming_requests() {
            let ctx = Arc::clone(&ctx);
            thread::spawn(move || serve(request, &ctx));
        }
    });
    Ok(UI.get_or_init(|| Ui {
        origin: format!("http://127.0.0.1:{port}"),
        token,
    }))
}

pub fn page_url(ui: &Ui, repo: &str, hash: &str) -> String {
    format!("{}/?t={}&repo={repo}#{hash}", ui.origin, ui.token)
}

pub fn open_browser(url: &str) {
    if std::env::var_os("ZPR_NO_BROWSER").is_some() {
        return;
    }
    let _ = if cfg!(windows) {
        Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };
}

fn reply(status: u16, content_type: &str, body: Vec<u8>) -> HttpResponse {
    let header = |name: &str, value: &str| Header::from_bytes(name, value).expect("static header");
    Response::from_data(body)
        .with_status_code(status)
        .with_header(header("Content-Type", content_type))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header(
            "Content-Security-Policy",
            "default-src 'self'; img-src data:; style-src 'unsafe-inline'; script-src 'unsafe-inline'",
        ))
}

fn header_value(request: &Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.to_string())
}

fn serve(mut request: Request, ctx: &ServerState) {
    let response = respond(&mut request, ctx);
    let _ = request.respond(response);
}

fn respond(request: &mut Request, ctx: &ServerState) -> HttpResponse {
    let forbidden = || reply(403, "text/plain", b"forbidden".to_vec());
    let host = header_value(request, "Host").unwrap_or_default();
    if !ctx.hosts.contains(&host) {
        return forbidden();
    }
    let url = request.url().to_owned();
    let (path, query) = url.split_once('?').unwrap_or((url.as_str(), ""));
    let query = parse_query(query);
    let token = header_value(request, "X-Token")
        .or_else(|| query.get("t").cloned())
        .unwrap_or_default();
    if token != ctx.token {
        return forbidden();
    }
    if path == "/" {
        return reply(200, "text/html; charset=utf-8", INDEX_HTML.into());
    }
    let method = request.method().clone();
    let mut body = Value::Null;
    if method == Method::Post {
        let mut raw = String::new();
        let read = request
            .as_reader()
            .take(MAX_BODY_BYTES)
            .read_to_string(&mut raw);
        body = read
            .ok()
            .and_then(|_| serde_json::from_str(&raw).ok())
            .unwrap_or(Value::Null);
    }
    match api(&method, path, &query, &body) {
        Ok(value) => reply(200, "application/json", value.to_string().into_bytes()),
        Err(e) => reply(
            500,
            "application/json",
            json!({ "error": format!("{e:#}") })
                .to_string()
                .into_bytes(),
        ),
    }
}

fn required_str<'a>(body: &'a Value, key: &str) -> Result<&'a str> {
    body[key]
        .as_str()
        .with_context(|| format!("`{key}` is required"))
}

fn api(
    method: &Method,
    path: &str,
    query: &HashMap<String, String>,
    body: &Value,
) -> Result<Value> {
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let repo = || gh::resolve_repo(query.get("repo").map(String::as_str));
    let number = |index: usize| -> Result<u64> {
        segments
            .get(index)
            .and_then(|s| s.parse().ok())
            .context("invalid pull request number")
    };
    let ref_param = |key: &str| query.get(key).cloned().unwrap_or_default();

    match (method, segments.as_slice()) {
        (Method::Get, ["api", "pulls"]) => Ok(json!(pulls::list_open(&repo()?)?)),
        (Method::Get, ["api", "pulls", _]) => {
            let (repo, number) = (repo()?, number(2)?);
            Ok(json!({
                "details": pulls::details(&repo, number)?,
                "files": pulls::files(&repo, number)?,
            }))
        }
        (Method::Get, ["api", "pulls", _, "review"]) => threads::review_data(&repo()?, number(2)?),
        (Method::Get, ["api", "pulls", _, "checks"]) => pulls::checks(&repo()?, number(2)?),
        (Method::Post, ["api", "pulls", _, "review"]) => {
            let comments: Vec<ReviewComment> = body
                .get("comments")
                .map(|c| serde_json::from_value(c.clone()))
                .transpose()
                .context("invalid comments")?
                .unwrap_or_default();
            let summary = body["body"].as_str().unwrap_or_default();
            reviews::submit_review(
                &repo()?,
                number(2)?,
                required_str(body, "event")?,
                summary,
                comments,
            )?;
            Ok(json!({ "ok": true }))
        }
        (Method::Post, ["api", "pulls", _, "reply"]) => {
            let comment_id = body["commentId"]
                .as_u64()
                .context("`commentId` is required")?;
            threads::reply(
                &repo()?,
                number(2)?,
                comment_id,
                required_str(body, "body")?,
            )?;
            Ok(json!({ "ok": true }))
        }
        (Method::Post, ["api", "pulls", _, "viewed"]) => {
            let viewed = body["viewed"].as_bool().unwrap_or(true);
            threads::set_file_viewed(
                required_str(body, "prId")?,
                required_str(body, "path")?,
                viewed,
            )?;
            Ok(json!({ "ok": true }))
        }
        (Method::Post, ["api", "threads", "resolve"]) => {
            let resolved = body["resolved"].as_bool().unwrap_or(true);
            threads::set_thread_resolved(required_str(body, "id")?, resolved)?;
            Ok(json!({ "ok": true }))
        }
        (Method::Get, ["api", "new-pr"]) => {
            create::prepare(&repo()?, query.get("base").map(String::as_str))
        }
        (Method::Post, ["api", "push"]) => Ok(json!({ "pushed": create::push_current_branch()? })),
        (Method::Post, ["api", "pulls"]) => create::create_pull(
            &repo()?,
            required_str(body, "title")?,
            body["body"].as_str().unwrap_or_default(),
            required_str(body, "base")?,
            body["draft"].as_bool().unwrap_or(false),
        ),
        (Method::Get, ["api", "branches"]) => compare::branches(&repo()?),
        (Method::Get, ["api", "compare"]) => {
            let (base, head) = (ref_param("base"), ref_param("head"));
            if query.get("local").is_some_and(|v| v == "1") {
                compare::local(&compare::local_ref(&base)?, &head)
            } else {
                compare::github(&repo()?, &base, &head)
            }
        }
        (Method::Post, ["api", "open"]) => open_in_zed(body),
        _ => bail!("not found: {path}"),
    }
}

/// Opens a file of the current working directory in Zed (`zed path:line`).
fn open_in_zed(body: &Value) -> Result<Value> {
    let path = required_str(body, "path")?;
    let line = body["line"].as_u64().unwrap_or(1);
    let relative = Path::new(path);
    let is_plain_relative = relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    if path.starts_with('-') || !is_plain_relative {
        bail!("invalid path");
    }
    if !relative.exists() {
        let cwd = std::env::current_dir().unwrap_or_default();
        bail!(
            "{path} does not exist in {}; check out the branch first",
            cwd.display()
        );
    }
    let zed = std::env::var("ZPR_ZED_BIN").unwrap_or_else(|_| "zed".to_owned());
    Command::new(zed)
        .arg(format!("{path}:{line}"))
        .spawn()
        .context("could not run the `zed` command; add the Zed CLI to PATH or set ZPR_ZED_BIN")?;
    Ok(json!({ "ok": true }))
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (percent_decode(key), percent_decode(value)))
        .collect()
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%' && i + 2 < bytes.len())
            .then(|| std::str::from_utf8(&bytes[i + 1..i + 3]).ok())
            .flatten()
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (_, Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (b'+', None) => {
                out.push(b' ');
                i += 1;
            }
            (byte, None) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_query_strings() {
        let q = parse_query("base=feat%2Fa+b&head=main&t=abc");
        assert_eq!(q["base"], "feat/a b");
        assert_eq!(q["head"], "main");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%e2%9c%93"), "✓");
    }
}
